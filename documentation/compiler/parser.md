//==compiler/parser.md==\\
# XCX Parser — v2.2

The XCX Parser transforms the token stream into a high-level Abstract Syntax Tree (AST).

## Architecture: Pratt Parsing

XCX uses a **Pratt Parser** (Top-Down Operator Precedence).

- **File**: `src/parser/pratt.rs`
- **Lookahead**: One token (`current` + `peek`), advanced manually with `advance()`.
- **Error Recovery**: On syntax error, `synchronize()` skips tokens until the next semicolon or a known statement-starting keyword (`func`, `fiber`, `if`, `for`, `const`, `return`, `>!`, etc.).

The `Parser` struct borrows the source string for the lifetime `'a`, and `Scanner<'a>` is parameterised by the same lifetime, reflecting the byte-slice-based scanner.

### Precedence Levels (lowest → highest)

| Level | Operators |
|---|---|
| `Lowest` | — |
| `Lambda` | `->` |
| `Assignment` | `=` |
| `LogicalOr` | `OR`, `\|\|` |
| `LogicalAnd` | `AND`, `&&` |
| `Equals` | `==`, `!=` |
| `LessGreater` | `>`, `<`, `>=`, `<=`, `HAS` |
| `Sum` | `+`, `-`, `++` |
| `SetOp` | `UNION`, `INTERSECTION`, `DIFFERENCE`, `SYMMETRIC_DIFFERENCE` |
| `Product` | `*`, `/`, `%` |
| `Power` | `^` |
| `Prefix` | `-x` |
| `Call` | `.`, `[` |

## Statement Dispatch

`parse_statement_internal()` dispatches on the current token:

- **Type keywords** (`i`, `f`, `s`, `b`, `array`, `set`, `map`, `date`, `table`, `json`) → `parse_var_decl()`, or `parse_assignment()` if followed by `=`
- **`const`** → `parse_var_decl()` with `is_const = true`
- **`var`** (identifier) → type-inferred variable declaration
- **`>!`** → `parse_print_stmt()`
- **`>?`** → `parse_input_stmt()`
- **`halt`** → `parse_halt_stmt()`
- **`if`** → `parse_if_statement()`
- **`while`** → `parse_while_statement()`
- **`for`** → `parse_for_statement()`
- **`break`** / **`continue`** → `parse_break_statement()` / `parse_continue_statement()`
- **`func`** → `parse_func_def()`
- **`fiber`** → `parse_fiber_statement()` (dispatches to def or decl based on peek)
- **`return`** → `parse_return_stmt()`
- **`yield`** → `parse_yield_stmt()` (handles `yield expr`, `yield from expr`, and `yield;`)
- **`@wait`** → `parse_wait_stmt()`
- **`serve`** → `parse_serve_stmt()`
- **`net`** → `parse_net_stmt()`
- **`include`** → `parse_include_stmt()`
- **Identifier + `=`** → `parse_assignment()`
- **Identifier + `(`** → `parse_func_call_stmt()`
- **Anything else** → `parse_expr_stmt()`

## Function Definition Styles

XCX supports two syntactically different styles for defining functions:

**Brace style** (C-like):
```xcx
func name(i: x, s: y -> i) {
    return x + 1;
}
```

**XCX style** (keyword block):
```xcx
func:i: name(i: x, s: y) do;
    return x + 1;
end;
```

Both produce identical `StmtKind::FunctionDef` AST nodes. The return type in brace style is declared with `-> type` inside the parameter list or after `)`.

## Fiber Statements

`parse_fiber_statement()` looks at `peek` to decide:
- `peek == Colon` → `parse_fiber_decl()` (instantiation: `fiber:T: varname = fiberDef(args);`)
- otherwise → `parse_fiber_def()` (definition: `fiber name(params) { body }`)

`parse_fiber_decl()` also handles the case where, after parsing the type and name, the current token is `(` — in that case it pivots to `finish_fiber_def()` (a definition with a leading `fiber:` type annotation).

## Key Constructs Parsed

- **Variable declarations**: `i: name = expr;`, `const s: NAME = expr;`, `var name = expr;`
- **Control flow**: `if (cond) then; ... elseif (cond) then; ... else; ... end;`
- **While loop**: `while (cond) do; ... end;`
- **For loop**: `for x in expr do; ... end;` and `for x in start to end @step n do; ... end;`
- **Functions**: `func` (two styles, see above)
- **Fibers**: `fiber name(params) { body }` and `fiber:T: varname = fiberName(args);`
- **Yield**: `yield expr;`, `yield from expr;`, `yield;`
- **HTTP**: `serve: name { port=..., routes=... };`, `net.get(url)`, `net.request { ... } as resp;`, `net.respond(status, body);`
- **Collections**: Array `[a, b, c]`, Set `set:N { 1,,10 }`, Map `[k :: v, ...]`, Table `table { columns=[...] rows=[...] }`
- **Raw blocks**: `<<<...>>>` for inline JSON/strings
- **Include**: `include "path";` or `include "path" as alias;`
- **I/O**: `>! expr;` (print), `>? varname;` (input)
- **Halt**: `halt.alert >! msg;`, `halt.error >! msg;`, `halt.fatal >! msg;`
- **Wait**: `@wait(ms);` or `@wait ms;`
- **Date literals**: `date("2024-01-01")` or `date("01/01/2024", "DD/MM/YYYY")`

## Expression Parsing

`parse_expression(precedence)` calls `parse_prefix()` for the left-hand side, then loops calling `parse_infix(left)` while the peek token's precedence exceeds the current minimum.

Key prefix parsers:
- **Identifiers**: If followed by `(`, parsed as a `FunctionCall`; otherwise as an `Identifier`.
- **Literals**: `IntLiteral`, `FloatLiteral`, `StringLiteral`, `True`, `False`
- **Unary minus**: Parsed as `Binary { left: IntLiteral(0), op: Minus, right }` (no separate `Unary::Neg`)
- **`not` / `!`**: `Unary { op: Not/Bang, right }`
- **`(`...`)` groups**: Single expression → unwrapped; multiple comma-separated → `Tuple`
- **`[`...`]`**: If first element followed by `::`, parsed as a `MapLiteral`; otherwise `ArrayLiteral`
- **`{`...`}`**: Parsed as `ArrayOrSetLiteral` (type resolved at semantic or compile time)
- **`set:N { }` etc.**: Explicit `SetLiteral` with known `SetType`
- **`map { schema=[...] data=[...] }`**: Explicit `MapLiteral`
- **`table { columns=[...] rows=[...] }`**: `TableLiteral`
- **`random.choice from expr`**: `RandomChoice`
- **`date(...)`**: `DateLiteral`
- **`net.get/post/put/delete/patch(...)` etc.**: `NetCall` or `NetRespond`
- **`<<<...>>>`**: `RawBlock`
- **`.terminal!cmd`**: `TerminalCommand`

Key infix parsers:
- **`.`**: `parse_dot_infix` — produces `MethodCall` if followed by `(`, else `MemberAccess`; also handles `.[key]` index access
- **`[`**: `parse_index_infix` → `Index`
- **`->`**: `parse_lambda_infix` → `Lambda`
- **All binary operators**: `Binary { left, op, right }`

## `parse_expr_stmt()` Post-Processing

After parsing a full expression statement, `parse_expr_stmt()` checks if the result is a `MethodCall`:
- Method name `bind` with 2 args and second arg is `Identifier` → rewrite as `StmtKind::JsonBind`
- Method name `inject` with 2 args → rewrite as `StmtKind::JsonInject`

This allows the sugar syntax `json.bind("path", target);` and `json.inject(mapping, table);` at the statement level.

## Expander (`src/parser/expander.rs`)

The Expander runs **after** parsing, **before** semantic analysis. It is a separate tree-rewriting pass.

### Responsibilities

**Include resolution**: `include "file.xcx";` is replaced by the inlined AST of that file. Circular dependencies are detected via `visiting_files: HashSet<PathBuf>`. Files are deduplicated via `included_files: HashSet<PathBuf>` (each file included only once unless aliased).

**Alias prefixing**: `include "math.xcx" as math;` causes all top-level names from that file to be renamed to `math.name`. Call sites (`math.sin(x)`) are rewritten from `MethodCall` to `FunctionCall { name: "math.sin" }` by `expand_expr_inplace`. The `prefix_program()` / `prefix_stmt_impl()` / `prefix_expr_impl()` functions walk the entire sub-AST renaming all references to top-level symbols.

**Fiber name prefixing**: `FiberDecl::fiber_name` references are also prefixed so that instantiations of renamed fibers resolve correctly after prefixing.

**YieldFrom prefixing**: `StmtKind::YieldFrom` expressions are traversed so that fiber constructor calls inside `yield from` are also renamed.

**Protected names** (never prefixed): `json`, `date`, `store`, `halt`, `terminal`, `net`, `env`, `crypto`, `EMPTY`, `math`, `random`, `i`, `f`, `s`, `b`, `from`, `main`.

**Include path search order**:
1. Relative to the current file's directory
2. In the `lib/` directory (relative to CWD, then walking up from the executable's path)

## AST Definitions (`src/parser/ast.rs`)

### `Expr` — Expression nodes

| Variant | Description |
|---|---|
| `IntLiteral(i64)` | Integer constant |
| `FloatLiteral(f64)` | Float constant |
| `StringLiteral(StringId)` | Interned string |
| `BoolLiteral(bool)` | `true` / `false` |
| `Identifier(StringId)` | Variable or function name |
| `Binary { left, op, right }` | Binary operation |
| `Unary { op, right }` | Unary operation (`not`, `!`) |
| `FunctionCall { name, args }` | Function call by interned name |
| `MethodCall { receiver, method, args }` | Dot-call on a value |
| `MemberAccess { receiver, member }` | Dot-access without call |
| `Index { receiver, index }` | Bracket index `a[i]` |
| `Lambda { params, return_type, body }` | Arrow lambda `x -> expr` |
| `ArrayLiteral { elements }` | Explicit `[a, b, c]` |
| `ArrayOrSetLiteral { elements }` | Ambiguous `{a, b, c}` — resolved later |
| `SetLiteral { set_type, elements, range }` | Typed set with optional range |
| `MapLiteral { key_type, value_type, elements }` | Map literal |
| `TableLiteral { columns, rows }` | Table literal |
| `DateLiteral { date_string, format }` | `date("2024-01-01")` |
| `Tuple(Vec<Expr>)` | Parenthesised comma-separated list |
| `NetCall { method, url, body }` | HTTP call expression |
| `NetRespond { status, body, headers }` | HTTP respond expression |
| `RawBlock(StringId)` | `<<<...>>>` raw content |
| `TerminalCommand(cmd, arg)` | `.terminal !cmd` |
| `RandomChoice { set }` | `random.choice from set` |

### `Stmt` — Statement nodes

Key variants: `VarDecl`, `Assign`, `Print`, `Input`, `If`, `While`, `For`, `Break`, `Continue`, `FunctionDef`, `FiberDef`, `FiberDecl`, `Return`, `Yield`, `YieldFrom`, `YieldVoid`, `Include`, `Serve`, `NetRequestStmt`, `JsonBind`, `JsonInject`, `Halt`, `Wait`, `ExprStmt`, `FunctionCallStmt`.

### `Type` — Type system

`Int`, `Float`, `String`, `Bool`, `Date`, `Json`, `Array(Box<Type>)`, `Set(SetType)`, `Map(Box<Type>, Box<Type>)`, `Table(Vec<ColumnDef>)`, `Fiber(Option<Box<Type>>)`, `Builtin(StringId)`, `Unknown`.

`SetType` variants: `N` (Natural), `Z` (Integer), `Q` (Rational/Float), `S` (String), `C` (Char/String), `B` (Boolean).

### `ForIterType`

`Range` (numeric `start to end`), `Array`, `Set`, `Fiber` — set by the type checker and used by the compiler to emit the correct loop pattern.

### `ColumnDef`

```rust
pub struct ColumnDef {
    pub name:    StringId,
    pub ty:      Type,
    pub is_auto: bool,    // @auto columns are auto-incremented on insert
}
```

## String Interner

All string values (identifiers, string literals, method names) are interned via `Interner` into `StringId (u32)`. The interner is created in the parser and passed through all subsequent phases. This means the checker, compiler, and VM all use numeric IDs for name comparisons instead of `String` comparisons.