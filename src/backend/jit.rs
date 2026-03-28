use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module, ModuleError};
use cranelift_codegen as codegen;
use codegen::ir::{MemFlags, types, AbiParam, InstBuilder, BlockArg};
use codegen::ir::condcodes::IntCC;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use codegen::ir::condcodes::FloatCC;
use crate::backend::vm::{Value as VMValue, Trace, TraceOp, QNAN_BASE, TAG_INT, TAG_BOOL};

pub type JITFunction = unsafe extern "C" fn(*mut VMValue, *mut VMValue, *const VMValue) -> i32;

#[inline(always)]
fn trusted() -> MemFlags {
    let mut f = MemFlags::new();
    f.set_notrap();
    f.set_aligned();
    f
}

pub struct JIT {
    builder_context: FunctionBuilderContext,
    ctx: codegen::Context,
    module: JITModule,
}

impl JIT {
    pub fn new() -> Self {
        let mut flag_builder = codegen::settings::builder();
        flag_builder.set("use_colocated_libcalls", "false").unwrap();
        flag_builder.set("is_pic", "false").unwrap();
        flag_builder.set("opt_level", "speed").unwrap();
        flag_builder.set("regalloc_checker", "false").unwrap();

        let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
            panic!("host machine is not supported: {}", msg);
        });
        let isa = isa_builder
            .finish(codegen::settings::Flags::new(flag_builder))
            .unwrap();

        let builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        let module = JITModule::new(builder);

        Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: codegen::Context::new(),
            module,
        }
    }

    pub fn compile(&mut self, trace: &Trace) -> Result<*const u8, String> {
        self.module.clear_context(&mut self.ctx);

        let mut sig = self.module.make_signature();
        let ptr_type = self.module.target_config().pointer_type();
        sig.params.push(AbiParam::new(ptr_type)); // locals_ptr
        sig.params.push(AbiParam::new(ptr_type)); // globals_ptr
        sig.params.push(AbiParam::new(ptr_type)); // consts_ptr
        sig.returns.push(AbiParam::new(types::I32)); // next_ip

        let func_id = self.module
            .declare_function(
                &format!("trace_{}", trace.start_ip),
                Linkage::Export,
                &sig,
            )
            .map_err(|e: ModuleError| e.to_string())?;

        self.ctx.func.signature = sig;
        let mut b = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);

        let entry_block = b.create_block();
        b.append_block_params_for_function_params(entry_block);
        b.switch_to_block(entry_block);

        let locals_ptr  = b.block_params(entry_block)[0];
        let globals_ptr = b.block_params(entry_block)[1];
        let _consts_ptr = b.block_params(entry_block)[2];

        macro_rules! no_args {
            () => { std::iter::empty::<&BlockArg>() };
        }

        let qnan_tag_int  = b.ins().iconst(types::I64, (QNAN_BASE | TAG_INT)  as i64);
        let qnan_tag_bool = b.ins().iconst(types::I64, (QNAN_BASE | TAG_BOOL) as i64);
        let mask_48       = b.ins().iconst(types::I64, 0x0000_FFFF_FFFF_FFFFu64 as i64);

        macro_rules! unpack_int {
            ($val:expr) => {{
                let shl = b.ins().ishl_imm($val, 16);
                b.ins().sshr_imm(shl, 16)
            }};
        }
        macro_rules! pack_int {
            ($raw:expr) => {{
                let lo = b.ins().band($raw, mask_48);
                b.ins().bor(qnan_tag_int, lo)
            }};
        }
        macro_rules! pack_bool {
            ($bit:expr) => {{
                b.ins().bor(qnan_tag_bool, $bit)
            }};
        }

        macro_rules! unpack_float {
            ($val:expr) => {{
                b.ins().bitcast(types::F64, MemFlags::new(), $val)
            }};
        }
        macro_rules! pack_float {
            ($raw:expr) => {{
                let bits = b.ins().bitcast(types::I64, MemFlags::new(), $raw);
                let qnan = b.ins().iconst(types::I64, QNAN_BASE as i64);
                let masked = b.ins().band(bits, qnan);
                let is_qnan = b.ins().icmp(IntCC::Equal, masked, qnan);
                let ext_qnan = b.ins().uextend(types::I64, is_qnan);
                b.ins().bor(bits, ext_qnan)
            }};
        }


        let has_loop = trace.ops.iter().any(|op| {
            matches!(op,
                TraceOp::IncVarLoopNext   { .. } |
                TraceOp::IncLocalLoopNext { .. } |
                TraceOp::LoopNextInt      { .. })
        });

        let loop_header: Option<Block> = if has_loop { Some(b.create_block()) } else { None };

        let mut entry_sealed      = false;
        let mut current_terminated = false;

        macro_rules! ensure_loop_started {
            () => {{
                let lh = loop_header.expect("ensure_loop_started: no loop_header");
                if !entry_sealed {
                    b.ins().jump(lh, &[]);
                    b.seal_block(entry_block);
                    entry_sealed = true;
                }
                b.switch_to_block(lh);
            }};
        }

        macro_rules! emit_loop_exit {
            ($cond:expr, $loop_target:expr, $exit_ip:expr) => {{
                let lh = loop_header.expect("emit_loop_exit: no loop_header");

                if $loop_target as usize == trace.start_ip {
                    let exit_blk = b.create_block();
                    b.ins().brif($cond, lh, no_args!(), exit_blk, no_args!());
                    b.seal_block(lh);
                    b.switch_to_block(exit_blk);
                    b.seal_block(exit_blk);
                    let rv = b.ins().iconst(types::I32, $exit_ip as i64);
                    b.ins().return_(&[rv]);
                } else {
                    let taken_blk = b.create_block();
                    let exit_blk  = b.create_block();
                    b.ins().brif($cond, taken_blk, no_args!(), exit_blk, no_args!());
                    b.seal_block(lh);
                    b.switch_to_block(taken_blk);
                    b.seal_block(taken_blk);
                    let rt = b.ins().iconst(types::I32, $loop_target as i64);
                    b.ins().return_(&[rt]);
                    b.switch_to_block(exit_blk);
                    b.seal_block(exit_blk);
                    let re = b.ins().iconst(types::I32, $exit_ip as i64);
                    b.ins().return_(&[re]);
                }
                current_terminated = true;
            }};
        }

        for op in &trace.ops {
            if current_terminated { break; }

            match op {

                TraceOp::LoadConst { dst, val } => {
                    let bits = b.ins().iconst(types::I64, val.0 as i64);
                    let addr = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), bits, addr, 0);
                }

                TraceOp::Move { dst, src } => {
                    let sa = b.ins().iadd_imm(locals_ptr, (*src as i64) * 8);
                    let sv = b.ins().load(types::I64, trusted(), sa, 0);
                    let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), sv, da, 0);
                }

                TraceOp::GetVar { dst, idx } => {
                    let ga = b.ins().iadd_imm(globals_ptr, (*idx as i64) * 8);
                    let gv = b.ins().load(types::I64, trusted(), ga, 0);
                    let la = b.ins().iadd_imm(locals_ptr,  (*dst as i64) * 8);
                    b.ins().store(trusted(), gv, la, 0);
                }

                TraceOp::SetVar { idx, src } => {
                    let la = b.ins().iadd_imm(locals_ptr,  (*src as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    let ga = b.ins().iadd_imm(globals_ptr, (*idx as i64) * 8);
                    b.ins().store(trusted(), lv, ga, 0);
                }

                TraceOp::GuardInt { reg, ip: fail_ip } => {
                    let la       = b.ins().iadd_imm(locals_ptr, (*reg as i64) * 8);
                    let lv       = b.ins().load(types::I64, trusted(), la, 0);
                    let tag_word = b.ins().ushr_imm(lv, 48);
                    let expected = b.ins().iconst(
                        types::I64,
                        ((QNAN_BASE | TAG_INT) >> 48) as i64,
                    );
                    let not_int = b.ins().icmp(IntCC::NotEqual, tag_word, expected);
                    let fail = b.create_block();
                    let ok   = b.create_block();
                    b.ins().brif(not_int, fail, no_args!(), ok, no_args!());
                    b.switch_to_block(fail); b.seal_block(fail);
                    let rv = b.ins().iconst(types::I32, *fail_ip as i64);
                    b.ins().return_(&[rv]);
                    b.switch_to_block(ok); b.seal_block(ok);
                }

                TraceOp::GuardFloat { reg, ip: fail_ip } => {
                    let la = b.ins().iadd_imm(locals_ptr, (*reg as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    
                    let mask = b.ins().iconst(types::I64, QNAN_BASE as i64);
                    let masked = b.ins().band(lv, mask);
                    let is_float = b.ins().icmp(IntCC::NotEqual, masked, mask);

                    let fail = b.create_block();
                    let ok   = b.create_block();
                    
                    b.ins().brif(is_float, ok, no_args!(), fail, no_args!());
                    b.switch_to_block(fail); b.seal_block(fail);
                    let rv = b.ins().iconst(types::I32, *fail_ip as i64);
                    b.ins().return_(&[rv]);
                    b.switch_to_block(ok); b.seal_block(ok);
                }

                TraceOp::CastIntToFloat { dst, src } => {
                    let sa = b.ins().iadd_imm(locals_ptr, (*src as i64) * 8);
                    let sv = b.ins().load(types::I64, trusted(), sa, 0);
                    let i  = unpack_int!(sv);
                    let f  = b.ins().fcvt_from_sint(types::F64, i);
                    let fb = pack_float!(f);
                    let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), fb, da, 0);
                }


                TraceOp::AddInt { dst, src1, src2 } => {
                    let la = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    let ra = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let rv = b.ins().load(types::I64, trusted(), ra, 0);
                    let l  = unpack_int!(lv);
                    let r  = if src1 == src2 { l } else { unpack_int!(rv) };
                    let s  = b.ins().iadd(l, r);
                    let sb = pack_int!(s);
                    let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), sb, da, 0);
                }

                TraceOp::SubInt { dst, src1, src2 } => {
                    let la = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    let ra = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let rv = b.ins().load(types::I64, trusted(), ra, 0);
                    let l  = unpack_int!(lv);
                    let r  = if src1 == src2 { l } else { unpack_int!(rv) };
                    let s  = b.ins().isub(l, r);
                    let sb = pack_int!(s);
                    let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), sb, da, 0);
                }

                TraceOp::MulInt { dst, src1, src2 } => {
                    let la = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    let ra = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let rv = b.ins().load(types::I64, trusted(), ra, 0);
                    let l  = unpack_int!(lv);
                    let r  = if src1 == src2 { l } else { unpack_int!(rv) };
                    let s  = b.ins().imul(l, r);
                    let sb = pack_int!(s);
                    let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), sb, da, 0);
                }

                TraceOp::DivInt { dst, src1, src2, fail_ip } => {
                    let la = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    let ra = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let rv = b.ins().load(types::I64, trusted(), ra, 0);
                    let l  = unpack_int!(lv);
                    let r  = if src1 == src2 { l } else { unpack_int!(rv) };

                    let is_zero = b.ins().icmp_imm(IntCC::Equal, r, 0);
                    let is_min  = b.ins().icmp_imm(IntCC::Equal, l, i64::MIN);
                    let is_minus_one = b.ins().icmp_imm(IntCC::Equal, r, -1);
                    let is_overflow = b.ins().band(is_min, is_minus_one);
                    let should_fail = b.ins().bor(is_zero, is_overflow);

                    let fail = b.create_block();
                    let ok   = b.create_block();
                    b.ins().brif(should_fail, fail, no_args!(), ok, no_args!());
                    
                    b.switch_to_block(fail);
                    b.seal_block(fail);
                    let rv_fail = b.ins().iconst(types::I32, *fail_ip as i64);
                    b.ins().return_(&[rv_fail]);

                    b.switch_to_block(ok);
                    b.seal_block(ok);
                    let s = b.ins().sdiv(l, r);
                    let sb = pack_int!(s);
                    let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), sb, da, 0);
                }

                TraceOp::ModInt { dst, src1, src2, fail_ip } => {
                    let la = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    let ra = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let rv = b.ins().load(types::I64, trusted(), ra, 0);
                    let l  = unpack_int!(lv);
                    let r  = if src1 == src2 { l } else { unpack_int!(rv) };

                    let is_zero = b.ins().icmp_imm(IntCC::Equal, r, 0);
                    let is_min  = b.ins().icmp_imm(IntCC::Equal, l, i64::MIN);
                    let is_minus_one = b.ins().icmp_imm(IntCC::Equal, r, -1);
                    let is_overflow = b.ins().band(is_min, is_minus_one);
                    let should_fail = b.ins().bor(is_zero, is_overflow);

                    let fail = b.create_block();
                    let ok   = b.create_block();
                    b.ins().brif(should_fail, fail, no_args!(), ok, no_args!());
                    
                    b.switch_to_block(fail);
                    b.seal_block(fail);
                    let rv_fail = b.ins().iconst(types::I32, *fail_ip as i64);
                    b.ins().return_(&[rv_fail]);

                    b.switch_to_block(ok);
                    b.seal_block(ok);
                    let s = b.ins().srem(l, r);
                    let sb = pack_int!(s);
                    let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), sb, da, 0);
                }

                TraceOp::AddFloat { dst, src1, src2 } => {
                    let la = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    let ra = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let rv = b.ins().load(types::I64, trusted(), ra, 0);
                    let l  = unpack_float!(lv);
                    let r  = if src1 == src2 { l } else { unpack_float!(rv) };
                    let s  = b.ins().fadd(l, r);
                    let sb = pack_float!(s);
                    let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), sb, da, 0);
                }

                TraceOp::SubFloat { dst, src1, src2 } => {
                    let la = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    let ra = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let rv = b.ins().load(types::I64, trusted(), ra, 0);
                    let l  = unpack_float!(lv);
                    let r  = if src1 == src2 { l } else { unpack_float!(rv) };
                    let s  = b.ins().fsub(l, r);
                    let sb = pack_float!(s);
                    let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), sb, da, 0);
                }

                TraceOp::MulFloat { dst, src1, src2 } => {
                    let la = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    let ra = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let rv = b.ins().load(types::I64, trusted(), ra, 0);
                    let l  = unpack_float!(lv);
                    let r  = if src1 == src2 { l } else { unpack_float!(rv) };
                    let s  = b.ins().fmul(l, r);
                    let sb = pack_float!(s);
                    let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), sb, da, 0);
                }

                TraceOp::DivFloat { dst, src1, src2, fail_ip } => {
                    let la = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    let ra = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let rv = b.ins().load(types::I64, trusted(), ra, 0);
                    let l  = unpack_float!(lv);
                    let r  = if src1 == src2 { l } else { unpack_float!(rv) };

                    let zero = b.ins().f32const(0.0);
                    let zero_f64 = b.ins().fpromote(types::F64, zero);
                    let is_zero = b.ins().fcmp(FloatCC::Equal, r, zero_f64);

                    let fail = b.create_block();
                    let ok   = b.create_block();
                    b.ins().brif(is_zero, fail, no_args!(), ok, no_args!());
                    
                    b.switch_to_block(fail);
                    b.seal_block(fail);
                    let rv_fail = b.ins().iconst(types::I32, *fail_ip as i64);
                    b.ins().return_(&[rv_fail]);

                    b.switch_to_block(ok);
                    b.seal_block(ok);
                    let s = b.ins().fdiv(l, r);
                    let sb = pack_float!(s);
                    let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), sb, da, 0);
                }

                TraceOp::ModFloat { dst, src1, src2, fail_ip } => {
                    let la = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    let ra = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let rv = b.ins().load(types::I64, trusted(), ra, 0);
                    let l  = unpack_float!(lv);
                    let r  = if src1 == src2 { l } else { unpack_float!(rv) };

                    let zero = b.ins().f32const(0.0);
                    let zero_f64 = b.ins().fpromote(types::F64, zero);
                    let is_zero = b.ins().fcmp(FloatCC::Equal, r, zero_f64);

                    let fail = b.create_block();
                    let ok   = b.create_block();
                    b.ins().brif(is_zero, fail, no_args!(), ok, no_args!());
                    
                    b.switch_to_block(fail);
                    b.seal_block(fail);
                    let rv_fail = b.ins().iconst(types::I32, *fail_ip as i64);
                    b.ins().return_(&[rv_fail]);

                    b.switch_to_block(ok);
                    b.seal_block(ok);
                    let div = b.ins().fdiv(l, r);
                    let trunc = b.ins().trunc(div);
                    let mul = b.ins().fmul(trunc, r);
                    let s = b.ins().fsub(l, mul);
                    
                    let sb = pack_float!(s);
                    let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), sb, da, 0);
                }

                TraceOp::IncLocal { reg } => {
                    let la   = b.ins().iadd_imm(locals_ptr, (*reg as i64) * 8);
                    let lv   = b.ins().load(types::I64, trusted(), la, 0);
                    let next = b.ins().iadd_imm(lv, 1);
                    b.ins().store(trusted(), next, la, 0);
                }

                TraceOp::IncVar { g_idx } => {
                    let ga   = b.ins().iadd_imm(globals_ptr, (*g_idx as i64) * 8);
                    let gv   = b.ins().load(types::I64, trusted(), ga, 0);
                    let gnxt = b.ins().iadd_imm(gv, 1);
                    b.ins().store(trusted(), gnxt, ga, 0);
                }


                TraceOp::CmpInt { dst, src1, src2, cc } => {
                    let la     = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let lv     = b.ins().load(types::I64, trusted(), la, 0);
                    let ra     = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let rv     = b.ins().load(types::I64, trusted(), ra, 0);
                    let l      = unpack_int!(lv);
                    let r      = if src1 == src2 { l } else { unpack_int!(rv) };
                    let int_cc = decode_intcc(*cc);
                    let cmp_i8 = b.ins().icmp(int_cc, l, r);
                    let cmp64  = b.ins().uextend(types::I64, cmp_i8);
                    let boxed  = pack_bool!(cmp64);
                    let da     = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), boxed, da, 0);
                }

                TraceOp::CmpFloat { dst, src1, src2, cc } => {
                    let la     = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let lv     = b.ins().load(types::I64, trusted(), la, 0);
                    let ra     = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let rv     = b.ins().load(types::I64, trusted(), ra, 0);
                    let l      = unpack_float!(lv);
                    let r      = if src1 == src2 { l } else { unpack_float!(rv) };
                    let flt_cc = decode_floatcc(*cc);
                    let cmp_i8 = b.ins().fcmp(flt_cc, l, r);
                    let cmp64  = b.ins().uextend(types::I64, cmp_i8);
                    let boxed  = pack_bool!(cmp64);
                    let da     = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), boxed, da, 0);
                }

                TraceOp::And { dst, src1, src2 } => {
                    let la = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    let ra = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let rv = b.ins().load(types::I64, trusted(), ra, 0);
                    let b1 = b.ins().band_imm(lv, 1);
                    let b2 = b.ins().band_imm(rv, 1);
                    let res = b.ins().band(b1, b2);
                    let boxed = pack_bool!(res);
                    let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), boxed, da, 0);
                }

                TraceOp::Or { dst, src1, src2 } => {
                    let la = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    let ra = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let rv = b.ins().load(types::I64, trusted(), ra, 0);
                    let b1 = b.ins().band_imm(lv, 1);
                    let b2 = b.ins().band_imm(rv, 1);
                    let res = b.ins().bor(b1, b2);
                    let boxed = pack_bool!(res);
                    let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), boxed, da, 0);
                }

                TraceOp::Not { dst, src } => {
                    let sa = b.ins().iadd_imm(locals_ptr, (*src as i64) * 8);
                    let sv = b.ins().load(types::I64, trusted(), sa, 0);
                    let bit = b.ins().band_imm(sv, 1);
                    let res = b.ins().bxor_imm(bit, 1);
                    let boxed = pack_bool!(res);
                    let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), boxed, da, 0);
                }

                TraceOp::GuardTrue { reg, fail_ip } => {
                    let ca     = b.ins().iadd_imm(locals_ptr, (*reg as i64) * 8);
                    let cv     = b.ins().load(types::I64, trusted(), ca, 0);
                    let bit    = b.ins().band_imm(cv, 1);
                    let is_false = b.ins().icmp_imm(IntCC::Equal, bit, 0);

                    let fail = b.create_block();
                    let ok   = b.create_block();
                    b.ins().brif(is_false, fail, no_args!(), ok, no_args!());
                    b.switch_to_block(fail); b.seal_block(fail);
                    let rv = b.ins().iconst(types::I32, *fail_ip as i64);
                    b.ins().return_(&[rv]);
                    b.switch_to_block(ok); b.seal_block(ok);
                }

                TraceOp::GuardFalse { reg, fail_ip } => {
                    let ca     = b.ins().iadd_imm(locals_ptr, (*reg as i64) * 8);
                    let cv     = b.ins().load(types::I64, trusted(), ca, 0);
                    let bit    = b.ins().band_imm(cv, 1);
                    let is_true = b.ins().icmp_imm(IntCC::NotEqual, bit, 0);

                    let fail = b.create_block();
                    let ok   = b.create_block();
                    b.ins().brif(is_true, fail, no_args!(), ok, no_args!());
                    b.switch_to_block(fail); b.seal_block(fail);
                    let rv = b.ins().iconst(types::I32, *fail_ip as i64);
                    b.ins().return_(&[rv]);
                    b.switch_to_block(ok); b.seal_block(ok);
                }

                TraceOp::IncVarLoopNext { g_idx, reg, limit_reg, target, exit_ip } => {
                    ensure_loop_started!();

                    let ga   = b.ins().iadd_imm(globals_ptr, (*g_idx as i64) * 8);
                    let gv   = b.ins().load(types::I64, trusted(), ga, 0);
                    let gnxt = b.ins().iadd_imm(gv, 1);
                    b.ins().store(trusted(), gnxt, ga, 0);

                    let la   = b.ins().iadd_imm(locals_ptr, (*reg as i64) * 8);
                    let lv   = b.ins().load(types::I64, trusted(), la, 0);
                    let lim_a = b.ins().iadd_imm(locals_ptr, (*limit_reg as i64) * 8);
                    let lim_v = b.ins().load(types::I64, trusted(), lim_a, 0);
                    let lnxt = b.ins().iadd_imm(lv, 1);
                    b.ins().store(trusted(), lnxt, la, 0);

                    let cond = b.ins().icmp(IntCC::SignedLessThanOrEqual, lnxt, lim_v);
                    emit_loop_exit!(cond, *target, *exit_ip);
                }

                TraceOp::IncLocalLoopNext { inc_reg, reg, limit_reg, target, exit_ip } => {
                    ensure_loop_started!();

                    let ia   = b.ins().iadd_imm(locals_ptr, (*inc_reg as i64) * 8);
                    let iv   = b.ins().load(types::I64, trusted(), ia, 0);
                    let inxt = b.ins().iadd_imm(iv, 1);
                    b.ins().store(trusted(), inxt, ia, 0);

                    let la    = b.ins().iadd_imm(locals_ptr, (*reg as i64) * 8);
                    let lv    = b.ins().load(types::I64, trusted(), la, 0);
                    let lim_a = b.ins().iadd_imm(locals_ptr, (*limit_reg as i64) * 8);
                    let lim_v = b.ins().load(types::I64, trusted(), lim_a, 0);
                    let lnxt  = b.ins().iadd_imm(lv, 1);
                    b.ins().store(trusted(), lnxt, la, 0);

                    let lnxt_raw = unpack_int!(lnxt);
                    let lim_raw  = unpack_int!(lim_v);
                    let cond = b.ins().icmp(IntCC::SignedLessThanOrEqual, lnxt_raw, lim_raw);
                    emit_loop_exit!(cond, *target, *exit_ip);
                }

                TraceOp::LoopNextInt { reg, limit_reg, target, exit_ip } => {
                    ensure_loop_started!();

                    let la    = b.ins().iadd_imm(locals_ptr, (*reg as i64) * 8);
                    let lv    = b.ins().load(types::I64, trusted(), la, 0);
                    let lim_a = b.ins().iadd_imm(locals_ptr, (*limit_reg as i64) * 8);
                    let lim_v = b.ins().load(types::I64, trusted(), lim_a, 0);
                    let lnxt  = b.ins().iadd_imm(lv, 1);
                    b.ins().store(trusted(), lnxt, la, 0);

                    let cond = b.ins().icmp(IntCC::SignedLessThanOrEqual, lnxt, lim_v);
                    emit_loop_exit!(cond, *target, *exit_ip);
                }

                _ => { /* unknown op — recording should have filtered this */ }
            }
        }

        if !current_terminated {
            if !entry_sealed {
                b.seal_block(entry_block);
            }
            let rv = b.ins().iconst(types::I32, 0);
            b.ins().return_(&[rv]);
        }

        b.finalize();

        match self.module.define_function(func_id, &mut self.ctx) {
            Ok(_) => {
                self.module
                    .finalize_definitions()
                    .map_err(|e: ModuleError| {
                        format!(
                            "Cranelift finalize_definitions failed: {}\nIR:\n{}",
                            e,
                            self.ctx.func.display()
                        )
                    })?;
                let code = self.module.get_finalized_function(func_id);
                Ok(code)
            }
            Err(e) => {
                let ir = format!("{}", self.ctx.func.display());
                Err(format!(
                    "Cranelift define_function failed: {}\nIR:\n{}",
                    e, ir
                ))
            }
        }
    }
}

#[inline]
pub fn decode_intcc(cc: u8) -> IntCC {
    match cc {
        0 => IntCC::Equal,
        1 => IntCC::NotEqual,
        2 => IntCC::SignedGreaterThan,
        3 => IntCC::SignedLessThan,
        4 => IntCC::SignedGreaterThanOrEqual,
        5 => IntCC::SignedLessThanOrEqual,
        _ => IntCC::Equal,
    }
}


#[inline]
pub fn encode_intcc(cc: IntCC) -> u8 {
    match cc {
        IntCC::Equal                    => 0,
        IntCC::NotEqual                 => 1,
        IntCC::SignedGreaterThan        => 2,
        IntCC::SignedLessThan           => 3,
        IntCC::SignedGreaterThanOrEqual => 4,
        IntCC::SignedLessThanOrEqual    => 5,
        _                               => 0,
    }
}


#[inline]
pub fn decode_floatcc(cc: u8) -> FloatCC {
    match cc {
        0 => FloatCC::Equal,
        1 => FloatCC::NotEqual,
        2 => FloatCC::GreaterThan,
        3 => FloatCC::LessThan,
        4 => FloatCC::GreaterThanOrEqual,
        5 => FloatCC::LessThanOrEqual,
        _ => FloatCC::Equal,
    }
}