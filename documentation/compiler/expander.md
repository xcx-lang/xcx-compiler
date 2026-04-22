# XCX Expander — Documentation

> **File:** `src/parser/expander.rs`  
> Runs **after** parsing, **before** semantic analysis.

---

## Table of Contents

1. [Overview](#overview)
2. [Responsibilities](#responsibilities)
3. [Resolving include](#resolving-include)
4. [Alias Prefixing](#alias-prefixing)
5. [Protected Names](#protected-names)
6. [Include Path Search Order](#include-path-search-order)

---

## Overview

The Expander is a separate AST rewriting pass run after the parsing phase and before semantic analysis. It processes `include` and `include ... as alias` directives.

```rust
pub struct Expander<'a> {
    interner:       &'a mut Interner,
    visiting_files: HashSet<PathBuf>,   // for circular dependency detection
    included_files: HashSet<PathBuf>,   // deduplication (each file once)
    aliases:        HashMap<StringId, String>,
    include_paths:  Vec<PathBuf>,       // additional search paths
}
```

---

## Responsibilities

### 1. Resolving include
`include "file.xcx";` is replaced by the inlined AST of that file.

- Circular dependencies are detected via `visiting_files: HashSet<PathBuf>`
- Files are deduplicated via `included_files: HashSet<PathBuf>` (each file included only once, unless aliased)

### 2. Alias Prefixing
`include "math.xcx" as math;` causes all top-level names from that file to receive the prefix `math.name`. Call sites (`math.sin(x)`) are rewritten from `MethodCall` to `FunctionCall { name: "math.sin" }` via `expand_expr_inplace`.

Functions `prefix_program()` / `prefix_stmt_impl()` / `prefix_expr_impl()` traverse the entire sub-AST, renaming all top-level symbol references.

### 3. Fiber Name Prefixing
`FiberDecl::fiber_name` references are also prefixed, so that renamed fiber instantiations are correctly resolved after prefixing.

### 4. YieldFrom Prefixing
`StmtKind::YieldFrom` expressions are traversed such that fiber constructor calls inside `yield from` are also renamed.

---

## Resolving include

```xcx
include "utils.xcx";           --- Simple include (deduplicated)
include "math.xcx" as math;    --- Aliased include
```

After an aliased include:
- All top-level symbols from `math.xcx` get the `math.` prefix
- Calls to `math.sin(x)` are rewritten from `MethodCall` to `FunctionCall { name: "math.sin" }`

Example:

```xcx
--- math.xcx defines:
func sin(f: x -> f) { ... }
func cos(f: x -> f) { ... }

--- After include "math.xcx" as math:
--- sin → math.sin
--- cos → math.cos
--- Call math.sin(3.14) → FunctionCall("math.sin", [3.14])
```

---

## Alias Prefixing

The `prefix_stmt_impl` / `prefix_expr_impl` algorithm:

1. Collects all top-level names from the program (`top_level_names: HashSet<StringId>`)
2. For each statement/expression: if the identifier belongs to `top_level_names`, it is replaced with `prefix.name`
3. Specifically handles:
   - `FiberDecl::fiber_name` — so that instantiations work after rename
   - `StmtKind::YieldFrom` — fiber expressions in `yield from`
   - `MethodCall` on an aliased object → rewritten to `FunctionCall`

---

## Protected Names

The following names are **never** prefixed (protected built-ins):

```
json    date    store   halt    terminal
net     env     crypto  EMPTY   math
random  i       f       s       b
from    main
```

---

## Include Path Search Order

1. Relative to the current file's directory
2. In the `lib/` directory (relative to CWD, then walking up from the executable path)

Additional paths can be added via:

```rust
expander.add_include_path(path);
```

In `main.rs`, the `lib/` directory relative to CWD is automatically added:

```rust
if let Ok(cwd) = std::env::current_dir() {
    let lib_path = cwd.join("lib");
    if lib_path.exists() {
        expander.add_include_path(lib_path);
    }
}
```

---

## Error Detection

| Error | Description |
|---|---|
| `Circular dependency` | Include loop detected (`visiting_files`) |
| `File not found` | `include` file does not exist in any of the search paths |
| `Could not read file` | I/O error while reading the file |
