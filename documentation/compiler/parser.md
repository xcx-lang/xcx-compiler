# XCX Parser — Documentation

> **File:** `src/parser/pratt.rs`  
> **Technique:** Pratt Parser (Top-Down Operator Precedence)

---

## Table of Contents

1. [Overview](#overview)
2. [Precedence Levels](#precedence-levels)
3. [Instruction Dispatch](#instruction-dispatch)
4. [Function Definition Styles](#function-definition-styles)
5. [Fiber Instructions](#fiber-instructions)
6. [Expression Parsing](#expression-parsing)
7. [AST Nodes — Expr](#ast-nodes--expr)
8. [AST Nodes — Stmt](#ast-nodes--stmt)
9. [Type System](#type-system)
10. [Expr Stmt Post-processing](#expr-stmt-post-processing)

---

## Overview

The XCX Parser uses the **Pratt** algorithm (Top-Down Operator Precedence).

- **File:** `src/parser/pratt.rs`
- **Lookahead:** One token (`current` + `peek`), manual advancement via `advance()`
- **Error Recovery:** `synchronize()` skips tokens until the next semicolon or a keyword starting a statement

The `Parser` struct borrows the source string for lifetime `'a`, and the `Scanner<'a>` is parameterized with the same lifetime.

---

## Precedence Levels

From lowest to highest:

| Level | Operators |
|---|---|
| `Lowest` | — |
| `Lambda` | `->` |
| `Assignment` | `=` |
| `LogicalOr` | `OR`, `||` |
| `LogicalAnd` | `AND`, `&&` |
| `Equals` | `==`, `!=` |
| `LessGreater` | `>`, `<`, `>=`, `<=`, `HAS` |
| `Sum` | `+`, `-`, `++` |
| `SetOp` | `UNION`, `INTERSECTION`, `DIFFERENCE`, `SYMMETRIC_DIFFERENCE` |
| `Product` | `*`, `/`, `%` |
| `Power` | `^` |
| `Prefix` | `-x` |
| `Concatenation` | `::` |
| `Call` | `.`, `[` |
| `AsPrec` | `as` |

---

## Instruction Dispatch

`parse_statement_internal()` dispatches based on the current token:

| Token | Parser |
|---|---|
| Type Keywords (`i`, `f`, `s`, `b`, `array`, ...) | `parse_var_decl()` or `parse_assignment()` |
| `const` | `parse_var_decl()` with `is_const = true` |
| `var` (identifier) | Variable declaration with type inference |
| `>!` | `parse_print_stmt()` |
| `>?` | `parse_input_stmt()` |
| `halt` | `parse_halt_stmt()` |
| `if` | `parse_if_statement()` |
| `while` | `parse_while_statement()` |
| `for` | `parse_for_statement()` |
| `break` / `continue` | `parse_break_statement()` / `parse_continue_statement()` |
| `func` | `parse_func_def()` |
| `fiber` | `parse_fiber_statement()` |
| `return` | `parse_return_stmt()` |
| `yield` | `parse_yield_stmt()` |
| `@wait` | `parse_wait_stmt()` |
| `serve` | `parse_serve_stmt()` |
| `net` | `parse_net_stmt()` |
| `include` | `parse_include_stmt()` |
| Identifier + `=` | `parse_assignment()` |
| Identifier + `(` | `parse_func_call_stmt()` |

---

## Function Definition Styles

XCX supports two syntactically different function definition styles:

### Curly Brace Style (C-like)

```xcx
func name(i: x, s: y -> i) {
    return x + 1;
}
```

### XCX Style (Keyword block)

```xcx
func:i: name(i: x, s: y) do;
    return x + 1;
end;
```

Both styles produce identical `StmtKind::FunctionDef` AST nodes. The return type in the curly brace style is declared via `-> type` inside the parameter list or after the `)`.

---

## Fiber Instructions

`parse_fiber_statement()` checks the `peek` to decide:
- `peek == Colon` → `parse_fiber_decl()` (instantiation: `fiber:T: varname = fiberDef(args);`)
- otherwise → `parse_fiber_def()` (definition: `fiber name(params) { body }`)

`parse_fiber_decl()` also handles the case where after parsing the type and name, the current token is `(` — in which case it pivots to `finish_fiber_def()`.

### Fiber Definition

```xcx
fiber myFiber(i: x) {
    yield x;
    yield x + 1;
}
```

### Fiber Instantiation

```xcx
fiber:i: f = myFiber(10);
```

### Yield

```xcx
yield expr;       // yield with value
yield from expr;  // delegate to another fiber
yield;            // void yield
```

---

## Expression Parsing

`parse_expression(precedence)` calls `parse_prefix()` for the left side, then loops calling `parse_infix(left)` as long as the peek token's precedence exceeds the current minimum.

### Prefix Parsers (Selection)

| Expression | AST Result |
|---|---|
| Identifier | `Identifier` or `FunctionCall` (if followed by `(`) |
| `IntLiteral`, `FloatLiteral`, `StringLiteral` | Corresponding literal node |
| `-x` (unary minus) | `Binary { left: IntLiteral(0), op: Minus, right }` |
| `not` / `!` | `Unary { op: Not/Bang, right }` |
| `(expr)` | Unwrapped expression or `Tuple` (multiple el.) |
| `[a, b, c]` | `ArrayLiteral` or `MapLiteral` (if containing `::`) |
| `{a, b, c}` | `ArrayOrSetLiteral` (type resolved semantically) |
| `set:N { 1,,10 }` | `SetLiteral` with type and optional range |
| `table { ... }` | `TableLiteral` |
| `random.choice from expr` | `RandomChoice` |
| `random.int(min, max)` | `RandomInt` |
| `random.float(min, max)` | `RandomFloat` |
| `date("2024-01-01")` | `DateLiteral` |
| `net.get/post/...(url)` | `NetCall` |
| `net.respond(status, body)` | `NetRespond` |
| `<<<...>>>` | `RawBlock` |
| `.terminal!cmd` | `TerminalCommand` |
| `#tag` | `Tag` |

### Infix Parsers (Selection)

| Operator | AST Result |
|---|---|
| `.method(args)` | `MethodCall` |
| `.member` | `MemberAccess` |
| `.[key]` | `Index` |
| `[index]` | `Index` |
| `->` | `Lambda` |
| `as name` | `As` |
| All binary operators | `Binary { left, op, right }` |

---

## AST Nodes — Expr

Defined in `src/parser/ast.rs` as the `ExprKind` enum:

```
ExprKind
├── Literals
│   ├── IntLiteral(i64)
│   ├── FloatLiteral(f64)
│   ├── StringLiteral(StringId)
│   ├── BoolLiteral(bool)
│   └── DateLiteral { date_string, format }
├── Identifiers and Access
│   ├── Identifier(StringId)
│   ├── MemberAccess { receiver, member }
│   └── Index { receiver, index }
├── Operations
│   ├── Binary { left, op, right }
│   └── Unary { op, right }
├── Calls
│   ├── FunctionCall { name, args }
│   ├── MethodCall { receiver, method, args, wait_after }
│   └── Lambda { params, return_type, body }
├── Collections
│   ├── ArrayLiteral { elements }
│   ├── ArrayOrSetLiteral { elements }
│   ├── SetLiteral { set_type, elements, range }
│   ├── MapLiteral { key_type, value_type, elements }
│   └── TableLiteral { columns, rows }
├── Networking
│   ├── NetCall { method, url, body }
│   └── NetRespond { status, body, headers }
├── Fiber
│   └── Yield(expr)
├── Randomness
│   ├── RandomChoice { set }
│   ├── RandomInt { min, max, step }
│   └── RandomFloat { min, max, step }
└── Other
    ├── RawBlock(StringId)
    ├── TerminalCommand(cmd, args)
    ├── Tuple(Vec<Expr>)
    ├── As { expr, name }
    └── Tag(StringId)
```

---

## AST Nodes — Stmt

Key `StmtKind` variants:

| Variant | Description |
|---|---|
| `VarDecl { is_const, ty, name, value }` | Variable declaration |
| `Assign { name, value }` | Assignment |
| `Print(expr)` | `>! expr;` statement |
| `Input(name, ty)` | `>? var;` statement |
| `If { condition, then_branch, else_ifs, else_branch }` | Conditional statement |
| `While { condition, body }` | While loop |
| `For { var_name, start, end, step, body, iter_type }` | For loop |
| `Break` / `Continue` | Loop control |
| `FunctionDef { name, params, return_type, body }` | Function definition |
| `FiberDef { name, params, return_type, body }` | Fiber definition |
| `FiberDecl { inner_type, name, fiber_name, args }` | Fiber instantiation |
| `Return(Option<Expr>)` | Return statement |
| `Yield(expr)` | Yield statement |
| `YieldFrom(expr)` | `yield from` delegation |
| `YieldVoid` | `yield;` without value |
| `Include { path, alias }` | Include directive |
| `Serve { name, port, host, workers, routes }` | HTTP server |
| `NetRequestStmt { ... }` | `net.request { }` statement |
| `JsonBind { json, path, target }` | `json.bind(...)` assignment |
| `JsonInject { json, mapping, table }` | `json.inject(...)` |
| `Halt { level, message }` | Halt with level |
| `Wait(expr)` | `@wait(ms)` |

---

## Type System

`Type` enum in `src/parser/ast.rs`:

```
Type
├── Int          — 48-bit integer (NaN-boxed)
├── Float        — 64-bit floating-point
├── String       — UTF-8
├── Bool         — true/false
├── Date         — ms timestamp
├── Json         — any JSON value
├── Array(Box<Type>)
├── Set(SetType) — N/Q/Z/S/C/B
├── Map(Box<Type>, Box<Type>)
├── Table(Vec<ColumnDef>)
├── Database
├── Fiber(Option<Box<Type>>)  — None = void
├── Builtin(StringId)          — json, date, store, etc.
└── Unknown                    — acts as a wildcard
```

`SetType` variants: `N` (Natural), `Z` (Integer), `Q` (Rational/Float), `S` (String), `C` (Char/String), `B` (Boolean).

### ColumnDef

```rust
pub struct ColumnDef {
    pub name:       StringId,
    pub ty:         Type,
    pub attributes: Vec<ColumnAttribute>,
}
```

Column attributes: `Auto` (`@auto`), `PrimaryKey` (`@pk`), `Unique` (`@unique`), `Optional` (`@optional`), `Default(Expr)` (`@default(val)`), `ForeignKey(table, col)` (`@fk(Table.col)`).

### ForIterType

Set by the type checker during semantic analysis:

| Variant | Description |
|---|---|
| `Range` | Numeric `start to end` |
| `Array` | Iteration over array |
| `Set` | Iteration over set |
| `Fiber` | Iteration over fiber |

---

## Expr Stmt Post-processing

After parsing a full statement expression, `parse_expr_stmt()` checks if the result is a `MethodCall`:

- Method name `bind` with 2 arguments, where the second is an `Identifier` → rewritten as `StmtKind::JsonBind`
- Method name `inject` with 2 arguments → rewritten as `StmtKind::JsonInject`

This enables the syntactic sugar `json.bind("path", target);` and `json.inject(mapping, table);` at the statement level.

---

## Language Constructs (Syntax)

```xcx
--- Variables
i: age = 25;
const s: NAME = "Alice";
var x = 42;

--- Flow Control
if (cond) then;
    ...
elseif (cond2) then;
    ...
else;
    ...
end;

while (cond) do;
    ...
end;

for x in 1 to 10 do;
    ...
end;

for item in myArray do;
    ...
end;

--- Functions
func:i: add(i: a, i: b) do;
    return a + b;
end;

--- HTTP
serve: myServer {
    port = 8080,
    routes = [["GET /api" :: handler]]
};

net.get("https://api.example.com") as resp;

--- Collections
array:i arrOfInts = [1, 2, 3];
set:N mySet = set:N { 1,,100 };
map:s<->i scores = [s::v :: ["Alice" :: 100]];

--- JSON
json: data = <<<{ "name": "Alice" }>>>;
data.bind("/name", nameVar);
```
