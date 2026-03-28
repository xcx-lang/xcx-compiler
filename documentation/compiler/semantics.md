//==compiler/semantics.md==\\
# XCX Semantic Analysis (Sema) — v2.2

The Sema phase validates the AST for logical correctness and type consistency before bytecode generation. It consists of two components: the **Symbol Table** and the **Type Checker**.

## String Interner (`src/sema/interner.rs`)

`Interner` maps `&str → StringId(u32)`. It is the single source of truth for all string identities in the compiler. Created during lexing/parsing, passed by reference to the checker and compiler.

```
Interner::intern("foo") → StringId(42)
Interner::lookup(StringId(42)) → "foo"
```

## Symbol Table (`src/sema/symbol_table.rs`)

The `SymbolTable` manages variable bindings across nested scopes using a **parent-pointer linked chain**.

### Structure

```rust
pub struct SymbolTable<'a> {
    parent: Option<&'a SymbolTable<'a>>,  // reference to enclosing scope
    scopes: Vec<HashMap<String, Type>>,   // stack of scope frames (local to this table)
    consts: Vec<HashSet<String>>,         // which names are const, per scope frame
}
```

### Creating a Child Scope

When entering a function or fiber body, a new `SymbolTable` is created with a reference to the enclosing table instead of cloning it:

```rust
let mut func_symbols = SymbolTable::new_with_parent(symbols);
```

This is O(1) — only a single empty `HashMap` is allocated for the new frame. Lookup travels up the parent chain as needed.

### Scope Lifecycle

- `enter_scope()` / `exit_scope()` push/pop local frames within a table — used for `if`, `while`, `for` bodies.
- `new_with_parent(parent)` creates a child table linked to the parent — used for function and fiber bodies.
- `define(name, ty, is_const)` always writes to the **innermost** (current) scope of the current table.
- `lookup(name)` walks local scopes from innermost to outermost, then follows the `parent` pointer chain.
- `has_in_current_scope(name)` checks only the innermost frame of the current table — used to detect redefinition within the same block.
- `is_const(name)` finds the scope that owns `name`, then checks `consts[scope_index]`.

### Important: No Variable Shadowing

XCX does **not** support variable shadowing. Defining a variable that already exists in the **current scope** raises `RedefinedVariable`. Variables in parent scopes are accessible but cannot be re-declared in a child scope using the same name.

## Type Checker (`src/sema/checker.rs`)

The `Checker` struct walks the AST and accumulates `TypeError` values. The program is only compiled if the resulting `Vec<TypeError>` is empty.

### Checker State

```rust
pub struct Checker<'a> {
    interner:       &'a Interner,
    loop_depth:     usize,
    functions:      HashMap<String, FunctionSignature>,
    fiber_context:  Option<Option<Type>>,
    is_table_lambda: bool,
    fiber_has_yield: bool,
}
```

### Pre-Scan Pass (`pre_scan_stmts`)

Before checking any statement body, the checker performs a **forward-declaration scan** of all `FunctionDef` and `FiberDef` nodes in the **current statement list** (not recursive). This allows functions and fibers to be called before their definition in the source file (mutual recursion, call before declare).

For each function/fiber found, the checker registers:
- A `FunctionSignature { params: Vec<Type>, return_type: Option<Type>, is_fiber: bool }` in `self.functions`
- An entry in the `SymbolTable` with `Type::Unknown` (for functions) or `Type::Fiber(...)` (for fibers)

Built-in cast functions `i`, `f`, `s`, `b` are pre-registered with `Type::Unknown` parameter and corresponding return types.

### Context Flags

| Field | Purpose |
|---|---|
| `loop_depth: usize` | Tracks nesting depth of `while`/`for`. Zero → `break`/`continue` are errors. Reset to 0 when entering a fiber body. |
| `fiber_context: Option<Option<Type>>` | `None` = not in fiber; `Some(None)` = void fiber; `Some(Some(T))` = typed fiber yielding `T`. |
| `fiber_has_yield: bool` | Set when a `yield` is encountered. Saved/restored across nested fiber definitions. |
| `is_table_lambda: bool` | Set inside `.where()` predicates; allows bare column names as identifiers by looking them up in `__row_tmp`. |

### Type Inference Rules

- Expression types are inferred bottom-up from literals and propagate through operators.
- `Type::Unknown` acts as a wildcard — any operation involving `Unknown` passes without error.
- `Type::Json` is compatible with any type in assignments and comparisons.
- Numeric promotion: `Int op Float → Float`.
- Empty array literal `[]` inherits type from assignment context if available.
- `Type::Table([])` (empty column list) is compatible with any `Table(cols)` — column info is propagated back into the variable's declared type after inference.

### `is_compatible(expected, actual) -> bool`

Key compatibility rules:

| Rule | Description |
|---|---|
| Either is `Unknown` | Always compatible |
| Either is `Json` | Always compatible |
| `Int` ↔ `Float` | Mutually compatible (numeric promotion) |
| `Int` ↔ `Date` | Compatible (timestamps are integers) |
| `Set(X)` ↔ `Array(inner)` | Compatible when inner element type matches |
| `Set(N)` ↔ `Set(Z)` | Compatible (both integer-typed sets) |
| `Set(S)` ↔ `Set(C)` | Compatible (both string-typed sets) |
| `Table([])` ↔ `Table(cols)` | Compatible when either column list is empty |
| `Table(a)` ↔ `Table(b)` | Compatible when lengths match and all column types are pairwise compatible |
| `Map(k1,v1)` ↔ `Map(k2,v2)` | Recursively checked |
| `Fiber(None)` ↔ `Fiber(None)` | Both void fibers |
| `Fiber(Some(T))` ↔ `Fiber(Some(U))` | Compatible iff T and U are compatible |

### Checking Function Calls

For named function calls (`ExprKind::FunctionCall`):
1. Look up in `self.functions` first (for declared functions/fibers).
2. If not found, look up in the symbol table (for first-class function values, dynamic calls).
3. If the resolved signature is a fiber, the call returns `Type::Fiber(Some(ret))` (fiber instantiation, not direct invocation).
4. Extra arguments beyond the declared parameter count are checked but not rejected (variadic tolerance).
5. Unresolved calls push `UndefinedVariable`.

For statement-level function calls (`StmtKind::FunctionCallStmt`):
- Argument count mismatch → `Other("Function expects N arguments, got M")`.
- Unknown name not in symbol table → `UndefinedVariable`.

### Checking Fiber Declarations (`FiberDecl`)

1. Look up `fiber_name` in `self.functions` (preferred) or symbol table.
2. Verify the resolved signature has `is_fiber: true`.
3. Check argument types against parameters.
4. Define the new variable with `Type::Fiber(inner_type)`.

### Checking `.where()` on Tables

When checking `table.where(predicate)`:
1. A temporary scope is opened and `__row_tmp: Table(cols)` is defined.
2. `is_table_lambda` is set to `true`.
3. `collect_pred_idents()` collects all `Identifier` names used in the predicate.
4. If any identifier both exists in the outer scope **and** matches a column name → `S301 WherePredicateNameCollision`.
5. The predicate is type-checked; it must return `Bool`.
6. Scope is exited and `is_table_lambda` is restored.

### Checking `Table.join()`

The checker merges column definitions: left table columns plus right table columns not already present (by `StringId` equality). If a column name conflicts, it is kept from the right table. The result type is `Type::Table(combined_cols)`.

### Checking `for` Loops

The `iter_type` field of `ForIterType` is mutated during checking based on the inferred type of `start`:
- `Type::Array(_)` → sets `ForIterType::Array`, loop variable gets inner type
- `Type::Set(st)` → sets `ForIterType::Set`, loop variable gets the set's element type
- `Type::Table(cols)` → treated as `ForIterType::Array`, loop variable gets `Type::Table(cols)`
- `Type::Fiber(inner)` → sets `ForIterType::Fiber`, loop variable gets inner yield type
- `Type::Int` (with `to` keyword) → stays `ForIterType::Range`, loop variable is `Int`

---

## Validated Error Codes

| Code | Condition |
|---|---|
| `UndefinedVariable(name)` | Name used before declaration |
| `RedefinedVariable(name)` | Name declared twice in the same scope |
| `ConstReassignment(name)` | Assignment to a `const` variable |
| `TypeMismatch { expected, actual }` | Expression type does not match expected |
| `InvalidBinaryOp { op, left, right }` | Operator used with incompatible types |
| `BreakOutsideLoop` | `break` outside `while`/`for` |
| `ContinueOutsideLoop` | `continue` outside `while`/`for` |
| `[S208] YieldOutsideFiber` | `yield` used outside any fiber body |
| `[S209] FiberTypeMismatch` | `yield expr;` inside a void fiber (should be `yield;`) |
| `[S210] ReturnTypeMismatchInFiber` | Bare `return;` in a typed fiber (missing return value) |
| `[S301] WherePredicateNameCollision` | Local variable name conflicts with a table column name in `.where()` |
| `Other(msg)` | Various contextual errors (argument count, unknown method, void fiber iteration, etc.) |

---

## Error Reporting

`TypeError` values carry a `Span { line, col, len }`. After checking, `main.rs` passes each error to `Reporter::error()`, which prints:
1. `ERROR: <message>`
2. The relevant source line with line number
3. A `~~~` underline starting at `col` with length `len`

Compilation halts immediately after error reporting — no bytecode is generated if any `TypeError` exists.