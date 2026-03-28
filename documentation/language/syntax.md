# XCX 2.2 Syntax Basics

## Comments

```xcx
--- Single-line comment
i: price = 100;  --- inline comment

---
Multi-line
comment block.
*---
```

## Identifiers

Identifiers are **case-sensitive** and must match the pattern `[a-zA-Z][a-zA-Z0-9_]*`.

| Valid         | Invalid     | Reason                       |
|---------------|-------------|------------------------------|
| `userData`    | `1stUser`    | Cannot start with a digit    |
| `user_data`   | `user-data`  | Hyphen is a minus operator    |
| `counter1`    | `user name`  | Spaces are not allowed       |

Keywords (`if`, `func`, `fiber`, `i`, `f`, `s`, `b`, etc.) are reserved.

## Instruction Terminals

Statements terminate with a semicolon `;`. This applies to variable declarations, function calls, assignments, and directives.

```xcx
i: x = 10;
>! "Hello";
```

## Blocks

Blocks are opened by a keyword (e.g., `then`, `do`, `{`) and closed by `end;`.

```xcx
if (x > 0) then;
    >! "Positive";
end;
```
