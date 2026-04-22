use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module, ModuleError};
use cranelift_codegen as codegen;
use codegen::ir::{MemFlags, types, AbiParam, InstBuilder, BlockArg};
use codegen::ir::condcodes::IntCC;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use codegen::ir::condcodes::FloatCC;
use crate::backend::vm::{
    Value as VMValue, Trace, TraceOp, QNAN_BASE, TAG_INT, TAG_BOOL, TAG_STR,
    xcx_jit_random_int, xcx_jit_random_float, xcx_jit_pow_int, xcx_jit_pow_float, xcx_jit_int_concat, xcx_jit_has, xcx_jit_random_choice,
    xcx_jit_array_size, xcx_jit_array_get, xcx_jit_array_push, xcx_jit_array_update, xcx_jit_call_recursive, xcx_jit_set_size, xcx_jit_set_contains,
    xcx_jit_inc_ref, xcx_jit_dec_ref, xcx_jit_method_dispatch
};

pub type JITFunction = unsafe extern "C" fn(*mut VMValue, *mut VMValue, *const VMValue) -> i32;
pub type MethodJitFunction = unsafe extern "C" fn(*mut VMValue, *mut VMValue, *const VMValue, *mut crate::backend::vm::VM, *mut crate::backend::vm::Executor) -> u64;

#[inline(always)]
fn trusted() -> MemFlags {
    let mut f = MemFlags::new();
    f.set_notrap();
    f.set_aligned();
    f
}

pub struct JIT {
    builder_context: FunctionBuilderContext,
    pub ctx: codegen::Context,
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

        let mut builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        builder.symbol("xcx_jit_random_int", xcx_jit_random_int as *const u8);
        builder.symbol("xcx_jit_random_float", xcx_jit_random_float as *const u8);
        builder.symbol("xcx_jit_pow_int", xcx_jit_pow_int as *const u8);
        builder.symbol("xcx_jit_pow_float", xcx_jit_pow_float as *const u8);
        builder.symbol("xcx_jit_int_concat", xcx_jit_int_concat as *const u8);
        builder.symbol("xcx_jit_has", xcx_jit_has as *const u8);
        builder.symbol("xcx_jit_random_choice", xcx_jit_random_choice as *const u8);
        builder.symbol("xcx_jit_array_size", xcx_jit_array_size as *const u8);
        builder.symbol("xcx_jit_array_get", xcx_jit_array_get as *const u8);
        builder.symbol("xcx_jit_array_push", xcx_jit_array_push as *const u8);
        builder.symbol("xcx_jit_array_update", xcx_jit_array_update as *const u8);
        builder.symbol("xcx_jit_call_recursive", xcx_jit_call_recursive as *const u8);
        builder.symbol("xcx_jit_set_size", xcx_jit_set_size as *const u8);
        builder.symbol("xcx_jit_set_contains", xcx_jit_set_contains as *const u8);
        builder.symbol("xcx_jit_inc_ref", xcx_jit_inc_ref as *const u8);
        builder.symbol("xcx_jit_dec_ref", xcx_jit_dec_ref as *const u8);
        builder.symbol("xcx_jit_method_dispatch", xcx_jit_method_dispatch as *const u8);
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
                TraceOp::LoopNextInt      { .. })
        });

        let loop_header: Option<Block> = if has_loop { Some(b.create_block()) } else { None };

        let mut entry_sealed      = false;
        let mut current_terminated = false;

        macro_rules! ensure_loop_started {
            () => {{
                let lh = loop_header.expect("ensure_loop_started: no loop_header");
                if !entry_sealed {
                    b.ins().jump(lh, no_args!());
                    b.seal_block(entry_block);
                    entry_sealed = true;
                    b.switch_to_block(lh);
                }
            }};
        }

        macro_rules! emit_loop_exit {
            ($cond:expr, $loop_target:expr, $exit_ip:expr) => {{
                let lh = loop_header.expect("emit_loop_exit: no loop_header");

                if $loop_target as usize == trace.start_ip {
                    let exit_blk = b.create_block();
                    b.ins().brif($cond, lh, no_args!(), exit_blk, no_args!());
                    b.seal_block(exit_blk);
                    b.switch_to_block(exit_blk);
                    let rv = b.ins().iconst(types::I32, $exit_ip as i64);
                    b.ins().return_(&[rv]);
                } else {
                    let taken_blk = b.create_block();
                    let exit_blk  = b.create_block();
                    b.ins().brif($cond, taken_blk, no_args!(), exit_blk, no_args!());
                    b.seal_block(taken_blk);
                    b.switch_to_block(taken_blk);
                    let rt = b.ins().iconst(types::I32, $loop_target as i64);
                    b.ins().return_(&[rt]);
                    b.seal_block(exit_blk);
                    b.switch_to_block(exit_blk);
                    let re = b.ins().iconst(types::I32, $exit_ip as i64);
                    b.ins().return_(&[re]);
                }
                current_terminated = true;
            }};
        }

        for op in &trace.ops {
            if current_terminated { break; }
            if has_loop { ensure_loop_started!(); }

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
                
                TraceOp::RandomInt { dst, min, max, step, has_step } => {
                    let min_addr = b.ins().iadd_imm(locals_ptr, (*min as i64) * 8);
                    let min_val  = b.ins().load(types::I64, trusted(), min_addr, 0);
                    let min_i64  = unpack_int!(min_val);
                    
                    let max_addr = b.ins().iadd_imm(locals_ptr, (*max as i64) * 8);
                    let max_val  = b.ins().load(types::I64, trusted(), max_addr, 0);
                    let max_i64  = unpack_int!(max_val);
                    
                    let step_addr = b.ins().iadd_imm(locals_ptr, (*step as i64) * 8);
                    let step_val  = b.ins().load(types::I64, trusted(), step_addr, 0);
                    let step_i64  = unpack_int!(step_val);
                    
                    let has_step_addr = b.ins().iadd_imm(locals_ptr, (*has_step as i64) * 8);
                    let has_step_val  = b.ins().load(types::I64, trusted(), has_step_addr, 0);
                    let has_step_bool = b.ins().band_imm(has_step_val, 1);
                    
                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64)); // min
                    sig.params.push(AbiParam::new(types::I64)); // max
                    sig.params.push(AbiParam::new(types::I64)); // step
                    sig.params.push(AbiParam::new(types::I8));  // has_step
                    sig.returns.push(AbiParam::new(types::I64)); // result
                    
                    let callee = self.module.declare_function("xcx_jit_random_int", Linkage::Import, &sig).unwrap();
                    let local_callee = self.module.declare_func_in_func(callee, &mut b.func);
                    
                    let has_step_i8 = b.ins().ireduce(types::I8, has_step_bool);
                    let call = b.ins().call(local_callee, &[min_i64, max_i64, step_i64, has_step_i8]);
                    let res_i64 = b.inst_results(call)[0];
                    
                    let res_val = pack_int!(res_i64);
                    let dst_addr = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), res_val, dst_addr, 0);
                }

                TraceOp::RandomFloat { dst, min, max, step, has_step, step_is_float } => {
                    let min_addr = b.ins().iadd_imm(locals_ptr, (*min as i64) * 8);
                    let min_val  = b.ins().load(types::I64, trusted(), min_addr, 0);
                    let min_f64  = unpack_float!(min_val);
                    
                    let max_addr = b.ins().iadd_imm(locals_ptr, (*max as i64) * 8);
                    let max_val  = b.ins().load(types::I64, trusted(), max_addr, 0);
                    let max_f64  = unpack_float!(max_val);
                    
                    let step_addr = b.ins().iadd_imm(locals_ptr, (*step as i64) * 8);
                    let step_val  = b.ins().load(types::I64, trusted(), step_addr, 0);
                    let step_f64  = if *step_is_float {
                        unpack_float!(step_val)
                    } else {
                        let step_i64 = unpack_int!(step_val);
                        b.ins().fcvt_from_sint(types::F64, step_i64)
                    };
                    
                    let has_step_addr = b.ins().iadd_imm(locals_ptr, (*has_step as i64) * 8);
                    let has_step_val  = b.ins().load(types::I64, trusted(), has_step_addr, 0);
                    let has_step_bool = b.ins().band_imm(has_step_val, 1);
                    
                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::F64)); // min
                    sig.params.push(AbiParam::new(types::F64)); // max
                    sig.params.push(AbiParam::new(types::F64)); // step
                    sig.params.push(AbiParam::new(types::I8));  // has_step
                    sig.returns.push(AbiParam::new(types::F64)); // result
                    
                    let callee = self.module.declare_function("xcx_jit_random_float", Linkage::Import, &sig).unwrap();
                    let local_callee = self.module.declare_func_in_func(callee, &mut b.func);
                    
                    let has_step_i8 = b.ins().ireduce(types::I8, has_step_bool);
                    let call = b.ins().call(local_callee, &[min_f64, max_f64, step_f64, has_step_i8]);
                    let res_f64 = b.inst_results(call)[0];
                    
                    let res_val = pack_float!(res_f64);
                    let dst_addr = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), res_val, dst_addr, 0);
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

                TraceOp::PowInt { dst, src1, src2 } => {
                    let addr1 = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let val1  = b.ins().load(types::I64, trusted(), addr1, 0);
                    let i1    = unpack_int!(val1);

                    let addr2 = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let val2  = b.ins().load(types::I64, trusted(), addr2, 0);
                    let i2    = unpack_int!(val2);

                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I64));
                    sig.returns.push(AbiParam::new(types::I64));

                    let callee = self.module.declare_function("xcx_jit_pow_int", Linkage::Import, &sig).unwrap();
                    let local_callee = self.module.declare_func_in_func(callee, &mut b.func);
                    let call = b.ins().call(local_callee, &[i1, i2]);
                    let res = b.inst_results(call)[0];
                    let boxed = pack_int!(res);
                    let dst_addr = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), boxed, dst_addr, 0);
                }

                TraceOp::PowFloat { dst, src1, src2 } => {
                    let addr1 = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let val1  = b.ins().load(types::I64, trusted(), addr1, 0);
                    let f1    = unpack_float!(val1);

                    let addr2 = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let val2  = b.ins().load(types::I64, trusted(), addr2, 0);
                    let f2    = unpack_float!(val2);

                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::F64));
                    sig.params.push(AbiParam::new(types::F64));
                    sig.returns.push(AbiParam::new(types::F64));

                    let callee = self.module.declare_function("xcx_jit_pow_float", Linkage::Import, &sig).unwrap();
                    let local_callee = self.module.declare_func_in_func(callee, &mut b.func);
                    let call = b.ins().call(local_callee, &[f1, f2]);
                    let res = b.inst_results(call)[0];
                    let boxed = pack_float!(res);
                    let dst_addr = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), boxed, dst_addr, 0);
                }

                TraceOp::IntConcat { dst, src1, src2 } => {
                    let addr1 = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let val1  = b.ins().load(types::I64, trusted(), addr1, 0);
                    let i1    = unpack_int!(val1);

                    let addr2 = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let val2  = b.ins().load(types::I64, trusted(), addr2, 0);
                    let i2    = unpack_int!(val2);

                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I64));
                    sig.returns.push(AbiParam::new(types::I64));

                    let callee = self.module.declare_function("xcx_jit_int_concat", Linkage::Import, &sig).unwrap();
                    let local_callee = self.module.declare_func_in_func(callee, &mut b.func);
                    let call = b.ins().call(local_callee, &[i1, i2]);
                    let res = b.inst_results(call)[0];
                    let boxed = pack_int!(res);
                    let dst_addr = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), boxed, dst_addr, 0);
                }

                TraceOp::Has { dst, src1, src2 } => {
                    let addr1 = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let val1  = b.ins().load(types::I64, trusted(), addr1, 0);
                    let addr2 = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let val2  = b.ins().load(types::I64, trusted(), addr2, 0);

                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I64));
                    sig.returns.push(AbiParam::new(types::I8));

                    let callee = self.module.declare_function("xcx_jit_has", Linkage::Import, &sig).unwrap();
                    let local_callee = self.module.declare_func_in_func(callee, &mut b.func);
                    let call = b.ins().call(local_callee, &[val1, val2]);
                    let res = b.inst_results(call)[0];
                    let res64 = b.ins().uextend(types::I64, res);
                    let boxed = pack_bool!(res64);
                    let dst_addr = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), boxed, dst_addr, 0);
                }

                TraceOp::RandomChoice { dst, src } => {
                    let addr = b.ins().iadd_imm(locals_ptr, (*src as i64) * 8);
                    let val  = b.ins().load(types::I64, trusted(), addr, 0);

                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64));
                    sig.returns.push(AbiParam::new(types::I64));

                    let callee = self.module.declare_function("xcx_jit_random_choice", Linkage::Import, &sig).unwrap();
                    let local_callee = self.module.declare_func_in_func(callee, &mut b.func);
                    let call = b.ins().call(local_callee, &[val]);
                    let res = b.inst_results(call)[0];
                    let dst_addr = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), res, dst_addr, 0);
                }

                TraceOp::ArraySize { dst, src } => {
                    let addr = b.ins().iadd_imm(locals_ptr, (*src as i64) * 8);
                    let val  = b.ins().load(types::I64, trusted(), addr, 0);

                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64));
                    sig.returns.push(AbiParam::new(types::I64));

                    let callee = self.module.declare_function("xcx_jit_array_size", Linkage::Import, &sig).unwrap();
                    let local_callee = self.module.declare_func_in_func(callee, &mut b.func);
                    let call = b.ins().call(local_callee, &[val]);
                    let res_i64 = b.inst_results(call)[0];
                    let boxed = pack_int!(res_i64);
                    let dst_addr = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), boxed, dst_addr, 0);
                }

                TraceOp::ArrayGet { dst, arr_reg, idx_reg, fail_ip } => {
                    let a_addr = b.ins().iadd_imm(locals_ptr, (*arr_reg as i64) * 8);
                    let a_val  = b.ins().load(types::I64, trusted(), a_addr, 0);
                    let i_addr = b.ins().iadd_imm(locals_ptr, (*idx_reg as i64) * 8);
                    let i_val  = b.ins().load(types::I64, trusted(), i_addr, 0);
                    let idx    = unpack_int!(i_val);

                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I64));
                    sig.returns.push(AbiParam::new(types::I64));

                    let callee = self.module.declare_function("xcx_jit_array_get", Linkage::Import, &sig).unwrap();
                    let local_callee = self.module.declare_func_in_func(callee, &mut b.func);
                    let call = b.ins().call(local_callee, &[a_val, idx]);
                    let res = b.inst_results(call)[0];

                    let zero_val = b.ins().iconst(types::I64, 0);
                    let is_err   = b.ins().icmp(IntCC::Equal, res, zero_val);

                    let fail = b.create_block();
                    let ok   = b.create_block();
                    b.ins().brif(is_err, fail, no_args!(), ok, no_args!());

                    b.switch_to_block(fail); b.seal_block(fail);
                    let rv = b.ins().iconst(types::I32, *fail_ip as i64);
                    b.ins().return_(&[rv]);

                    b.switch_to_block(ok); b.seal_block(ok);
                    let dst_addr = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), res, dst_addr, 0);
                }

                TraceOp::ArrayPush { arr_reg, val_reg } => {
                    let a_addr = b.ins().iadd_imm(locals_ptr, (*arr_reg as i64) * 8);
                    let a_val  = b.ins().load(types::I64, trusted(), a_addr, 0);
                    let v_addr = b.ins().iadd_imm(locals_ptr, (*val_reg as i64) * 8);
                    let v_val  = b.ins().load(types::I64, trusted(), v_addr, 0);

                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I64));

                    let callee = self.module.declare_function("xcx_jit_array_push", Linkage::Import, &sig).unwrap();
                    let local_callee = self.module.declare_func_in_func(callee, &mut b.func);
                    b.ins().call(local_callee, &[a_val, v_val]);
                }

                TraceOp::ArrayUpdate { arr_reg, idx_reg, val_reg, fail_ip } => {
                    let a_addr = b.ins().iadd_imm(locals_ptr, (*arr_reg as i64) * 8);
                    let a_val  = b.ins().load(types::I64, trusted(), a_addr, 0);
                    let i_addr = b.ins().iadd_imm(locals_ptr, (*idx_reg as i64) * 8);
                    let i_val  = b.ins().load(types::I64, trusted(), i_addr, 0);
                    let idx    = unpack_int!(i_val);
                    let v_addr = b.ins().iadd_imm(locals_ptr, (*val_reg as i64) * 8);
                    let v_val  = b.ins().load(types::I64, trusted(), v_addr, 0);

                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I64));
                    sig.returns.push(AbiParam::new(types::I32));

                    let callee = self.module.declare_function("xcx_jit_array_update", Linkage::Import, &sig).unwrap();
                    let local_callee = self.module.declare_func_in_func(callee, &mut b.func);
                    let call = b.ins().call(local_callee, &[a_val, idx, v_val]);
                    let ok_res = b.inst_results(call)[0];

                    let zero = b.ins().iconst(types::I32, 0);
                    let failed = b.ins().icmp(IntCC::Equal, ok_res, zero);

                    let fail = b.create_block();
                    let ok   = b.create_block();
                    b.ins().brif(failed, fail, no_args!(), ok, no_args!());

                    b.switch_to_block(fail); b.seal_block(fail);
                    let rv = b.ins().iconst(types::I32, *fail_ip as i64);
                    b.ins().return_(&[rv]);

                    b.switch_to_block(ok); b.seal_block(ok);
                }

                TraceOp::SetSize { dst, src } => {
                    let addr = b.ins().iadd_imm(locals_ptr, (*src as i64) * 8);
                    let val  = b.ins().load(types::I64, trusted(), addr, 0);

                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64));
                    sig.returns.push(AbiParam::new(types::I64));

                    let callee = self.module.declare_function("xcx_jit_set_size", Linkage::Import, &sig).unwrap();
                    let local_callee = self.module.declare_func_in_func(callee, &mut b.func);
                    let call = b.ins().call(local_callee, &[val]);
                    let res_i64 = b.inst_results(call)[0];
                    let boxed = pack_int!(res_i64);
                    let dst_addr = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), boxed, dst_addr, 0);
                }

                TraceOp::SetContains { dst, set_reg, val_reg } => {
                    let s_addr = b.ins().iadd_imm(locals_ptr, (*set_reg as i64) * 8);
                    let s_val  = b.ins().load(types::I64, trusted(), s_addr, 0);
                    let v_addr = b.ins().iadd_imm(locals_ptr, (*val_reg as i64) * 8);
                    let v_val  = b.ins().load(types::I64, trusted(), v_addr, 0);

                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64));
                    sig.params.push(AbiParam::new(types::I64));
                    sig.returns.push(AbiParam::new(types::I8));

                    let callee = self.module.declare_function("xcx_jit_set_contains", Linkage::Import, &sig).unwrap();
                    let local_callee = self.module.declare_func_in_func(callee, &mut b.func);
                    let call = b.ins().call(local_callee, &[s_val, v_val]);
                    let res = b.inst_results(call)[0];
                    let res64 = b.ins().uextend(types::I64, res);
                    let boxed = pack_bool!(res64);
                    let dst_addr = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), boxed, dst_addr, 0);
                }

                TraceOp::IncLocal { reg } => {
                    let la   = b.ins().iadd_imm(locals_ptr, (*reg as i64) * 8);
                    let lv   = b.ins().load(types::I64, trusted(), la, 0);
                    let i    = unpack_int!(lv);
                    let next = b.ins().iadd_imm(i, 1);
                    let boxed = pack_int!(next);
                    b.ins().store(trusted(), boxed, la, 0);
                }

                TraceOp::IncVar { g_idx } => {
                    let ga   = b.ins().iadd_imm(globals_ptr, (*g_idx as i64) * 8);
                    let gv   = b.ins().load(types::I64, trusted(), ga, 0);
                    let i    = unpack_int!(gv);
                    let gnxt = b.ins().iadd_imm(i, 1);
                    let boxed = pack_int!(gnxt);
                    b.ins().store(trusted(), boxed, ga, 0);
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
                    let ga   = b.ins().iadd_imm(globals_ptr, (*g_idx as i64) * 8);
                    let gv   = b.ins().load(types::I64, trusted(), ga, 0);
                    // Fast path increment for global
                    let gnxt_bits = b.ins().iadd_imm(gv, 1);
                    b.ins().store(trusted(), gnxt_bits, ga, 0);

                    let la   = b.ins().iadd_imm(locals_ptr, (*reg as i64) * 8);
                    let lv   = b.ins().load(types::I64, trusted(), la, 0);
                    // Fast path increment for local
                    let lnxt_bits = b.ins().iadd_imm(lv, 1);
                    b.ins().store(trusted(), lnxt_bits, la, 0);

                    let lim_a = b.ins().iadd_imm(locals_ptr, (*limit_reg as i64) * 8);
                    let lim_v = b.ins().load(types::I64, trusted(), lim_a, 0);
                    let lim_i = unpack_int!(lim_v);
                    let lnxt_unpacked = unpack_int!(lnxt_bits);

                    let cond = b.ins().icmp(IntCC::SignedLessThanOrEqual, lnxt_unpacked, lim_i);
                    emit_loop_exit!(cond, *target, *exit_ip);
                }

                TraceOp::IncLocalLoopNext { inc_reg, reg, limit_reg, target, exit_ip } => {

                    let ia   = b.ins().iadd_imm(locals_ptr, (*inc_reg as i64) * 8);
                    let iv   = b.ins().load(types::I64, trusted(), ia, 0);
                    // Fast path: increment packed bits directly
                    let inxt_bits = b.ins().iadd_imm(iv, 1);
                    b.ins().store(trusted(), inxt_bits, ia, 0);

                    let lnxt_unpacked = if *inc_reg != *reg {
                        let la    = b.ins().iadd_imm(locals_ptr, (*reg as i64) * 8);
                        let lv    = b.ins().load(types::I64, trusted(), la, 0);
                        let lnxt_bits = b.ins().iadd_imm(lv, 1);
                        b.ins().store(trusted(), lnxt_bits, la, 0);
                        unpack_int!(lnxt_bits)
                    } else {
                        unpack_int!(inxt_bits)
                    };

                    let lim_a = b.ins().iadd_imm(locals_ptr, (*limit_reg as i64) * 8);
                    let lim_v = b.ins().load(types::I64, trusted(), lim_a, 0);
                    let lim_i = unpack_int!(lim_v);

                    let cond = b.ins().icmp(IntCC::SignedLessThanOrEqual, lnxt_unpacked, lim_i);
                    emit_loop_exit!(cond, *target, *exit_ip);
                }

                TraceOp::LoopNextInt { reg, limit_reg, target, exit_ip } => {

                    let la    = b.ins().iadd_imm(locals_ptr, (*reg as i64) * 8);
                    let lv    = b.ins().load(types::I64, trusted(), la, 0);
                    // Fast path increment
                    let lnxt_bits = b.ins().iadd_imm(lv, 1);
                    b.ins().store(trusted(), lnxt_bits, la, 0);

                    let lim_a = b.ins().iadd_imm(locals_ptr, (*limit_reg as i64) * 8);
                    let lim_v = b.ins().load(types::I64, trusted(), lim_a, 0);
                    let lim_i = unpack_int!(lim_v);
                    let lnxt_unpacked = unpack_int!(lnxt_bits);

                    let cond = b.ins().icmp(IntCC::SignedLessThanOrEqual, lnxt_unpacked, lim_i);
                    emit_loop_exit!(cond, *target, *exit_ip);
                }

                _ => { /* unknown op — recording should have filtered this */ }
            }
        }

        if !current_terminated {
            if !entry_sealed {
                b.seal_block(entry_block);
            }
            if let Some(lh) = loop_header {
                b.ins().jump(lh, no_args!());
                b.seal_block(lh);
            }
            let rv = b.ins().iconst(types::I32, 0);
            b.ins().return_(&[rv]);
        } else if let Some(lh) = loop_header {
            b.seal_block(lh);
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
                let ir = self.ctx.func.display().to_string();
                if let Err(errs) = cranelift_codegen::verify_function(&self.ctx.func, self.module.isa()) {
                    let mut err_msg = format!("Verifier errors:\n{}\n", errs);
                    err_msg.push_str("IR:\n");
                    err_msg.push_str(&ir);
                    return Err(err_msg);
                }
                Err(format!("Cranelift define_function failed: {}\nIR:\n{}", e, ir))
            }
        }
    }

    pub fn compile_method(&mut self, func_id_idx: usize, chunk: &crate::backend::vm::FunctionChunk, constants: &[VMValue]) -> Result<*const u8, String> {
        self.module.clear_context(&mut self.ctx);

        let mut sig = self.module.make_signature();
        let ptr_type = self.module.target_config().pointer_type();
        sig.params.push(AbiParam::new(ptr_type)); // locals_ptr
        sig.params.push(AbiParam::new(ptr_type)); // globals_ptr
        sig.params.push(AbiParam::new(ptr_type)); // consts_ptr
        sig.params.push(AbiParam::new(ptr_type)); // vm_ptr
        sig.params.push(AbiParam::new(ptr_type)); // executor_ptr
        sig.returns.push(AbiParam::new(types::I64)); // Return Value bits

        let func_id = self.module
            .declare_function(
                &format!("func_{}", func_id_idx),
                Linkage::Export,
                &sig,
            )
            .map_err(|e: ModuleError| e.to_string())?;

        self.ctx.func.signature = sig;
        let mut b = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);

        let mut blocks = std::collections::HashMap::new();
        // Entry block
        let entry_block = b.create_block();
        blocks.insert(0, entry_block);

        // Pre-scan for jump targets
        for (_ip, op) in chunk.bytecode.iter().enumerate() {
            match op {
                crate::backend::vm::OpCode::Jump { target } => {
                    blocks.entry(*target as usize).or_insert_with(|| b.create_block());
                }
                crate::backend::vm::OpCode::JumpIfFalse { target, .. } |
                crate::backend::vm::OpCode::JumpIfTrue { target, .. } => {
                    blocks.entry(*target as usize).or_insert_with(|| b.create_block());
                    blocks.entry(_ip + 1).or_insert_with(|| b.create_block());
                }
                _ => {}
            }
        }

        b.append_block_params_for_function_params(entry_block);
        b.switch_to_block(entry_block);

        let locals_ptr  = b.block_params(entry_block)[0];
        let globals_ptr = b.block_params(entry_block)[1];
        let consts_ptr = b.block_params(entry_block)[2];
        let _vm_ptr = b.block_params(entry_block)[3];
        let _executor_ptr = b.block_params(entry_block)[4];

        let qnan_tag_int  = b.ins().iconst(types::I64, (QNAN_BASE | TAG_INT)  as i64);
        let qnan_tag_bool = b.ins().iconst(types::I64, (QNAN_BASE | TAG_BOOL) as i64);
        let mask_48       = b.ins().iconst(types::I64, 0x0000_FFFF_FFFF_FFFFu64 as i64);
        let one           = b.ins().iconst(types::I64, 1);
        let zero          = b.ins().iconst(types::I64, 0);

        let mut terminated = false;


        macro_rules! pack_int {
            ($val:expr) => {{
                let low = b.ins().band($val, mask_48);
                b.ins().bor(low, qnan_tag_int)
            }}
        }
        macro_rules! unpack_int {
            ($bits:expr) => {{
                let raw = b.ins().band($bits, mask_48);
                let shifted_left = b.ins().ishl_imm(raw, 16);
                b.ins().sshr_imm(shifted_left, 16)
            }}
        }

        for (ip, op) in chunk.bytecode.iter().enumerate() {
            if let Some(&block) = blocks.get(&ip) {
                if ip > 0 {
                    if !terminated {
                        b.ins().jump(block, &[]);
                    }
                    b.switch_to_block(block);
                    terminated = false;
                }
            }

            if terminated { continue; }

            match op {
                crate::backend::vm::OpCode::LoadConst { dst, idx } => {
                    let ca = b.ins().iadd_imm(consts_ptr, (*idx as i64) * 8);
                    let cv = b.ins().load(types::I64, trusted(), ca, 0);
                    let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    let old_v = b.ins().load(types::I64, trusted(), da, 0);

                    // Ref counting - Increment new value if it's a pointer (checked at compile time)
                    if constants[*idx as usize].is_ptr() {
                        let mut sig = self.module.make_signature();
                        sig.params.push(AbiParam::new(types::I64));
                        let callee_inc = self.module.declare_function("xcx_jit_inc_ref", Linkage::Import, &sig).unwrap();
                        let local_inc = self.module.declare_func_in_func(callee_inc, &mut b.func);
                        b.ins().call(local_inc, &[cv]);
                    }

                    // Ref counting - Decrement old value (inline pointer check for performance)
                    let mut sig_dec = self.module.make_signature();
                    sig_dec.params.push(AbiParam::new(types::I64));
                    let callee_dec = self.module.declare_function("xcx_jit_dec_ref", Linkage::Import, &sig_dec).unwrap();
                    let local_dec = self.module.declare_func_in_func(callee_dec, &mut b.func);

                    let tag_old = b.ins().ushr_imm(old_v, 48);
                    let ptr_tag_min = b.ins().iconst(types::I64, ((QNAN_BASE | TAG_STR) >> 48) as i64);
                    let is_ptr_old = b.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, tag_old, ptr_tag_min);
                    
                    let dec_block = b.create_block();
                    let ok_block = b.create_block();
                    b.ins().brif(is_ptr_old, dec_block, &[], ok_block, &[]);
                    
                    b.switch_to_block(dec_block); b.seal_block(dec_block);
                    b.ins().call(local_dec, &[old_v]);
                    b.ins().jump(ok_block, &[]);
                    
                    b.switch_to_block(ok_block); b.seal_block(ok_block);

                    b.ins().store(trusted(), cv, da, 0);
                }
                crate::backend::vm::OpCode::IncVar { idx } => {
                    let ga = b.ins().iadd_imm(globals_ptr, (*idx as i64) * 8);
                    let gv = b.ins().load(types::I64, trusted(), ga, 0);
                    let v = unpack_int!(gv);
                    let next = b.ins().iadd_imm(v, 1);
                    let next_v = pack_int!(next);
                    b.ins().store(trusted(), next_v, ga, 0);
                }
                crate::backend::vm::OpCode::GetVar { dst, idx } => {
                    let ga = b.ins().iadd_imm(globals_ptr, (*idx as i64) * 8);
                    let gv = b.ins().load(types::I64, trusted(), ga, 0);
                    let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    let old_v = b.ins().load(types::I64, trusted(), da, 0);

                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64));
                    let callee_inc = self.module.declare_function("xcx_jit_inc_ref", Linkage::Import, &sig).unwrap();
                    let local_inc = self.module.declare_func_in_func(callee_inc, &mut b.func);
                    b.ins().call(local_inc, &[gv]);
                    let callee_dec = self.module.declare_function("xcx_jit_dec_ref", Linkage::Import, &sig).unwrap();
                    let local_dec = self.module.declare_func_in_func(callee_dec, &mut b.func);
                    b.ins().call(local_dec, &[old_v]);

                    b.ins().store(trusted(), gv, da, 0);
                }
                crate::backend::vm::OpCode::SetVar { idx, src } => {
                    let sa = b.ins().iadd_imm(locals_ptr, (*src as i64) * 8);
                    let sv = b.ins().load(types::I64, trusted(), sa, 0);
                    let ga = b.ins().iadd_imm(globals_ptr, (*idx as i64) * 8);
                    let old_g = b.ins().load(types::I64, trusted(), ga, 0);

                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64));
                    let callee_inc = self.module.declare_function("xcx_jit_inc_ref", Linkage::Import, &sig).unwrap();
                    let local_inc = self.module.declare_func_in_func(callee_inc, &mut b.func);
                    b.ins().call(local_inc, &[sv]);
                    let callee_dec = self.module.declare_function("xcx_jit_dec_ref", Linkage::Import, &sig).unwrap();
                    let local_dec = self.module.declare_func_in_func(callee_dec, &mut b.func);
                    b.ins().call(local_dec, &[old_g]);

                    b.ins().store(trusted(), sv, ga, 0);
                }
                crate::backend::vm::OpCode::Move { dst, src } => {
                    if dst != src {
                        let sa = b.ins().iadd_imm(locals_ptr, (*src as i64) * 8);
                        let sv = b.ins().load(types::I64, trusted(), sa, 0);
                        let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                        let old_v = b.ins().load(types::I64, trusted(), da, 0);

                        // Ref counting signatures
                        let mut sig = self.module.make_signature();
                        sig.params.push(AbiParam::new(types::I64));
                        let callee_inc = self.module.declare_function("xcx_jit_inc_ref", Linkage::Import, &sig).unwrap();
                        let local_inc = self.module.declare_func_in_func(callee_inc, &mut b.func);
                        let callee_dec = self.module.declare_function("xcx_jit_dec_ref", Linkage::Import, &sig).unwrap();
                        let local_dec = self.module.declare_func_in_func(callee_dec, &mut b.func);

                        // Inline pointer checks for performance
                        let ptr_tag_min = b.ins().iconst(types::I64, ((QNAN_BASE | TAG_STR) >> 48) as i64);
                        
                        // Increment new value
                        let tag_new = b.ins().ushr_imm(sv, 48);
                        let is_ptr_new = b.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, tag_new, ptr_tag_min);
                        let inc_block = b.create_block();
                        let dec_check_block = b.create_block();
                        b.ins().brif(is_ptr_new, inc_block, &[], dec_check_block, &[]);
                        
                        b.switch_to_block(inc_block); b.seal_block(inc_block);
                        b.ins().call(local_inc, &[sv]);
                        b.ins().jump(dec_check_block, &[]);

                        // Decrement old value
                        b.switch_to_block(dec_check_block); b.seal_block(dec_check_block);
                        let tag_old = b.ins().ushr_imm(old_v, 48);
                        let is_ptr_old = b.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, tag_old, ptr_tag_min);
                        let dec_block = b.create_block();
                        let ok_block = b.create_block();
                        b.ins().brif(is_ptr_old, dec_block, &[], ok_block, &[]);
                        
                        b.switch_to_block(dec_block); b.seal_block(dec_block);
                        b.ins().call(local_dec, &[old_v]);
                        b.ins().jump(ok_block, &[]);
                        
                        b.switch_to_block(ok_block); b.seal_block(ok_block);

                        b.ins().store(trusted(), sv, da, 0);
                    }
                }
                crate::backend::vm::OpCode::Add { dst, src1, src2 } => {
                    let la = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    let ra = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let rv = b.ins().load(types::I64, trusted(), ra, 0);
                    let li = unpack_int!(lv);
                    let ri = unpack_int!(rv);
                    let res_i = b.ins().iadd(li, ri);
                    let res_v = pack_int!(res_i);
                    let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), res_v, da, 0);
                }
                crate::backend::vm::OpCode::LoopNext { reg, limit_reg, target } => {
                    let ra = b.ins().iadd_imm(locals_ptr, (*reg as i64) * 8);
                    let rv = b.ins().load(types::I64, trusted(), ra, 0);
                    let la = b.ins().iadd_imm(locals_ptr, (*limit_reg as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    let v = unpack_int!(rv);
                    let l = unpack_int!(lv);
                    let next = b.ins().iadd_imm(v, 1);
                    let next_v = pack_int!(next);
                    b.ins().store(trusted(), next_v, ra, 0);
                    let cond = b.ins().icmp(IntCC::SignedLessThanOrEqual, next, l);
                    let target_block = blocks[&(*target as usize)];
                    b.ins().brif(cond, target_block, &[], blocks[&(ip + 1)], &[]);
                    terminated = true;
                }
                crate::backend::vm::OpCode::IncLocalLoopNext { inc_reg, reg, limit_reg, target } => {
                    let ia = b.ins().iadd_imm(locals_ptr, (*inc_reg as i64) * 8);
                    let iv = b.ins().load(types::I64, trusted(), ia, 0);
                    let ii = unpack_int!(iv);
                    let next_i = b.ins().iadd_imm(ii, 1);
                    let next_iv = pack_int!(next_i);
                    b.ins().store(trusted(), next_iv, ia, 0);
                    let ra = b.ins().iadd_imm(locals_ptr, (*reg as i64) * 8);
                    let rv = b.ins().load(types::I64, trusted(), ra, 0);
                    let la = b.ins().iadd_imm(locals_ptr, (*limit_reg as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    let v = unpack_int!(rv);
                    let l = unpack_int!(lv);
                    let next = b.ins().iadd_imm(v, 1);
                    let next_v = pack_int!(next);
                    b.ins().store(trusted(), next_v, ra, 0);
                    let cond = b.ins().icmp(IntCC::SignedLessThanOrEqual, next, l);
                    let target_block = blocks[&(*target as usize)];
                    b.ins().brif(cond, target_block, &[], blocks[&(ip + 1)], &[]);
                    terminated = true;
                }
                crate::backend::vm::OpCode::IncVarLoopNext { g_idx, reg, limit_reg, target } => {
                    let ga = b.ins().iadd_imm(globals_ptr, (*g_idx as i64) * 8);
                    let gv = b.ins().load(types::I64, trusted(), ga, 0);
                    let gi = unpack_int!(gv);
                    let next_g = b.ins().iadd_imm(gi, 1);
                    let next_gv = pack_int!(next_g);
                    b.ins().store(trusted(), next_gv, ga, 0);
                    let ra = b.ins().iadd_imm(locals_ptr, (*reg as i64) * 8);
                    let rv = b.ins().load(types::I64, trusted(), ra, 0);
                    let la = b.ins().iadd_imm(locals_ptr, (*limit_reg as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    let v = unpack_int!(rv);
                    let l = unpack_int!(lv);
                    let next = b.ins().iadd_imm(v, 1);
                    let next_v = pack_int!(next);
                    b.ins().store(trusted(), next_v, ra, 0);
                    let cond = b.ins().icmp(IntCC::SignedLessThanOrEqual, next, l);
                    let target_block = blocks[&(*target as usize)];
                    b.ins().brif(cond, target_block, &[], blocks[&(ip + 1)], &[]);
                    terminated = true;
                }
                crate::backend::vm::OpCode::ArrayLoopNext { idx_reg, size_reg, target } => {
                    let ra = b.ins().iadd_imm(locals_ptr, (*idx_reg as i64) * 8);
                    let rv = b.ins().load(types::I64, trusted(), ra, 0);
                    let la = b.ins().iadd_imm(locals_ptr, (*size_reg as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    let v = unpack_int!(rv);
                    let l = unpack_int!(lv);
                    let next = b.ins().iadd_imm(v, 1);
                    let next_v = pack_int!(next);
                    b.ins().store(trusted(), next_v, ra, 0);
                    let cond = b.ins().icmp(IntCC::SignedLessThan, next, l);
                    let target_block = blocks[&(*target as usize)];
                    b.ins().brif(cond, target_block, &[], blocks[&(ip + 1)], &[]);
                    terminated = true;
                }
                crate::backend::vm::OpCode::Sub { dst, src1, src2 } => {
                    let la = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    let ra = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let rv = b.ins().load(types::I64, trusted(), ra, 0);
                    let li = unpack_int!(lv);
                    let ri = unpack_int!(rv);
                    let res_i = b.ins().isub(li, ri);
                    let res_v = pack_int!(res_i);
                    let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), res_v, da, 0);
                }
                crate::backend::vm::OpCode::Mul { dst, src1, src2 } => {
                    let la = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    let ra = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let rv = b.ins().load(types::I64, trusted(), ra, 0);
                    let li = unpack_int!(lv);
                    let ri = unpack_int!(rv);
                    let res_i = b.ins().imul(li, ri);
                    let res_v = pack_int!(res_i);
                    let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), res_v, da, 0);
                }
                crate::backend::vm::OpCode::Equal { dst, src1, src2 } => {
                    let la = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    let ra = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let rv = b.ins().load(types::I64, trusted(), ra, 0);
                    let li = unpack_int!(lv);
                    let ri = unpack_int!(rv);
                    let cond = b.ins().icmp(IntCC::Equal, li, ri);
                    let res_i = b.ins().select(cond, one, zero);
                    let res_v = b.ins().bor(res_i, qnan_tag_bool);
                    let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), res_v, da, 0);
                }
                crate::backend::vm::OpCode::NotEqual { dst, src1, src2 } => {
                    let la = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    let ra = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let rv = b.ins().load(types::I64, trusted(), ra, 0);
                    let li = unpack_int!(lv);
                    let ri = unpack_int!(rv);
                    let cond = b.ins().icmp(IntCC::NotEqual, li, ri);
                    let res_i = b.ins().select(cond, one, zero);
                    let res_v = b.ins().bor(res_i, qnan_tag_bool);
                    let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), res_v, da, 0);
                }
                crate::backend::vm::OpCode::Greater { dst, src1, src2 } => {
                    let la = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    let ra = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let rv = b.ins().load(types::I64, trusted(), ra, 0);
                    let li = unpack_int!(lv);
                    let ri = unpack_int!(rv);
                    let cond = b.ins().icmp(IntCC::SignedGreaterThan, li, ri);
                    let res_i = b.ins().select(cond, one, zero);
                    let res_v = b.ins().bor(res_i, qnan_tag_bool);
                    let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), res_v, da, 0);
                }
                crate::backend::vm::OpCode::Less { dst, src1, src2 } => {
                    let la = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    let ra = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let rv = b.ins().load(types::I64, trusted(), ra, 0);
                    let li = unpack_int!(lv);
                    let ri = unpack_int!(rv);
                    let cond = b.ins().icmp(IntCC::SignedLessThan, li, ri);
                    let res_i = b.ins().select(cond, one, zero);
                    let res_v = b.ins().bor(res_i, qnan_tag_bool);
                    let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), res_v, da, 0);
                }
                crate::backend::vm::OpCode::GreaterEqual { dst, src1, src2 } => {
                    let la = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    let ra = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let rv = b.ins().load(types::I64, trusted(), ra, 0);
                    let li = unpack_int!(lv);
                    let ri = unpack_int!(rv);
                    let cond = b.ins().icmp(IntCC::SignedGreaterThanOrEqual, li, ri);
                    let res_i = b.ins().select(cond, one, zero);
                    let res_v = b.ins().bor(res_i, qnan_tag_bool);
                    let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), res_v, da, 0);
                }
                crate::backend::vm::OpCode::LessEqual { dst, src1, src2 } => {
                    let la = b.ins().iadd_imm(locals_ptr, (*src1 as i64) * 8);
                    let lv = b.ins().load(types::I64, trusted(), la, 0);
                    let ra = b.ins().iadd_imm(locals_ptr, (*src2 as i64) * 8);
                    let rv = b.ins().load(types::I64, trusted(), ra, 0);
                    let li = unpack_int!(lv);
                    let ri = unpack_int!(rv);
                    let cond = b.ins().icmp(IntCC::SignedLessThanOrEqual, li, ri);
                    let res_i = b.ins().select(cond, one, zero);
                    let res_v = b.ins().bor(res_i, qnan_tag_bool);
                    let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                    b.ins().store(trusted(), res_v, da, 0);
                }
                crate::backend::vm::OpCode::JumpIfFalse { src, target } => {
                    let sa = b.ins().iadd_imm(locals_ptr, (*src as i64) * 8);
                    let sv = b.ins().load(types::I64, trusted(), sa, 0);
                    let val = b.ins().band(sv, one);
                    let cond = b.ins().icmp_imm(IntCC::Equal, val, 0);
                    let target_block = blocks[&(*target as usize)];
                    b.ins().brif(cond, target_block, &[], blocks[&(ip + 1)], &[]);
                    terminated = true;
                }
                crate::backend::vm::OpCode::JumpIfTrue { src, target } => {
                    let sa = b.ins().iadd_imm(locals_ptr, (*src as i64) * 8);
                    let sv = b.ins().load(types::I64, trusted(), sa, 0);
                    let val = b.ins().band(sv, one);
                    let cond = b.ins().icmp_imm(IntCC::NotEqual, val, 0);
                    let target_block = blocks[&(*target as usize)];
                    b.ins().brif(cond, target_block, &[], blocks[&(ip + 1)], &[]);
                    terminated = true;
                }
                crate::backend::vm::OpCode::Jump { target } => {
                    let target_block = blocks[&(*target as usize)];
                    b.ins().jump(target_block, &[]);
                    terminated = true;
                }
                crate::backend::vm::OpCode::MethodCall { dst, kind, base, arg_count } => {
                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I8));  // dst
                    sig.params.push(AbiParam::new(types::I8));  // kind
                    sig.params.push(AbiParam::new(types::I64)); // receiver
                    sig.params.push(AbiParam::new(ptr_type));   // args_ptr
                    sig.params.push(AbiParam::new(types::I8));  // arg_count
                    sig.params.push(AbiParam::new(ptr_type));   // locals_ptr
                    sig.params.push(AbiParam::new(ptr_type));   // executor_ptr

                    let callee = self.module.declare_function("xcx_jit_method_dispatch", Linkage::Import, &sig).unwrap();
                    let local_callee = self.module.declare_func_in_func(callee, &mut b.func);
                    
                    let dst_v = b.ins().iconst(types::I8, *dst as i64);
                    let kind_v = b.ins().iconst(types::I8, *kind as i64);
                    
                    let ra = b.ins().iadd_imm(locals_ptr, (*base as i64) * 8);
                    let rv = b.ins().load(types::I64, trusted(), ra, 0);
                    
                    let args_p = b.ins().iadd_imm(locals_ptr, ((*base as i64) + 1) * 8);
                    let acount_v = b.ins().iconst(types::I8, *arg_count as i64);
                    
                    b.ins().call(local_callee, &[dst_v, kind_v, rv, args_p, acount_v, locals_ptr, _executor_ptr]);
                }
                crate::backend::vm::OpCode::Call { dst, func_idx, base, arg_count } => {
                    if (*func_idx as usize) == func_id_idx {
                        let params_ptr = b.ins().iadd_imm(locals_ptr, (*base as i64) * 8);
                        
                        let mut sig = self.module.make_signature();
                        sig.params.push(AbiParam::new(types::I64)); // func_id_idx
                        sig.params.push(AbiParam::new(ptr_type));   // params_ptr
                        sig.params.push(AbiParam::new(types::I8));    // params_count
                        sig.params.push(AbiParam::new(ptr_type));   // vm_ptr
                        sig.params.push(AbiParam::new(ptr_type));   // executor_ptr
                        sig.params.push(AbiParam::new(ptr_type));   // globals_ptr
                        sig.returns.push(AbiParam::new(types::I64)); // result bits
                        
                        let callee = self.module.declare_function("xcx_jit_call_recursive", Linkage::Import, &sig).unwrap();
                        let local_callee = self.module.declare_func_in_func(callee, &mut b.func);
                        
                        let fid_v = b.ins().iconst(types::I64, func_id_idx as i64);
                        let pcount_v = b.ins().iconst(types::I8, *arg_count as i64);
                        
                        let call_inst = b.ins().call(local_callee, &[fid_v, params_ptr, pcount_v, _vm_ptr, _executor_ptr, globals_ptr]);
                        let res_v = b.inst_results(call_inst)[0];
                        let da = b.ins().iadd_imm(locals_ptr, (*dst as i64) * 8);
                        b.ins().store(trusted(), res_v, da, 0);
                    } else {
                    }
                }
                crate::backend::vm::OpCode::Return { src } => {
                    let sa = b.ins().iadd_imm(locals_ptr, (*src as i64) * 8);
                    let sv = b.ins().load(types::I64, trusted(), sa, 0);
                    b.ins().return_(&[sv]);
                    terminated = true;
                }
                crate::backend::vm::OpCode::ReturnVoid => {
                    let zero_val = b.ins().iconst(types::I64, (QNAN_BASE | TAG_BOOL) as i64); // false
                    b.ins().return_(&[zero_val]);
                    terminated = true;
                }
                _ => {
                    let zero_val = b.ins().iconst(types::I64, 0);
                    b.ins().return_(&[zero_val]);
                    terminated = true;
                }
            }
        }

        for (_, &block) in &blocks {
            b.seal_block(block);
        }

        b.finalize();

        match self.module.define_function(func_id, &mut self.ctx) {
            Ok(_) => {
                self.module.finalize_definitions().unwrap();
                let code = self.module.get_finalized_function(func_id);
                Ok(code)
            }
            Err(e) => {
                let ir = self.ctx.func.display().to_string();
                if let Err(errs) = cranelift_codegen::verify_function(&self.ctx.func, self.module.isa()) {
                    let mut err_msg = format!("Verifier errors:\n{}\n", errs);
                    err_msg.push_str("IR:\n");
                    err_msg.push_str(&ir);
                    return Err(err_msg);
                }
                Err(format!("Cranelift define_function failed: {}\nIR:\n{}", e, ir))
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