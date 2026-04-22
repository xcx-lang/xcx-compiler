# XCX Compiler — Documentation v3.0

> XCX language compiler implemented in Rust. Multi-stage pipeline: lexer → parser → semantic analysis → bytecode compiler → virtual machine → JIT (Cranelift).

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Quick Start](#quick-start)
3. [Project Structure](#project-structure)
4. [Compilation Pipeline](#compilation-pipeline)
5. [Modules](#modules)
   - [Lexer (Scanner)](#lexer-scanner)
   - [Parser (Pratt)](#parser-pratt)
   - [Expander](#expander)
   - [Semantic Analysis (Sema)](#semantic-analysis-sema)
   - [Compiler (Backend)](#compiler-backend)
   - [Virtual Machine (VM)](#virtual-machine-vm)
   - [JIT (Cranelift)](#jit-cranelift)
6. [Key Design Decisions](#key-design-decisions)
7. [Diagnostic System](#diagnostic-system)
8. [Security](#security)

---

## Architecture Overview

```
Source Code (.xcx)
        │
        ▼
  1. Lexer         src/lexer/scanner.rs       → token stream
        │
        ▼
  2. Parser        src/parser/pratt.rs        → raw AST (Program)
        │
        ▼
  3. Expander      src/parser/expander.rs     → expanded AST (include, aliases)
        │
        ▼
  4. Sema          src/sema/checker.rs        → validated, annotated AST
        │
        ▼
  5. Compiler    src/backend/mod.rs         → FunctionChunk + constants + functions
        │
        ▼
  6. VM            src/backend/vm.rs          → bytecode execution (register-based)
        │  hot loops → trace recording
        ▼
  7. JIT           src/backend/jit.rs         → native machine code (Cranelift)
```

---

## Quick Start

```bash
# Start REPL
xcx

# Run file
xcx program.xcx

# Version
xcx --version

# Help
xcx --help
```

**Inside REPL:**

```
xcx> !help     # display help
xcx> !clear    # clear screen
xcx> !exit     # exit
```

---

## Project Structure

```
src/
├── lexer/
│   ├── scanner.rs        # Byte scanner (&[u8])
│   └── token.rs          # TokenKind and Span
├── parser/
│   ├── pratt.rs          # Pratt Parser (tokens → AST)
│   ├── expander.rs       # Resolving include and prefixing aliases
│   └── ast.rs            # AST node definitions (Expr, Stmt, Type, Program)
├── sema/
│   ├── checker.rs        # Type checking and variable resolution
│   ├── symbol_table.rs   # Hierarchical symbol table
│   └── interner.rs       # String interner (str → StringId)
├── backend/
│   ├── mod.rs            # Bytecode compiler (AST → OpCode)
│   ├── vm.rs             # Register-based VM with NaN-boxing + JIT hooks
│   ├── jit.rs            # Cranelift trace compiler
│   └── repl.rs           # Interactive REPL
└── diagnostic/
    └── report.rs         # Error reporter with code highlighting
```

---

## Compilation Pipeline

### 1. Lexer
Converts source bytes into tokens. Works on `&[u8]` — no `Vec<char>` allocation.

### 2. Parser
Pratt Parser (Top-Down Operator Precedence) builds the AST. One-token lookahead. Error recovery via `synchronize()`.

### 3. Expander
Runs **after** parsing, **before** semantic analysis. Resolves `include` directives and prefixes alias names.

### 4. Semantic Analysis
Checks types, detects undefined variables, validates fiber/loop context. Collects all errors before generating bytecode.

### 5. Compiler
Two-pass. Pass 1: registers globals/functions. Pass 2: emits bytecode with Span annotations.

### 6. VM
Register-based virtual machine. Each frame has a flat `Vec<Value>` indexed by `u8` slot numbers. 8-byte values (NaN-boxing).

### 7. JIT
Automatic compilation of hot loops to native code via Cranelift. Transparent to the developer.

---

## Modules

Detailed descriptions in separate files:

- [`docs/lexer.md`](docs/lexer.md) — Lexer / Scanner
- [`docs/parser.md`](docs/parser.md) — Pratt Parser and AST
- [`docs/expander.md`](docs/expander.md) — Expander
- [`docs/sema.md`](docs/sema.md) — Semantic Analysis
- [`docs/backend.md`](docs/backend.md) — Compiler and VM
- [`docs/jit.md`](docs/jit.md) — Cranelift JIT
- [`docs/language.md`](docs/language.md) — XCX Language Reference

---

## Key Design Decisions

### NaN-Boxing of values
Each value is a single `u64` (8 bytes). High bits of IEEE 754 quiet NaN serve as type tags. Lower 48 bits carry the payload. No heap allocation for scalars, no tag overhead in enums, no pointer indirection for primitives.

### Register-based VM
Instead of an operand stack — a flat `Vec<Value>` array per frame. Opcodes refer directly to source/destination registers. Simple bump-pointer register allocation in the compiler.

### Tracing JIT (Cranelift)
Backward jumps (loop edges) are counted per IP. After reaching a threshold (50 iterations), trace recording begins. The finished trace is compiled to native code by Cranelift. Type guards (`GuardInt`, `GuardFloat`) handle type specialization; a failed guard triggers a fallback to the interpreter.

### String Interning
All identifiers and string literals are interned by the `Interner` to `StringId (u32)`. Eliminates repetitive string allocations and heap comparisons throughout the pipeline.

### Two-pass Compilation
Pass 1 (`register_globals_recursive`) assigns indices to all globals and functions before emitting any bytecode — enabling mutual recursion and calls before declaration.

---

## Diagnostic System

The `Reporter` in `src/diagnostic/report.rs` produces contextual error messages:

```
ERROR: [S101] Undefined variable: foo
   12 | i: x = foo + 1;
              ~~~
```

Each error contains:
- **Level**: ERROR or HALT
- **Location**: line and column number
- **Visual highlighting**: relevant source line with `~~~` underline

Semantic errors (`TypeError`) are collected in a `Vec` during the checking phase and reported all at once before bytecode generation.

---

## Security

### SSRF Protection in Networking
Blocked targets:
- `file://` URLs
- `169.254.x.x` (link-local / AWS metadata endpoint)
- Private ranges: `10.x`, `192.168.x`, `172.16–31.x` (excluding localhost)

Applied to both `HttpCall` and `HttpRequest`.

### HTTP Content Size Limit
In `HttpServe`, the response content is checked after `into_string()`. If it exceeds **10 MB**, a 413 JSON error is returned instead of the actual content.

### CORS Headers
All `HttpServe` responses automatically include:
- `Access-Control-Allow-Origin: *`
- `Access-Control-Allow-Methods: GET, POST, OPTIONS, DELETE, PATCH`
- `Access-Control-Allow-Headers: Content-Type, Authorization, X-CSRF-TOKEN`

Preflight `OPTIONS` requests receive a `204` response without calling the handler.

### Secure File Paths
`store.*` operations validate paths — blocking `..`, absolute paths, and Windows drive letters.
