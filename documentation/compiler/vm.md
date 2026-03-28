//==compiler/vm.md==\\
# XCX Virtual Machine (VM) — v2.2

The XCX VM is a custom **register-based** runtime for executing XCX bytecode, augmented by a tracing JIT compiler built on Cranelift.

## Architecture

- **File**: `src/backend/vm.rs`
- **Execution Model**: Fetch-Decode-Execute loop (`execute_bytecode`) over a flat register file
- **Register File**: A `Vec<Value>` owned per frame, indexed by `u8` slot numbers
- **Globals**: A single flat `Vec<Value>` behind `Arc<RwLock<Vec<Value>>>`, shared across all worker threads
- **JIT**: `src/backend/jit.rs` — Cranelift-based native code compiler for hot traces

### VM State

```rust
pub struct VM {
    pub globals:     Arc<RwLock<Vec<Value>>>,
    pub error_count: AtomicUsize,
    pub traces:      Arc<RwLock<HashMap<usize, Arc<Trace>>>>,
    pub jit:         Mutex<JIT>,
}
```

`VM` is wrapped in `Arc<VM>` and shared across HTTP worker threads. Each worker creates its own `Executor` with private locals.

### Executor State

```rust
struct Executor {
    vm:               Arc<VM>,
    ctx:              SharedContext,
    current_spans:    Option<Arc<Vec<Span>>>,
    fiber_yielded:    bool,
    hot_counts:       Vec<usize>,         // per-IP backward-jump counter
    recording_trace:  Option<Trace>,      // trace being recorded
    is_recording:     bool,
    trace_cache:      Vec<Option<Arc<Trace>>>, // compiled traces indexed by start IP
    http_req:         Option<Arc<Mutex<Option<tiny_http::Request>>>>,
    http_req_val:     Option<Value>,
}
```

### SharedContext

```rust
pub struct SharedContext {
    pub constants: Arc<Vec<Value>>,
    pub functions: Arc<Vec<FunctionChunk>>,
}
```

`SharedContext` is cheaply cloned (two `Arc` pointer bumps) and passed to each worker thread independently. No deep copy occurs.

---

## Value Representation: NaN-Boxing

Every value is a single `Value(u64)` — a 64-bit word. XCX uses **NaN-boxing**: the IEEE 754 quiet NaN bit pattern is repurposed as a type tag prefix.

```
Bit layout: [63..52: exponent/QNAN] [51..48: type tag] [47..0: payload]

Float : stored directly as f64 bits — does NOT have QNAN_BASE prefix set
Int   : QNAN_BASE | TAG_INT  | (i48 value & 0x0000_FFFF_FFFF_FFFF)
Bool  : QNAN_BASE | TAG_BOOL | (0 or 1)
Date  : QNAN_BASE | TAG_DATE | (i48 timestamp ms)
Ptr   : QNAN_BASE | TAG_XXX  | (pointer & 0x0000_FFFF_FFFF_FFFF)
```

### Tag Constants

| Constant | Value | Type |
|---|---|---|
| `QNAN_BASE` | `0x7FF0_0000_0000_0000` | base NaN marker |
| `TAG_INT`   | `0x0001_0000_0000_0000` | 48-bit signed integer |
| `TAG_BOOL`  | `0x0002_0000_0000_0000` | boolean (payload 0/1) |
| `TAG_DATE`  | `0x0003_0000_0000_0000` | 48-bit timestamp (ms) |
| `TAG_STR`   | `0x0004_0000_0000_0000` | `Arc<String>` pointer |
| `TAG_ARR`   | `0x0005_0000_0000_0000` | `Arc<RwLock<Vec<Value>>>` pointer |
| `TAG_SET`   | `0x0006_0000_0000_0000` | `Arc<RwLock<SetData>>` pointer |
| `TAG_MAP`   | `0x0007_0000_0000_0000` | `Arc<RwLock<Vec<(Value,Value)>>>` pointer |
| `TAG_TBL`   | `0x0008_0000_0000_0000` | `Arc<RwLock<TableData>>` pointer |
| `TAG_FUNC`  | `0x0009_0000_0000_0000` | function index (u32) |
| `TAG_ROW`   | `0x000A_0000_0000_0000` | `Arc<RowRef>` pointer |
| `TAG_JSON`  | `0x000B_0000_0000_0000` | `Arc<RwLock<serde_json::Value>>` pointer |
| `TAG_FIB`   | `0x000C_0000_0000_0000` | `Arc<RwLock<FiberState>>` pointer |

Pointer payloads use only the low 48 bits — valid on all x86-64 and AArch64 platforms where user-space pointers fit in 48 bits.

### Reference Counting for Pointer Values

Pointer-tagged values carry `Arc` reference counts. The VM manages these manually via `inc_ref()` / `dec_ref()` on every assignment, return, and collection modification — ensuring that heap-allocated objects (strings, arrays, JSON, fibers, etc.) are freed when no longer referenced, without a garbage collector.

---

## Instruction Set (OpCodes)

All opcodes are register-based: they reference named `u8` register slots rather than an operand stack.

### Register / Variable Movement

| OpCode | Description |
|---|---|
| `LoadConst { dst, idx }` | Load `constants[idx]` into register `dst` |
| `Move { dst, src }` | Copy register `src` into `dst` |
| `GetVar { dst, idx }` | Load `globals[idx]` into `dst` (read-locks globals) |
| `SetVar { idx, src }` | Write `src` into `globals[idx]` (write-locks globals) |

### Arithmetic

All arithmetic ops are 3-register: `dst = src1 OP src2`. Runtime type dispatch selects integer, float, string-concat, date-arithmetic, or set-operation paths.

`Add`, `Sub`, `Mul`, `Div`, `Mod`, `Pow`, `IntConcat` (`++`)

### Comparison (result is Bool)

`Equal`, `NotEqual`, `Greater`, `Less`, `GreaterEqual`, `LessEqual`

### Logic

`And { dst, src1, src2 }`, `Or { dst, src1, src2 }`, `Not { dst, src }`, `Has { dst, src1, src2 }`

### Control Flow

| OpCode | Description |
|---|---|
| `Jump { target }` | Unconditional jump; increments `hot_counts[target]` on backward jumps |
| `JumpIfFalse { src, target }` | Jump if `src` is `Bool(false)` |
| `JumpIfTrue { src, target }` | Jump if `src` is `Bool(true)` |
| `Call { dst, func_idx, base, arg_count }` | Call function; args are `locals[base..base+arg_count]`; result → `dst` |
| `Return { src }` | Return value in `src` from current frame |
| `ReturnVoid` | Return without a value |
| `Halt` | Stop execution (explicit or on unrecoverable error) |

### Collections

| OpCode | Description |
|---|---|
| `ArrayInit { dst, base, count }` | Collect `count` registers starting at `base` → new Array in `dst` |
| `SetInit { dst, base, count }` | Collect `count` registers → new Set in `dst` |
| `SetRange { dst, start, end, step, has_step }` | Build ranged Set from register values |
| `MapInit { dst, base, count }` | Collect `count` key-value pairs (registers in alternating pairs from `base`) → new Map |
| `TableInit { dst, skeleton_idx, base, row_count }` | Build Table from column-schema constant + row values |

### Set Operations

`SetUnion`, `SetIntersection`, `SetDifference`, `SetSymDifference` — all 3-register, both operands must be `TAG_SET`.

### Method Dispatch

| OpCode | Description |
|---|---|
| `MethodCall { dst, kind, base, arg_count }` | Dispatch built-in method by `MethodKind` enum — no string lookup at runtime |
| `MethodCallCustom { dst, method_name_idx, base, arg_count }` | Dispatch dynamic method (JSON field, alias) by string from constants table |

`base` points to the receiver register; arguments are `locals[base+1..base+1+arg_count]`. `MethodKind` is a `#[derive(Copy)]` enum covering ~50 built-in methods (`Push`, `Pop`, `Get`, `Insert`, `Update`, `Delete`, `Where`, `Join`, `Sort`, `Format`, `Next`, `IsDone`, `Close`, etc.). The compiler resolves method names to `MethodKind` variants at compile time via `map_method_kind()`.

### Fiber Operations

| OpCode | Description |
|---|---|
| `FiberCreate { dst, func_idx, base, arg_count }` | Allocate `FiberState`, pre-populate locals from args, store `Fiber` in `dst` |
| `Yield { src }` | Suspend fiber, return value in `src` to caller |
| `YieldVoid` | Suspend void fiber |

### I/O and System

| OpCode | Description |
|---|---|
| `Print { src }` | Print `locals[src]` to stdout |
| `Input { dst }` | Read line from stdin → `dst` |
| `Wait { src }` | Sleep for `src` milliseconds |
| `HaltAlert { src }` | Print alert message, continue execution |
| `HaltError { src }` | Print error + span info, halt frame, increment error count |
| `HaltFatal { src }` | Print fatal + span info, halt frame, increment error count |
| `TerminalExit` | `std::process::exit(0)` |
| `TerminalClear` | Clear terminal via ANSI escape or OS command |
| `TerminalRun { dst, cmd_src }` | Execute external command, result → `dst` |
| `EnvGet { dst, src }` | Read environment variable named by `src` → `dst` |
| `EnvArgs { dst }` | Push `Array<String>` of CLI arguments → `dst` |

### HTTP

| OpCode | Description |
|---|---|
| `HttpCall { dst, method_idx, url_src, body_src }` | Simple HTTP call (GET/POST/etc.) via `ureq`, result JSON → `dst` |
| `HttpRequest { dst, arg_src }` | Full HTTP call from config map (method, url, headers, body, timeout) → `dst` |
| `HttpRespond { status_src, body_src, headers_src }` | Send HTTP response from within a handler fiber; triggers `Yield` to give control back |
| `HttpServe { func_idx, port_src, host_src, workers_src, routes_src }` | Start `tiny_http` server, spawn worker threads, block main thread until `SHUTDOWN` |

### Storage

`StoreWrite { base }`, `StoreRead { dst, base }`, `StoreAppend { base }`, `StoreExists { dst, base }`, `StoreDelete { base }`

### JSON

`JsonParse { dst, src }`, `JsonBind { idx, json_src, path_src }`, `JsonBindLocal { dst, json_src, path_src }`, `JsonInject { table_idx, json_src, mapping_src }`, `JsonInjectLocal { table_reg, json_src, mapping_src }`

### Type Casts

`CastInt { dst, src }`, `CastFloat { dst, src }`, `CastString { dst, src }`, `CastBool { dst, src }`

### Crypto and Dates

`CryptoHash { dst, pass_src, alg_src }`, `CryptoVerify { dst, pass_src, hash_src, alg_src }`, `CryptoToken { dst, len_src }`, `DateNow { dst }`

### Loop Optimisations

These opcodes are emitted by the compiler to fuse common loop-counter patterns into single instructions, reducing dispatch overhead and improving JIT traceability.

| OpCode | Description |
|---|---|
| `IncLocal { reg }` | Increment integer in register `reg` by 1 |
| `IncVar { idx }` | Increment global at `idx` by 1 |
| `LoopNext { reg, limit_reg, target }` | Increment `reg`, jump to `target` if `reg <= limit_reg`, otherwise fall through |
| `IncLocalLoopNext { inc_reg, reg, limit_reg, target }` | Increment `inc_reg` (a separate counter, e.g. array index), increment `reg` (loop var), conditional jump |
| `IncVarLoopNext { g_idx, reg, limit_reg, target }` | Like `IncLocalLoopNext` but `g_idx` is a global counter |

---

## Tracing JIT Compiler

### Overview

XCX 2.2 includes a **tracing JIT** that automatically compiles hot loops to native machine code using the [Cranelift](https://github.com/bytecodealliance/wasmtime/tree/main/cranelift) code generation framework.

The JIT is entirely transparent to the programmer — it activates automatically and falls back to the interpreter on type guards or unsupported operations.

### Trace Detection

On every `Jump { target }` where `target < current_ip` (a backward, i.e. loop back-edge):

1. `hot_counts[target]` is incremented.
2. When `hot_counts[target] >= 50`, a new `Trace` is started with `start_ip = target`.
3. Trace recording is gated: it only begins if `trace_cache[target].is_none()` (no compiled trace exists yet) and `is_recording` is false.

### Trace Recording

While `is_recording` is true, the interpreter executes each opcode normally **and** records a `TraceOp` specialised for the current runtime types. For example:

- An `Add` on two integer registers records `GuardInt { reg: src1 }`, `GuardInt { reg: src2 }`, `AddInt { dst, src1, src2 }` — not a generic `Add`.
- A `JumpIfFalse` that is **not** taken records `GuardTrue { reg: src, fail_ip: target }` — asserting the branch is always false.
- A `JumpIfFalse` that **is** taken records `GuardFalse { reg: src, fail_ip: next_ip }`.

If an opcode cannot be traced (complex method calls, string operations, etc.), recording is aborted and `is_recording` is set back to false.

### Trace Compilation

When the traced loop executes back to `start_ip`, the complete `Trace` is handed to the Cranelift `JIT::compile()` function (`src/backend/jit.rs`). Cranelift compiles the `TraceOp` sequence to a native function with the signature:

```rust
unsafe extern "C" fn(
    locals_ptr: *mut Value,
    globals_ptr: *mut Value,
    consts_ptr:  *const Value,
) -> i32
```

The return value is the next IP to resume at (0 = continue normally, positive = side-exit IP, negative = halt).

The compiled function pointer is stored in `Trace::native_ptr` (an `AtomicPtr<u8>`) and the `Arc<Trace>` is inserted into both `vm.traces` (globally shared) and `trace_cache` (per-executor fast path).

### Trace Execution

On each iteration of the dispatch loop, before fetching the next opcode:

```
if trace_cache[current_ip].is_some() {
    execute_trace(trace, ip, locals, &mut glbs)
    continue
}
```

If a compiled native function is available (`native_ptr != null`), it is called via `transmute` directly — bypassing the interpreter entirely for the entire loop body. If the JIT has not compiled yet, the interpreted `TraceOp` path is used as an intermediate step.

### TraceOp Variants

| Variant | Description |
|---|---|
| `LoadConst`, `Move` | Register moves with constant values |
| `AddInt/SubInt/MulInt/DivInt/ModInt` | Integer arithmetic with fail-IP for div/mod by zero |
| `AddFloat/SubFloat/MulFloat/DivFloat/ModFloat` | Float arithmetic |
| `CmpInt / CmpFloat` | Comparison using a `cc: u8` condition code |
| `GuardInt / GuardFloat` | Type guard — exits trace if register is wrong type |
| `GuardTrue / GuardFalse` | Branch guard — exits trace on unexpected branch direction |
| `CastIntToFloat` | Widen int register to float |
| `IncLocal / IncVar` | Single-register/global increment |
| `LoopNextInt` | Combined increment + conditional jump for range loops |
| `IncVarLoopNext / IncLocalLoopNext` | Fused variants for array and for-range loops |
| `GetVar / SetVar` | Global variable access |
| `And / Or / Not` | Boolean logic |
| `Jump` | Unconditional jump (triggers loop-back detection in trace) |

---

## Execution Flow

```
VM::run(main_chunk, ctx)                        [Arc<VM>]
  └─ Executor::run_frame_owned(main_chunk)
       └─ execute_bytecode(bytecode, &mut ip, &mut locals)
            │
            ├─ [JIT fast path] if trace_cache[ip].is_some():
            │     execute_trace(trace, ip, locals, globals)
            │     → returns next IP or None
            │
            └─ [Interpreter path] fetch opcode, execute
                 ├─ Continue      → advance ip normally
                 ├─ Jump(t)       → ip = t; increment hot_counts if backward
                 ├─ Return(val)   → exit frame, return val
                 ├─ Yield(val)    → suspend (fiber), return val to caller
                 └─ Halt          → stop, increment error_count
```

Functions are called via `run_frame(func_id, params)`, which creates a fresh `locals` vector pre-sized to `chunk.max_locals`. `current_spans` is swapped to the called function's span table and restored on return.

---

## Fiber Execution Model

Fibers are **cooperative coroutines**, not threads.

```rust
pub struct FiberState {
    pub func_id:       usize,
    pub ip:            usize,
    pub locals:        Vec<Value>,    // moved out during resume, moved back after
    pub is_done:       bool,
    pub yielded_value: Option<Value>, // cached value for IsDone + Next pattern
}
```

### Resume Sequence (`resume_fiber`)

1. Read `func_id`, `ip`, and **move** `locals` out of `FiberState` via `std::mem::take` — no clone.
2. Run `execute_bytecode` from `fiber.ip` with the moved locals.
3. On `Yield`: set `fiber_yielded = true`. Move locals back into `FiberState`. Update `fiber.ip`. Return yielded value.
4. On `Return` / bytecode end: set `fiber.is_done = true`. Return final value.

Resume/suspend involves no heap allocation beyond the initial `Vec` creation — only moves.

### IsDone / Next Pattern

`IsDone` checks `FiberState::is_done`, factoring in whether `yielded_value` is cached. `Next` takes the cached `yielded_value` if present (from a previous resume that already ran), or calls `resume_fiber`. This ensures a `for x in fiber` loop never double-advances the fiber.

### For-Loop over Fiber (`ForIterType::Fiber`)

The compiler emits:
1. `MethodCall(IsDone)` → `JumpIfTrue` to exit
2. `MethodCall(Next)` → assign to loop variable
3. Loop body
4. `Jump` back to step 1
5. On `break`: `MethodCall(Close, base=fiber_reg)` marks fiber done before jumping out

---

## HTTP Server (`HttpServe`)

`HttpServe` starts a `tiny_http::Server` and spawns N OS threads:

```rust
for _ in 0..workers {
    let server  = server.clone();    // Arc<tiny_http::Server>
    let vm      = vm_arc.clone();    // Arc<VM>
    let ctx     = self.ctx.clone();  // SharedContext (two Arc clones)
    let routes  = routes.clone();    // Arc<Vec<(String, usize)>>
    std::thread::spawn(move || { /* recv → match route → run handler fiber */ });
}
```

Each worker runs its own `Executor` with its own locals. Globals are shared via `Arc<RwLock<Vec<Value>>>`.

### Request Handling

For each incoming request, the worker:
1. Matches the `"METHOD /path"` key against the routes table (case-insensitive).
2. Builds a JSON object `{ method, url, body, ip, headers }` as a `Value::Json`.
3. Stores the `tiny_http::Request` in `Arc<Mutex<Option<tiny_http::Request>>>` and passes it to a fresh `Executor` via `http_req`.
4. Runs the matched handler fiber synchronously in that worker's `Executor`.
5. When the handler calls `net.respond(...)`, the VM executes `HttpRespond` which sends the response and returns `OpResult::Yield` to end the handler.
6. If the handler exits without calling `net.respond`, the worker sends a `500` fallback response.

### Graceful Shutdown

`SHUTDOWN` is a `pub static AtomicBool` in `vm.rs`. A Ctrl+C handler in `main.rs` sets it to `true`. Workers check it every `recv_timeout(100ms)` cycle. The main thread blocks in a `sleep(500ms)` loop also polling `SHUTDOWN`. Once set, all loops exit and the process terminates cleanly.

---

## Compiler (`src/backend/mod.rs`)

### Register Allocation

`FunctionCompiler` tracks the next available register with `next_local: usize`:

```rust
pub fn push_reg(&mut self) -> u8 {
    let r = self.next_local as u8;
    self.next_local += 1;
    if self.next_local > self.max_locals_used {
        self.max_locals_used = self.next_local;
    }
    r
}

pub fn pop_reg(&mut self) {
    self.next_local -= 1;
}
```

Locals (named variables) are assigned a slot via `define_local(id, slot)` and stored in `scopes: Vec<HashMap<StringId, usize>>`. Temporaries use `push_reg()`/`pop_reg()` — they are reused once the expression result is consumed.

`max_locals_used` is recorded so `FunctionChunk::max_locals` can pre-allocate the exact right-size locals vector when the function is called.

### Constant Deduplication

`CompileContext::add_constant` deduplicates string constants via `string_constants: HashMap<String, usize>`. Duplicate string constants (e.g., `"insert"` appearing many times as a method name argument) reuse the same constants-table slot.

### Two-Pass Compilation

**Pass 1** — `register_globals_recursive`:
- Assigns a slot index to every global variable and fiber-decl instance
- Assigns a function index to every function/fiber
- Pre-allocates empty `FunctionChunk` slots in `functions: Vec<FunctionChunk>`

**Pass 2** — `compile_stmt` / `compile_expr`:
- Emits bytecode paired with spans via `emit(op, span)`
- Top-level statements in `main` use `GetVar`/`SetVar` (globals); nested statements use registers

### `FunctionChunk`

```rust
pub struct FunctionChunk {
    pub bytecode:   Arc<Vec<OpCode>>,
    pub spans:      Arc<Vec<Span>>,    // spans[i] corresponds to bytecode[i]
    pub is_fiber:   bool,
    pub max_locals: usize,
}
```

Bytecode and spans are `Arc`-wrapped so they can be shared across HTTP worker threads without copying.

---

## Runtime Error Reporting

Every runtime error in the VM appends `self.current_span_info(ip)` which returns `" [line: X, col: Y]"` by looking up `current_spans[ip - 1]`. Example:

```
ERROR: R303: Array index out of bounds: 5 [line: 14, col: 7]
```

`current_spans` is swapped to the correct `Arc<Vec<Span>>` on every `run_frame` call and restored on exit.

---

## Memory Model

- **No garbage collector**. Reference counting via `Arc` (with manual `inc_ref`/`dec_ref` for NaN-boxed pointer values).
- **Scalar values** (Int, Float, Bool, Date, Function index): stored entirely in the `u64` — zero heap allocation.
- **Collection values**: `Arc<RwLock<T>>` provides shared ownership. Cloning a collection `Value` increments only the `Arc` counter.
- **Mutations**: `.insert()`, `.update()`, `.delete()` acquire a write lock. All handles to the same collection see the change.
- **Read-only methods**: `.size()`, `.get()`, `.contains()` acquire a read lock — multiple concurrent readers are allowed.

---

## Security Controls

### Network SSRF Protection (`is_safe_url`)

Blocked targets:
- `file://` URLs
- `169.254.x.x` (link-local / AWS metadata endpoint)
- Private ranges: `10.x`, `192.168.x`, `172.16–31.x` (when not localhost)

Applied to both `HttpCall` and `HttpRequest`.

### HTTP Body Limit

In `HttpServe`, the response body is checked after `into_string()`. If it exceeds **10 MB**, a `413` error JSON is returned instead of the actual body.

### CORS Headers

All `HttpServe` responses automatically include:
- `Access-Control-Allow-Origin: *`
- `Access-Control-Allow-Methods: GET, POST, OPTIONS, DELETE, PATCH`
- `Access-Control-Allow-Headers: Content-Type, Authorization, X-CSRF-TOKEN`

`OPTIONS` preflight requests receive a `204` response without invoking any handler.