# XCX Tracing JIT (Cranelift) — Documentation

> **File:** `src/backend/jit.rs`  
> XCX 3.0 includes a **dual-mode JIT** that automatically compiles both hot loops and frequently-called functions into native machine code using the [Cranelift](https://github.com/bytecodealliance/wasmtime/tree/main/cranelift) framework.

---

## Table of Contents

1. [Overview](#overview)
2. [Two JIT Modes](#two-jit-modes)
3. [Tracing JIT — Hot Loop Detection](#tracing-jit--hot-loop-detection)
4. [Trace Recording](#trace-recording)
5. [Trace Compilation](#trace-compilation)
6. [Trace Execution](#trace-execution)
7. [Method JIT — Function Compilation](#method-jit--function-compilation)
8. [TraceOp Variants](#traceop-variants)
9. [Exported C Functions](#exported-c-functions)
10. [JIT Configuration](#jit-configuration)

---

## Overview

The JIT is **completely transparent** to the developer — it activates automatically and falls back to the interpreter upon hitting type guards or unsupported operations.

There are two independent compilation paths:

```
[Tracing JIT] — for hot loops
  Interpreter executes loop
      ↓ (50 iterations)
  Trace recording starts
      ↓ (loop returns to start IP)
  Trace compiled by Cranelift → native C function (3-arg signature)
      ↓ (next iteration)
  Native code executed directly (interpreter bypass)
      ↓ (failed guard)
  Fallback to interpreter at the correct IP

[Method JIT] — for frequently-called functions without loops
  Function is called
      ↓ (10 calls)
  Entire function bytecode compiled by Cranelift → native C function (5-arg signature)
      ↓ (next call)
  Native code called directly (bypasses interpreter + tracing machinery)
```

---

## Two JIT Modes

### JITFunction (Tracing)

```rust
pub type JITFunction = unsafe extern "C" fn(
    *mut VMValue,   // locals_ptr
    *mut VMValue,   // globals_ptr
    *const VMValue, // consts_ptr
) -> i32            // next_ip (0 = continue, >0 = side-exit IP, <0 = halt)
```

Used for hot loop traces. The return value is the next instruction pointer.

### MethodJitFunction (Method)

```rust
pub type MethodJitFunction = unsafe extern "C" fn(
    *mut VMValue,   // locals_ptr
    *mut VMValue,   // globals_ptr
    *const VMValue, // consts_ptr
    *mut VM,        // vm_ptr
    *mut Executor,  // executor_ptr
) -> u64            // return value as raw Value bits
```

Used for compiled full functions. Returns the function's return value directly as NaN-boxed bits. Needs access to the VM and Executor for recursive calls and method dispatch.

---

## Tracing JIT — Hot Loop Detection

On every `Jump { target }` where `target < current_ip` (backward jump, i.e., loop return edge):

1. `hot_counts[target]` is incremented
2. When `hot_counts[target] >= 50`, a new `Trace` is started with `start_ip = target`
3. Trace recording is guarded: starts only if `trace_cache[target].is_none()` (no compiled trace) and `is_recording` is false

```rust
fn check_start_recording(&mut self, target_ip: usize, threshold: usize) {
    if target_ip < self.hot_counts.len() {
        let hc = unsafe { self.hot_counts.get_unchecked_mut(target_ip) };
        *hc += 1;
        if *hc >= threshold && !self.is_recording && self.trace_cache[target_ip].is_none() {
            self.recording_trace = Some(Trace { ops: vec![], start_ip: target_ip, ... });
            self.is_recording = true;
        }
    }
}
```

The threshold is **50** for the main `Jump` opcode and **1000** for the fused loop opcodes (`LoopNext`, `IncVarLoopNext`, `IncLocalLoopNext`).

---

## Trace Recording

When `is_recording` is true, the interpreter executes each opcode normally **and** records a `TraceOp` specialized for the current runtime types. Examples:

- `Add` on two integer registers records `GuardInt { reg: src1 }`, `GuardInt { reg: src2 }`, `AddInt { dst, src1, src2 }` — not a generic `Add`
- `JumpIfFalse` that is **not** taken records `GuardTrue { reg: src, fail_ip: target }` — asserting that the branch is always false
- `JumpIfFalse` that **is** taken records `GuardFalse { reg: src, fail_ip: next_ip }`

If an opcode cannot be traced (complex method calls, string operations, etc.), recording is aborted and `is_recording` is set back to false.

### What is Traceable?

| Traceable | Not Traceable |
|---|---|
| Int/Float Arithmetic | Object methods (except ArraySize, ArrayGet, ArrayPush, ArrayUpdate, SetSize, SetContains) |
| Int/Float Comparisons | String operations |
| Boolean logic | Function calls |
| Global access/modification | I/O, HTTP |
| Counter increments | Fiber operations |
| Array operations: size, get, push, update | |
| Set operations: size, contains | |
| RandomInt, RandomFloat, RandomChoice | |
| Pow, IntConcat, Has | |
| CastIntToFloat | |

---

## Trace Compilation

When the trace returns to `start_ip`, the complete `Trace` is passed to `JIT::compile()` in Cranelift. Cranelift compiles the `TraceOp` sequence into a native function with the `JITFunction` signature (3 params, `i32` return).

The compiled function pointer is stored in `Trace::native_ptr` (`AtomicPtr<u8>`) and the `Arc<Trace>` is inserted into both `vm.traces` (globally shared) and `trace_cache` (fast path per-executor).

### Cranelift Compiler Settings

```rust
flag_builder.set("opt_level", "speed").unwrap();
flag_builder.set("use_colocated_libcalls", "false").unwrap();
flag_builder.set("is_pic", "false").unwrap();
flag_builder.set("regalloc_checker", "false").unwrap();
```

---

## Trace Execution

On every loop dispatch iteration, before fetching the next opcode:

```rust
if !self.is_recording && current_ip < self.trace_cache.len() {
    if let Some(trace) = &self.trace_cache[current_ip] {
        let jit_res = self.execute_trace(trace, ip, locals, glbs.as_mut().unwrap());
        if let Some(res) = jit_res { return res; }
        continue;
    }
}
```

If a compiled native function is available (`native_ptr != null`), it is called directly via `transmute` — completely bypassing the interpreter for the entire loop body. If the JIT hasn't compiled it yet, the interpreted `TraceOp` path is used as an intermediate step.

---

## Method JIT — Function Compilation

Functions **without loops** (`has_loops = false`) and with fewer than 500 bytecode instructions are eligible for Method JIT compilation.

### Trigger

After **10 calls** to the same function, the JIT compilation is triggered asynchronously:

```rust
let count = chunk.call_count.fetch_add(1, Ordering::Relaxed);
if count == 10 {
    let mut jit = vm_copy.jit.lock();
    match jit.compile_method(func_id_copy, &chunk_copy, &self.ctx.constants) {
        Ok(ptr) => {
            chunk_copy.jit_ptr.store(ptr as *mut u8, Ordering::Release);
        }
        Err(_) => {}
    }
}
```

The compiled pointer is stored in `FunctionChunk::jit_ptr` (`Arc<AtomicPtr<u8>>`), shared across all threads.

### Fast Path Execution

On every `run_frame_with_guard` call, the JIT pointer is checked first:

```rust
let jit_ptr = chunk.jit_ptr.load(Ordering::Relaxed);
if !jit_ptr.is_null() && !self.is_recording {
    let jit_fn: MethodJitFunction = unsafe { std::mem::transmute(jit_ptr) };
    // Prepare locals from params, call native function directly
    let res_bits = unsafe { jit_fn(locals.as_mut_ptr(), glbs_ptr, consts.as_ptr(), vm_ptr, executor_ptr) };
    return Some(Value(res_bits));
}
```

This bypasses the entire interpreter loop, tracing machinery, and hot_counts/trace_cache allocation for compiled functions.

### `compile_method` — Full Bytecode Compilation

`JIT::compile_method` compiles a complete `FunctionChunk` to native code. It handles a larger subset of opcodes than trace compilation:

- All arithmetic, comparison, and logical opcodes (integer only)
- `LoadConst`, `Move`, `GetVar`, `SetVar` with full reference counting
- Control flow: `Jump`, `JumpIfFalse`, `JumpIfTrue`, `Return`, `ReturnVoid`
- Loop opcodes: `LoopNext`, `IncLocalLoopNext`, `IncVarLoopNext`, `ArrayLoopNext`
- `MethodCall` — dispatched via `xcx_jit_method_dispatch` extern C function
- `Call` — recursive calls via `xcx_jit_call_recursive` (only self-recursive calls are supported inline; other function calls fall through to the interpreter path)

Unsupported opcodes cause an early `return` with a zero value (false), effectively falling back gracefully.

### Reference Counting in Method JIT

Unlike the Tracing JIT (which skips ref-counting for speed), the Method JIT includes full `inc_ref`/`dec_ref` calls via the `xcx_jit_inc_ref` and `xcx_jit_dec_ref` extern C functions. This is required because compiled functions may hold and release heap-allocated values across their lifetime.

Pointer checks are done inline by inspecting the tag bits before calling ref-count helpers, avoiding unnecessary function call overhead for scalar values.

---

## Fast-Path Pooling

The executor maintains three pools to avoid O(N) allocations in deeply recursive or frequently-called functions:

```rust
hot_counts_pool:   Vec<Vec<usize>>,
trace_cache_pool:  Vec<Vec<Option<Arc<Trace>>>>,
locals_pool:       Vec<Vec<Value>>,
```

When entering `run_frame_with_guard`, vectors are retrieved from the pool (or freshly allocated). When returning, they are returned to the pool. This eliminates most per-call heap allocations in hot code paths.

The `locals_pool` is also used by `xcx_jit_call_recursive` for the fast path of self-recursive compiled functions.

---

## TraceOp Variants

### Data Movement

| Variant | Description |
|---|-----
| `LoadConst { dst, val }` | Load constant into register |
| `Move { dst, src }` | Copy register |
| `GetVar { dst, idx }` | Load global |
| `SetVar { idx, src }` | Store global |

### Integer Arithmetic

| Variant | Description |
|---|---|
| `AddInt / SubInt / MulInt` | Basic operations (wrapping) |
| `DivInt / ModInt` | With `fail_ip` for division by zero / overflow |
| `PowInt` | Exponentiation via extern C function |
| `IntConcat` | Digit concatenation (123 ++ 456 = 123456) |

### Float Arithmetic

| Variant | Description |
|---|---|
| `AddFloat / SubFloat / MulFloat` | Basic operations |
| `DivFloat / ModFloat` | With `fail_ip` for division by zero |
| `PowFloat` | Exponentiation via extern C function |
| `CastIntToFloat` | int → float conversion |

### Type Guards

| Variant | Description |
|---|---|
| `GuardInt { reg, ip }` | Side-exit to `ip` if register is not Int |
| `GuardFloat { reg, ip }` | Side-exit to `ip` if register is not Float |
| `GuardTrue { reg, fail_ip }` | Side-exit if boolean value is false |
| `GuardFalse { reg, fail_ip }` | Side-exit if boolean value is true |

### Comparisons

| Variant | Description |
|---|---|
| `CmpInt { dst, src1, src2, cc }` | Integer comparison with condition code |
| `CmpFloat { dst, src1, src2, cc }` | Float comparison with condition code |

**Condition Codes (cc):**

| cc | IntCC | FloatCC |
|---|---|---|
| 0 | Equal | Equal |
| 1 | NotEqual | NotEqual |
| 2 | SignedGreaterThan | GreaterThan |
| 3 | SignedLessThan | LessThan |
| 4 | SignedGreaterThanOrEqual | GreaterThanOrEqual |
| 5 | SignedLessThanOrEqual | LessThanOrEqual |

### Loop Control

| Variant | Description |
|---|---|
| `LoopNextInt { reg, limit_reg, target, exit_ip }` | Increment + conditional jump for range loops |
| `IncVarLoopNext { g_idx, reg, limit_reg, target, exit_ip }` | Global counters + range loop |
| `IncLocalLoopNext { inc_reg, reg, limit_reg, target, exit_ip }` | Local counters + range loop |
| `IncLocal { reg }` | Simple local variable increment |
| `IncVar { g_idx }` | Simple global variable increment |
| `Jump { target_ip }` | Unconditional jump (triggers loop return detection) |

### Collections

| Variant | Description |
|---|---|
| `ArraySize { dst, src }` | Get array size |
| `ArrayGet { dst, arr_reg, idx_reg, fail_ip }` | Get element (with bounds check) |
| `ArrayPush { arr_reg, val_reg }` | Add element to array |
| `ArrayUpdate { arr_reg, idx_reg, val_reg, fail_ip }` | Update element at index (with bounds check) |
| `SetSize { dst, src }` | Get set size |
| `SetContains { dst, set_reg, val_reg }` | Check set membership |

### Randomness

| Variant | Description |
|---|---|
| `RandomInt { dst, min, max, step, has_step }` | Random integer with range/step |
| `RandomFloat { dst, min, max, step, has_step, step_is_float }` | Random float with range/step |
| `RandomChoice { dst, src }` | Random element from collection |

### Logic

`And / Or / Not` — operations on boolean bits

---

## Exported C Functions

The JIT calls external Rust functions via C ABI for operations that cannot be trivially inlined. All are registered in `JITBuilder` during JIT initialization.

### Arithmetic and Math

```rust
#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_random_int(min: i64, max: i64, step: i64, has_step: bool) -> i64

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_random_float(min: f64, max: f64, step: f64, has_step: bool) -> f64

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_pow_int(a: i64, b: i64) -> i64

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_pow_float(a: f64, b: f64) -> f64

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_int_concat(a: i64, b: i64) -> i64

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_has(container: Value, item: Value) -> bool

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_random_choice(col: Value) -> Value
```

### Collection Operations

```rust
#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_array_size(arr: Value) -> i64

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_array_get(arr: Value, idx: i64) -> Value

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_array_push(arr: Value, val: Value)

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_array_update(arr: Value, idx: i64, val: Value) -> i32
// Returns 1 on success, 0 on out-of-bounds (triggers JIT side-exit)

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_set_size(set: Value) -> i64

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_set_contains(set: Value, val: Value) -> bool
```

### Reference Counting

```rust
#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_inc_ref(v: Value)
// Increments Arc ref count if value is a pointer type

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_dec_ref(v: Value)
// Decrements Arc ref count if value is a pointer type; may free memory
```

Used exclusively by the Method JIT to manage heap-allocated value lifetimes. The Tracing JIT skips ref-counting for maximum loop performance.

### Method Dispatch and Recursive Calls

```rust
#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_method_dispatch(
    dst: u8,
    kind: u8,           // MethodKind cast to u8
    receiver: Value,
    args_ptr: *const Value,
    arg_count: u8,
    locals_ptr: *mut Value,
    executor_ptr: *mut Executor,
)
// Dispatches a built-in method call from within a compiled function.
// Delegates to Executor::handle_method_call or handle_database_method.

#[unsafe(no_mangle)]
pub extern "C" fn xcx_jit_call_recursive(
    func_id_idx: usize,
    params_ptr: *const Value,
    params_count: u8,
    vm_ptr: *const VM,
    executor_ptr: *mut Executor,
    globals_ptr: *mut Value,
) -> u64
// Calls a function from within a compiled function.
// Fast path: if the target function is also JIT-compiled, calls it directly
//            using locals from the executor's pool (avoids locking).
// Slow path: falls back to run_frame_with_guard for uncompiled functions.
```

---

## JIT Configuration

```rust
pub struct JIT {
    builder_context: FunctionBuilderContext,
    pub ctx:         codegen::Context,
    module:          JITModule,
}
```

Initialization:
1. Creates `JITBuilder` with native ISA (host auto-detection via `cranelift_native`)
2. Registers all external C symbols (arithmetic, collections, ref-counting, dispatch)
3. Sets `opt_level: "speed"` for maximum performance
4. Creates `JITModule` managing executable memory

### Trace compilation (`compile`)

1. Clears context (`module.clear_context`)
2. Declares function with `JITFunction` signature: `(locals, globals, consts) -> i32`
3. Detects loop presence (`has_loop`) — if a `LoopNextInt`, `IncVarLoopNext`, etc. is present, wraps ops in a Cranelift loop block
4. Builds Cranelift IR for each `TraceOp`
5. Compiles and finalizes definitions
6. Returns code pointer (`get_finalized_function`)

### Method compilation (`compile_method`)

1. Clears context (`module.clear_context`)
2. Declares function with `MethodJitFunction` signature: `(locals, globals, consts, vm, executor) -> u64`
3. Pre-scans bytecode for all jump targets to create the correct number of Cranelift blocks
4. Builds Cranelift IR for each supported `OpCode`
5. Seals all blocks and finalizes
6. Returns code pointer; stored in `FunctionChunk::jit_ptr`

---

## NaN-boxing Macros in JIT

The JIT uses macros to work with NaN-boxed values in Cranelift IR:

```rust
// Unpack integer (sign-extend from 48 bits)
macro_rules! unpack_int {
    ($val:expr) => {{
        let shl = b.ins().ishl_imm($val, 16);
        b.ins().sshr_imm(shl, 16)
    }};
}

// Pack integer
macro_rules! pack_int {
    ($raw:expr) => {{
        let lo = b.ins().band($raw, mask_48);
        b.ins().bor(qnan_tag_int, lo)
    }};
}

// Optimization: For NaN-boxed Int, increment is a simple iadd_imm(val, 1)
// on the entire 64-bit word — tag bits remain unaffected!
let lnxt_bits = b.ins().iadd_imm(lv, 1);
b.ins().store(trusted(), lnxt_bits, la, 0);
```

This optimization works because NaN tags are in the high bits, and the Integer value occupies the lower 48 bits — adding 1 to the whole word only changes the value part (as long as there is no overflow in the 48-bit domain). The same fast-path increment is used for both local and global variables in all fused loop opcodes.