# XCX Backend — Compiler and VM

> **Files:** `src/backend/mod.rs`, `src/backend/vm.rs`

---

## Table of Contents

1. [Bytecode Compiler](#bytecode-compiler)
2. [Value Representation — NaN-Boxing](#value-representation--nan-boxing)
3. [Instruction Set (OpCodes)](#instruction-set-opcodes)
4. [VM Architecture](#vm-architecture)
5. [Fiber Execution Model](#fiber-execution-model)
6. [HTTP Server](#http-server)
7. [Memory Management](#memory-management)
8. [Loop Optimizations](#loop-optimizations)

---

## Bytecode Compiler

**File:** `src/backend/mod.rs`

### Register Allocation

`FunctionCompiler` tracks the next available register via `next_local: usize`:

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

Local variables (named) are assigned to a slot via `define_local(id, slot)` and stored in `scopes: Vec<HashMap<StringId, usize>>`. Temporary variables use `push_reg()`/`pop_reg()` — they are reused when the result of an expression is consumed.

`max_locals_used` is recorded so that `FunctionChunk::max_locals` can pre-allocate a locals vector of exactly the right size when the function is called.

### Constant Deduplication

`CompileContext::add_constant` deduplicates string constants using `string_constants: HashMap<String, usize>`. Duplicate string constants (e.g., `"insert"` appearing multiple times as a method name argument) reuse the same constant table slot.

### Two-pass Compilation

**Pass 1** — `register_globals_recursive`:
- Assigns a slot index to every global variable and fiber-decl instance
- Assigns a function index to every function/fiber
- Pre-allocates empty `FunctionChunk` slots in `functions: Vec<FunctionChunk>`

**Pass 2** — `compile_stmt` / `compile_expr`:
- Emits bytecode paired with spans via `emit(op, span)`
- Top-level instructions in `main` use `GetVar`/`SetVar` (globals); nested instructions use registers

### FunctionChunk

```rust
pub struct FunctionChunk {
    pub bytecode:   Arc<Vec<OpCode>>,
    pub spans:      Arc<Vec<Span>>,    // spans[i] corresponds to bytecode[i]
    pub is_fiber:   bool,
    pub max_locals: usize,
}
```

Bytecode and spans are wrapped in `Arc` so they can be shared across HTTP worker threads without copying.

---

## Value Representation — NaN-Boxing

Each value is a single `Value(u64)` — a 64-bit word. XCX uses **NaN-boxing**: the IEEE 754 quiet NaN bit pattern is repurposed as a type tag prefix.

```
Bit layout: [63..52: exponent/QNAN] [51..48: type tag] [47..0: payload]

Float : stored directly as f64 bits — DOES NOT have the QNAN_BASE prefix set
Int   : QNAN_BASE | TAG_INT  | (i48 value & 0x0000_FFFF_FFFF_FFFF)
Bool  : QNAN_BASE | TAG_BOOL | (0 or 1)
Date  : QNAN_BASE | TAG_DATE | (i48 ms timestamp)
Ptr   : QNAN_BASE | TAG_XXX  | (pointer & 0x0000_FFFF_FFFF_FFFF)
```

### Tag Constants

| Constant | Value | Type |
|---|---|---|
| `QNAN_BASE` | `0x7FF0_0000_0000_0000` | base NaN marker |
| `TAG_INT` | `0x0001_0000_0000_0000` | 48-bit signed integer |
| `TAG_BOOL` | `0x0002_0000_0000_0000` | boolean (payload 0/1) |
| `TAG_DATE` | `0x0003_0000_0000_0000` | 48-bit timestamp (ms) |
| `TAG_STR` | `0x0004_0000_0000_0000` | `Arc<Vec<u8>>` pointer |
| `TAG_ARR` | `0x0005_0000_0000_0000` | `Arc<RwLock<Vec<Value>>>` pointer |
| `TAG_SET` | `0x0006_0000_0000_0000` | `Arc<RwLock<SetData>>` pointer |
| `TAG_MAP` | `0x0007_0000_0000_0000` | `Arc<RwLock<Vec<(Value,Value)>>>` pointer |
| `TAG_TBL` | `0x0008_0000_0000_0000` | `Arc<RwLock<TableData>>` pointer |
| `TAG_FUNC` | `0x0009_0000_0000_0000` | function index (u32) |
| `TAG_ROW` | `0x000A_0000_0000_0000` | `Arc<RowRef>` pointer |
| `TAG_JSON` | `0x000B_0000_0000_0000` | `Arc<RwLock<serde_json::Value>>` pointer |
| `TAG_FIB` | `0x000C_0000_0000_0000` | `Arc<RwLock<FiberState>>` pointer |
| `TAG_DB` | `0x000D_0000_0000_0000` | `Arc<DatabaseData>` pointer |

Pointer payloads use only the lower 48 bits — valid on all x86-64 and AArch64 platforms where user-space pointers fit in 48 bits.

### Reference Counting

Pointer-tagged values carry `Arc` reference counts. The VM manages them manually via `inc_ref()` / `dec_ref()` on every assignment, return, and collection modification — ensuring that heap-allocated objects are freed when no longer referenced, without a garbage collector.

---

## Instruction Set (OpCodes)

All opcodes are register-based: they refer to named `u8` register slots instead of an operand stack.

### Register / Variable Movement

| OpCode | Description |
|---|---|
| `LoadConst { dst, idx }` | Load `constants[idx]` into register `dst` |
| `Move { dst, src }` | Copy register `src` to `dst` |
| `GetVar { dst, idx }` | Load `globals[idx]` into `dst` (read-lock globals) |
| `SetVar { idx, src }` | Store `src` into `globals[idx]` (write-lock globals) |

### Arithmetic

All arithmetic operations are 3-register: `dst = src1 OP src2`. Runtime type dispatch selects integer, float, string-concat, date-arithmetic, or set-operation paths.

`Add`, `Sub`, `Mul`, `Div`, `Mod`, `Pow`, `IntConcat` (`++`)

### Comparisons (Bool result)

`Equal`, `NotEqual`, `Greater`, `Less`, `GreaterEqual`, `LessEqual`

### Logic

`And { dst, src1, src2 }`, `Or { dst, src1, src2 }`, `Not { dst, src }`, `Has { dst, src1, src2 }`

### Control Flow

| OpCode | Description |
|---|---|
| `Jump { target }` | Unconditional jump; increments `hot_counts[target]` on backward jumps |
| `JumpIfFalse { src, target }` | Jump if `src` is `Bool(false)` |
| `JumpIfTrue { src, target }` | Jump if `src` is `Bool(true)` |
| `Call { dst, func_idx, base, arg_count }` | Call function; result → `dst` |
| `Return { src }` | Return value in `src` from current frame |
| `ReturnVoid` | Return without value |
| `Halt` | Stop execution |

### Collections

| OpCode | Description |
|---|---|
| `ArrayInit { dst, base, count }` | Collect `count` registers from `base` → new array in `dst` |
| `SetInit { dst, base, count }` | Collect `count` registers → new set in `dst` |
| `SetRange { dst, start, end, step, has_step }` | Build a range set from register values |
| `MapInit { dst, base, count }` | Collect `count` key-value pairs → new map in `dst` |
| `TableInit { dst, skeleton_idx, base, row_count }` | Build a table from column schema constant + row values |

### Set Operations

`SetUnion`, `SetIntersection`, `SetDifference`, `SetSymDifference` — all 3-register, both operands must be `TAG_SET`.

### Method Dispatch

| OpCode | Description |
|---|---|
| `MethodCall { dst, kind, base, arg_count }` | Dispatch built-in method via `MethodKind` enum — no runtime string lookup |
| `MethodCallCustom { dst, method_name_idx, base, arg_count }` | Dispatch dynamic method (JSON field, alias) via string from constant table |
| `MethodCallNamed { dst, kind, base, arg_count, names_idx }` | Method call with named arguments |

`base` points to the receiver register; arguments are in `locals[base+1..base+1+arg_count]`. `MethodKind` is a `#[derive(Copy)]` enum covering ~50 built-in methods (`Push`, `Pop`, `Get`, `Insert`, `Update`, `Delete`, `Where`, `Join`, `Sort`, `Format`, `Next`, `IsDone`, `Close`, etc.).

### Fiber Operations

| OpCode | Description |
|---|---|
| `FiberCreate { dst, func_idx, base, arg_count }` | Allocate `FiberState`, pre-fill locals from args, store `Fiber` in `dst` |
| `Yield { src }` | Suspend fiber, return value in `src` to caller |
| `YieldVoid` | Suspend void fiber |

### I/O and System

| OpCode | Description |
|---|---|
| `Print { src }` | Print `locals[src]` to stdout |
| `Input { dst, ty }` | Read line from stdin → `dst` with type casting |
| `Wait { src }` | Sleep for `src` milliseconds |
| `HaltAlert { src }` | Print warning message, continue execution |
| `HaltError { src }` | Print error + span info, stop frame, increment error_count |
| `HaltFatal { src }` | Print fatal + span info, stop frame, increment error_count |
| `TerminalExit` | `std::process::exit(0)` |
| `TerminalClear` | Clear terminal via ANSI escape |
| `TerminalRaw / TerminalNormal` | Enable/disable raw terminal mode |
| `TerminalCursor { on }` | Show/hide cursor |
| `TerminalMove { x_src, y_src }` | Move cursor |
| `TerminalWrite { src }` | Write without newline |
| `InputKey { dst }` | Read key (non-blocking) |
| `InputKeyWait { dst }` | Read key (blocking) |
| `InputReady { dst }` | Check if input is available |
| `EnvGet { dst, src }` | Read environment variable |
| `EnvArgs { dst }` | Get CLI arguments |

### HTTP

| OpCode | Description |
|---|---|
| `HttpCall { dst, method_idx, url_src, body_src }` | Simple HTTP call via `ureq`, JSON result → `dst` |
| `HttpRequest { dst, arg_src }` | Full HTTP call from config map (method, url, headers, body, timeout) → `dst` |
| `HttpRespond { status_src, body_src, headers_src }` | Send HTTP response from inside fiber handler |
| `HttpServe { func_idx, port_src, host_src, workers_src, routes_src }` | Start `tiny_http` server, spawn worker threads |

### Storage

`StoreWrite`, `StoreRead`, `StoreAppend`, `StoreExists`, `StoreDelete`, `StoreList`, `StoreIsDir`, `StoreSize`, `StoreMkdir`, `StoreGlob`, `StoreZip`, `StoreUnzip`

### JSON

`JsonParse { dst, src }`, `JsonBind { idx, json_src, path_src }`, `JsonBindLocal { dst, json_src, path_src }`, `JsonInject { table_idx, json_src, mapping_src }`, `JsonInjectLocal { table_reg, json_src, mapping_src }`

### Type Casts

`CastInt { dst, src }`, `CastFloat { dst, src }`, `CastString { dst, src }`, `CastBool { dst, src }`

### Crypto and Dates

`CryptoHash`, `CryptoVerify`, `CryptoToken`, `DateNow`

### Database

`DatabaseInit { dst, engine_src, path_src, tables_base_reg, table_count }`

---

## VM Architecture

**File:** `src/backend/vm.rs`

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
    hot_counts:       Vec<usize>,         // IP backward jump counter
    recording_trace:  Option<Trace>,      // trace being recorded
    is_recording:     bool,
    trace_cache:      Vec<Option<Arc<Trace>>>, // compiled traces indexed by start IP
    http_req:         Option<Arc<Mutex<Option<tiny_http::Request>>>>,
    http_req_val:     Option<Value>,
    terminal_raw_enabled: bool,
}
```

### SharedContext

```rust
pub struct SharedContext {
    pub constants: Arc<Vec<Value>>,
    pub functions: Arc<Vec<FunctionChunk>>,
}
```

`SharedContext` is cheaply cloned (two `Arc` counter bumps) and passed to each worker thread independently. No deep copying.

### Execution Flow

```
VM::run(main_chunk, ctx)                        [Arc<VM>]
  └─ Executor::run_frame_owned(main_chunk)
       └─ execute_bytecode_inner(bytecode, &mut ip, &mut locals, ...)
            │
            ├─ [JIT Fast Path] if trace_cache[ip].is_some():
            │     execute_trace(trace, ip, locals, globals)
            │     → returns next IP or None
            │
            └─ [Interpreter Path] fetch opcode, execute
                 ├─ Continue      → proceed normally
                 ├─ Jump(t)       → ip = t; increment hot_counts if backward
                 ├─ Return(val)   → exit frame, return val
                 ├─ Yield(val)    → suspend (fiber), return val to caller
                 └─ Halt          → stop, increment error_count
```

---

## Fiber Execution Model

Fibers are **cooperative coroutines**, not OS threads.

```rust
pub struct FiberState {
    pub func_id:       usize,
    pub ip:            usize,
    pub locals:        Vec<Value>,    // moved during resume, returned after
    pub is_done:       bool,
    pub yielded_value: Option<Value>, // cache for IsDone + Next pattern
}
```

### Resume Sequence (`resume_fiber`)

1. Read `func_id`, `ip` and **move** `locals` from `FiberState` via `std::mem::take` — no cloning
2. Start `execute_bytecode` from `fiber.ip` with moved locals
3. On `Yield`: set `fiber_yielded = true`. Move locals back to `FiberState`. Update `fiber.ip`. Return yielded value.
4. On `Return` / end of bytecode: set `fiber.is_done = true`. Return final value.

Resuming/suspending does not trigger any heap allocations besides initial `Vec` creation — only moves.

### IsDone / Next Pattern

`IsDone` checks `FiberState::is_done`, taking into account if `yielded_value` is cached. `Next` takes cached `yielded_value` if present, or calls `resume_fiber`. This ensures that a `for x in fiber` loop never double-advances the fiber.

### For Loop Over Fiber (`ForIterType::Fiber`)

The compiler emits:
1. `MethodCall(IsDone)` → `JumpIfTrue` to exit
2. `MethodCall(Next)` → assign to loop variable
3. Loop body
4. `Jump` back to step 1
5. On `break`: `MethodCall(Close, base = fiber_reg)` marks the fiber as done before jumping

---

## HTTP Server

`HttpServe` starts a `tiny_http` server and spawns N OS threads:

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
1. Matches the `"METHOD /path"` key against the route table (case-insensitive)
2. Builds a JSON object `{ method, url, body, ip, headers }` as `Value::Json`
3. Stores `tiny_http::Request` in `Arc<Mutex<Option<tiny_http::Request>>>` and passes it to a fresh `Executor`
4. Runs the matched handler fiber synchronously in that worker's `Executor`
5. When the handler calls `net.respond(...)`, the VM executes `HttpRespond` which sends the response
6. If the handler exits without `net.respond`, the worker sends a `500` response

### Graceful Shutdown

`SHUTDOWN` is a `pub static AtomicBool` in `vm.rs`. The Ctrl+C handler in `main.rs` sets it to `true`. Worker threads check it every `recv_timeout(100ms)`. The main thread blocks in a `sleep(500ms)` loop also polling `SHUTDOWN`. Once set, all loops exit and the process terminates cleanly.

---

## Memory Management

- **No garbage collector**. Reference counting via `Arc` (with manual `inc_ref`/`dec_ref` for NaN-boxed pointer values).
- **Scalar values** (Int, Float, Bool, Date, Function index): stored entirely within the `u64` — zero heap allocations.
- **Collection values**: `Arc<RwLock<T>>` ensures shared ownership. Cloning a collection `Value` only increments the `Arc` counter.
- **Mutations**: `.insert()`, `.update()`, `.delete()` acquire a write lock. All handles to the same collection see the change.
- **Read-only methods**: `.size()`, `.get()`, `.contains()` acquire a read lock — multiple simultaneous readers are allowed.

---

## Loop Optimizations

These opcodes are emitted by the compiler to merge common loop counter patterns into single instructions, reducing dispatch overhead and improving JIT tracing:

| OpCode | Description |
|---|---|
| `IncLocal { reg }` | Increment integer in register `reg` by 1 |
| `IncVar { idx }` | Increment global at `idx` by 1 |
| `LoopNext { reg, limit_reg, target }` | Increment `reg`, jump to `target` if `reg <= limit_reg` |
| `IncLocalLoopNext { inc_reg, reg, limit_reg, target }` | Increment `inc_reg` (separate counter, e.g., array index), increment `reg` (loop variable), conditional jump |
| `IncVarLoopNext { g_idx, reg, limit_reg, target }` | Like `IncLocalLoopNext` but `g_idx` is a global counter |
| `ArrayLoopNext { idx_reg, size_reg, target }` | Combined array index iteration |

### Optimization Transformation

The compiler checks the last emitted instruction before the end of a loop step. If it is `IncVar` or `IncLocal`, it replaces them with `IncVarLoopNext` or `IncLocalLoopNext` respectively, merging the increment with the conditional loop test:

```rust
match self.bytecode[len - 1] {
    OpCode::IncVar { idx } => {
        self.bytecode.pop();
        self.emit(OpCode::IncVarLoopNext { g_idx: idx, reg: loop_var_reg, ... });
    }
    OpCode::IncLocal { reg } => {
        self.bytecode.pop();
        self.emit(OpCode::IncLocalLoopNext { inc_reg: reg, ... });
    }
}
```
