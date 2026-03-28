//==compiler/architecture.md==\\
# XCX Compiler Architecture — v2.2

The XCX Compiler is implemented in Rust and follows a multi-stage pipeline architecture.

## Compilation Pipeline

```
Source Code
    │
    ▼
1. Lexer (Scanner)        — src/lexer/scanner.rs
    │  Produces: Token stream
    ▼
2. Parser (Pratt)         — src/parser/pratt.rs
    │  Produces: Raw AST (Program)
    ▼
3. Expander               — src/parser/expander.rs
    │  Produces: Expanded AST (include directives resolved, aliases prefixed)
    ▼
4. Type Checker (Sema)    — src/sema/checker.rs
    │  Produces: Validated, annotated AST
    ▼
5. Compiler (Backend)     — src/backend/mod.rs
    │  Produces: FunctionChunk (main) + Arc<Vec<Value>> constants + Arc<Vec<FunctionChunk>> functions
    ▼
6. VM                     — src/backend/vm.rs
    │  Executes register-based bytecode
    │  Hot loops detected → Trace recording begins
    ▼
7. JIT (Cranelift)        — src/backend/jit.rs
       Compiles recorded traces to native machine code
```

> **Note**: The Expander is part of the `src/parser/` module but runs as a distinct post-parse phase, before semantic analysis. The JIT is an optional acceleration layer that activates automatically for hot loops — bytecode execution continues uninterrupted if a trace is not yet compiled.

## Project Structure

```
src/
├── lexer/
│   ├── scanner.rs      # Byte-level scanner (&[u8])
│   └── token.rs        # TokenKind and Span definitions
├── parser/
│   ├── pratt.rs        # Pratt parser (token stream → AST)
│   ├── expander.rs     # Include resolution and alias prefixing
│   └── ast.rs          # AST node definitions (Expr, Stmt, Type, Program)
├── sema/
│   ├── checker.rs      # Type checker and variable resolver
│   ├── symbol_table.rs # Hierarchical scope/symbol table with parent-pointer chain
│   └── interner.rs     # String interner (str → StringId)
├── backend/
│   ├── mod.rs          # Bytecode compiler (AST → OpCode) with register allocator
│   ├── vm.rs           # Register-based VM with NaN-boxed Values + tracing JIT hooks
│   ├── jit.rs          # Cranelift-based trace compiler
│   └── repl.rs         # Interactive REPL
└── diagnostic/
    └── report.rs       # Error reporter with source highlighting
```

## Diagnostic System

The compiler uses a `Reporter` struct to produce contextual error messages. Each error includes:
- **Level**: ERROR or HALT variant
- **Location**: line and column number
- **Visual highlight**: the relevant source line with a `~~~` underline

Semantic errors (`TypeError`) are collected in a `Vec` during the checking phase and reported all at once before bytecode generation begins. If any errors exist, compilation stops at that point.

Runtime errors produced by the VM include source location information (line and column) derived from the `spans` table stored alongside each `FunctionChunk`'s bytecode.

## Key Design Decisions

### NaN-Boxed Values
All values are represented as a single `u64` using NaN-boxing. The high bits of an IEEE 754 quiet NaN are used as type tags, and the low 48 bits carry the payload (integer, boolean, or pointer). Floats are stored as-is. This means every `Value` is exactly 8 bytes — no heap allocation for scalars, no tag-on-enum overhead, no pointer indirection for primitives. See `src/backend/vm.rs` for the full tag layout.

### Register-Based VM
The VM is **register-based**, not stack-based. Each function frame owns a flat `Vec<Value>` indexed by slot number. Opcodes reference source and destination registers directly (`dst`, `src1`, `src2` fields). The compiler's `FunctionCompiler` maintains a simple bump allocator (`push_reg()` / `pop_reg()`) for temporary registers. Locals and temporaries live in the same flat array — there is no separate operand stack.

### Tracing JIT (Cranelift)
Hot backward jumps are detected by a per-frame `hot_counts: Vec<usize>` counter. When a loop back-edge reaches the threshold (50 iterations), a trace recording phase begins. The `Executor` records `TraceOp` variants — typed, specialised versions of interpreter ops — until the trace closes (loop back to start IP). The completed `Trace` is compiled to native code via Cranelift and cached in `trace_cache`. Subsequent iterations of that loop bypass the interpreter entirely. Guards (`GuardInt`, `GuardFloat`, `GuardTrue`, `GuardFalse`) in the trace handle type specialisation; a guard failure causes a side-exit back to the interpreter at the correct IP.

### String Interning
All identifiers and string literals are interned via `Interner` into `StringId` (u32). This avoids repeated string allocations and heap comparisons throughout the pipeline.

### Constant Deduplication
The compiler maintains a `string_constants: HashMap<String, usize>` that ensures each unique string value is stored only once in the constants table. This is especially effective for built-in method names emitted frequently during compilation.

### Method Dispatch via Enum
Built-in method calls are compiled to `OpCode::MethodCall { kind: MethodKind, base, arg_count }` where `MethodKind` is a `Copy` enum covering ~50 built-in methods. Unknown or dynamic method names (e.g., JSON field access) use the separate `OpCode::MethodCallCustom { method_name_idx, base, arg_count }` path. This eliminates string comparisons in the VM dispatch loop.

### Two-Pass Compilation
The backend performs a first pass (`register_globals_recursive`) to assign indices to all globals, functions, and fibers before emitting any bytecode.

### Span-Annotated Bytecode
Each emitted opcode is paired with a `Span` from the source AST. `FunctionChunk` stores `spans: Arc<Vec<Span>>` alongside `bytecode: Arc<Vec<OpCode>>`. The VM uses this to produce line-accurate runtime error messages.

### Fiber-as-Coroutine
Fibers are not OS threads. Each `FiberState` stores its own `ip` and `locals`, resumed cooperatively by the VM. On yield, locals are moved back into `FiberState`; on resume, they are moved out again. This is allocation-free beyond the initial `Vec` creation.

### Thread-Safe Value Representation
All shared mutable collections use `Arc<RwLock<T>>` (via `parking_lot`). `FunctionChunk` bytecode and spans are wrapped in `Arc<Vec<...>>` so they can be shared across HTTP worker threads without copying. `Value` is `Copy` + `Send` + `Sync`.

### Graceful HTTP Shutdown
A global `AtomicBool` (`SHUTDOWN`) is set by a Ctrl+C signal handler (via the `ctrlc` crate). All HTTP worker threads and the main dispatch loop poll this flag and exit cleanly before the process terminates.

### Scope Chain Symbol Table
The `SymbolTable` uses a parent-pointer linked chain instead of deep cloning. Entering a function scope creates a new `SymbolTable` with a reference to the parent — lookup walks the chain upward. Function-scope entry is O(1) instead of O(n).

### Byte-Level Scanner
The lexer operates on `&[u8]` (a reference to the original source bytes) without allocating a `Vec<char>`. Unicode handling is done only where needed. Comment detection uses `slice.starts_with(b"---")`.