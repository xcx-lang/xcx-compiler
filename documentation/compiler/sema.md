# XCX Semantic Analysis (Sema) — Documentation

> **Files:** `src/sema/checker.rs`, `src/sema/symbol_table.rs`, `src/sema/interner.rs`

---

## Table of Contents

1. [Overview](#overview)
2. [String Interner](#string-interner)
3. [Symbol Table](#symbol-table)
4. [Type Checker](#type-checker)
5. [Type Compatibility Rules](#type-compatibility-rules)
6. [Error Codes](#error-codes)
7. [Error Reporting](#error-reporting)

---

## Overview

The Sema phase validates the AST for logical correctness and type consistency before bytecode generation. It consists of three components:

```
Interner → StringId (u32)
     ↓
SymbolTable → hierarchical type scopes
     ↓
Checker → collection of TypeErrors
```

The program is compiled only if the resulting `Vec<TypeError>` is empty.

---

## String Interner

**File:** `src/sema/interner.rs`

The `Interner` maps `&str → StringId(u32)`. It is the single source of truth for all string identities in the compiler. It is created during lexing/parsing and passed by reference to the checker and compiler.

```rust
Interner::intern("foo") → StringId(42)
Interner::lookup(StringId(42)) → "foo"
```

**Implementation:**

```rust
pub struct Interner {
    map:     HashMap<String, StringId>,
    strings: Vec<String>,
}
```

Each unique string is stored once in `strings: Vec<String>`. The rest of the pipeline works with numerical IDs, eliminating heap comparisons during type checking and compilation.

---

## Symbol Table

**File:** `src/sema/symbol_table.rs`

The `SymbolTable` manages variable bindings in nested scopes using a **chain of parent pointers**.

### Structure

```rust
pub struct SymbolTable<'a> {
    parent: Option<&'a SymbolTable<'a>>,  // ref to surrounding scope
    scopes: Vec<HashMap<String, Type>>,   // stack of scope frames (local)
    consts: Vec<HashSet<String>>,         // which names are const, per frame
}
```

### Creating a Child Scope

When entering a function or fiber body, a new `SymbolTable` is created with a reference to the surrounding table instead of cloning:

```rust
let mut func_symbols = SymbolTable::new_with_parent(symbols);
```

This is O(1) — only one empty `HashMap` is allocated for the new frame. Lookups traverse the parent chain as needed.

### Scope Lifecycle

| Method | Description |
|---|---|
| `enter_scope()` | Pushes a new frame — for `if`, `while`, `for` bodies |
| `exit_scope()` | Exits the current scope |
| `new_with_parent(parent)` | Creates a child table — for function/fiber bodies |
| `define(name, ty, is_const)` | Always writes to the **innermost** scope |
| `lookup(name)` | Traverses from inside out, then to the parent |
| `has_in_current_scope(name)` | Checks only the innermost frame — for redeclaration detection |
| `is_const(name)` | Checks if the owning scope marked the name as const |

### No Variable Shadowing

XCX **does not** support variable shadowing. Defining a variable that already exists in the **current scope** returns `RedefinedVariable`. Variables in parent scopes are accessible but cannot be redeclared in a child scope with the same name.

---

## Type Checker

**File:** `src/sema/checker.rs`

The `Checker` struct traverses the AST and accumulates `TypeError` values.

### Checker State

```rust
pub struct Checker<'a> {
    interner:          &'a Interner,
    loop_depth:        usize,
    functions:         HashMap<String, FunctionSignature>,
    fiber_context:     Option<Option<Type>>,  // None=outside fiber, Some(None)=void, Some(Some(T))=typed
    is_table_lambda:   bool,
    fiber_has_yield:   bool,
    in_yield_expr:     bool,
    last_expr_was_db_io: bool,
}
```

### Context Flags

| Field | Purpose |
|---|---|
| `loop_depth: usize` | Tracks nesting depth of `while`/`for`. Zero → `break`/`continue` are errors. Resets to 0 when entering a fiber body. |
| `fiber_context: Option<Option<Type>>` | `None` = not in fiber; `Some(None)` = void fiber; `Some(Some(T))` = typed fiber yielding `T` |
| `fiber_has_yield: bool` | Set when `yield` is encountered. Saved/restored by nested fiber definitions. |
| `is_table_lambda: bool` | Set inside `.where()` predicates; allows naked column names as identifiers via `__row_tmp` |
| `in_yield_expr: bool` | Tracks if we are inside a yield expression |
| `last_expr_was_db_io: bool` | Flag for database I/O operations |

### Pre-scan Pass

Before checking any statement body, the checker performs a **forward-declaration scan** of all `FunctionDef` and `FiberDef` nodes in the **current statement list** (non-recursively). This allows functions and fibers to be called before their definition in the source file (mutual recursion, call-before-declaration).

For each function/fiber found, the checker registers:
- `FunctionSignature { params: Vec<Type>, return_type: Option<Type>, is_fiber: bool }` in `self.functions`
- An entry in the `SymbolTable` with `Type::Unknown` (for functions) or `Type::Fiber(...)` (for fibers)

Built-in casting functions `i`, `f`, `s`, `b` are pre-registered.

### Type Inference Rules

- Expression types are inferred bottom-up from literals and propagated through operators.
- `Type::Unknown` acts as a wildcard — any operation with `Unknown` passes without error.
- `Type::Json` is compatible with any type in assignments and comparisons.
- Numeric promotion: `Int op Float → Float`.
- An empty array literal `[]` inherits its type from the assignment context if available.
- `Type::Table([])` (empty column list) is compatible with any `Table(cols)`.

---

## Type Compatibility Rules

The `is_compatible(expected, actual) -> bool` function:

| Rule | Description |
|---|---|
| Either is `Unknown` | Always compatible |
| Either is `Json` | Always compatible |
| `Int` ↔ `Float` | Mutually compatible (numeric promotion) |
| `Int` ↔ `Date` | Compatible (timestamps are integers) |
| `Set(X)` ↔ `Array(inner)` | Compatible when element type matches |
| `Set(N)` ↔ `Set(Z)` | Compatible (both integer-typed sets) |
| `Set(S)` ↔ `Set(C)` | Compatible (both string-typed sets) |
| `Table([])` ↔ `Table(cols)` | Compatible when either column list is empty |
| `Table(a)` ↔ `Table(b)` | Compatible when lengths match and column types match pairwise |
| `Map(k1,v1)` ↔ `Map(k2,v2)` | Recursively checked |
| `Fiber(None)` ↔ `Fiber(None)` | Both void fibers |
| `Fiber(Some(T))` ↔ `Fiber(Some(U))` | Compatible if T and U are compatible |

---

## Error Codes

| Code | Condition |
|---|---|
| `[S101] UndefinedVariable(name)` | Name used before declaration |
| `[S102] RedefinedVariable(name)` | Name declared twice in the same scope |
| `[S103] TypeMismatch { expected, actual }` | Expression type does not match expected type |
| `[S104] InvalidBinaryOp { op, left, right }` | Operator used with incompatible types |
| `[S105] ConstReassignment(name)` | Assignment to a `const` variable |
| `[S106] BreakOutsideLoop` | `break` outside of `while`/`for` |
| `[S107] ContinueOutsideLoop` | `continue` outside of `while`/`for` |
| `[S108] IndexAccessNotSupported(type)` | Indexing an unsupported type |
| `[S109] PropertyNotFound` | Property does not exist on type |
| `[S110] MethodNotFound` | Method does not exist on type |
| `[S111] InvalidArgumentCount` | Incorrect number of arguments |
| `[S208] YieldOutsideFiber` | `yield` used outside a fiber body |
| `[S209] FiberTypeMismatch` | `yield expr;` inside a void fiber (should be `yield;`) |
| `[S210] ReturnTypeMismatchInFiber` | Bare `return;` in a typed fiber (missing return value) |
| `[S211] CannotIterateOverVoidFiber` | Iterating over a void fiber |
| `[S212] CannotRunTypedFiber` | Calling `.run()` on a typed fiber |
| `[S301] WherePredicateNameCollision` | Local variable name conflicts with column name in `.where()` |
| `[S302] TableRowCountMismatch` | Table row has different number of columns than schema |
| `[D401] Rule violation` | `remove()` without `.where()` |
| `Other(msg)` | Miscellaneous contextual errors (arg count, unknown method, etc.) |

---

## Error Reporting

`TypeError` values carry a `Span { line, col, len }`. After checking, `main.rs` passes each error to `Reporter::error()`, which prints:

```
ERROR: [S101] Undefined variable: foo
   12 | i: x = foo + 1;
              ~~~
```

Compilation stops immediately after reporting errors — bytecode is not generated if any `TypeError` exists.

---

## Function Call Checking

For named function calls (`ExprKind::FunctionCall`):
1. Look first in `self.functions` (for declared functions/fibers)
2. If not found, look in the symbol table (for first-class function values, dynamic calls)
3. If the resolved signature is a fiber, the call returns `Type::Fiber(Some(ret))` (fiber instantiation, not direct call)
4. Additional arguments beyond the declared parameter count are checked but not rejected (variadic tolerance)
5. Unresolved calls add an `UndefinedVariable` error

---

## `.where()` Checking on Tables

When `table.where(predicate)` is checked:
1. A temporary scope is opened and `__row_tmp: Table(cols)` is defined
2. `is_table_lambda` is set to `true`
3. `collect_pred_idents()` collects all `Identifier` names used in the predicate
4. If any identifier exists in both the outer scope **and** matches a column name → `S301 WherePredicateNameCollision`
5. The predicate is checked; it must return `Bool`
6. The scope is closed and `is_table_lambda` is restored
