# XCX 2.2 Operators

## Arithmetic Operators

| Operator | Description                   | Example    | Result  |
|----------|-------------------------------|------------|---------|
| `+`      | Addition                      | `10 + 3`   | `13`    |
| `-`      | Subtraction / Unary Negation  | `10 - 3`   | `7`     |
| `*`      | Multiplication                | `10 * 3`   | `30`    |
| `/`      | Division (truncates for `i`)  | `10 / 3`   | `3`     |
| `%`      | Modulo (Remainder)            | `10 % 3`   | `1`     |
| `^`      | Exponentiation                | `2 ^ 10`   | `1024`  |
| `++`     | Integer Digit Concatenation   | `48 ++ 77` | `4877`  |

## String Operators

| Operator | Description           | Example                  | Result           |
|----------|-----------------------|--------------------------|------------------|
| `+`      | Concatenation         | `"Hi " + "User"`         | `"Hi User"`      |
| `HAS`    | Substring search      | `"Search" HAS "ear"`     | `true`           |

> [!TIP]
> String concatenation automatically converts numbers and booleans to strings (e.g., `"Value: " + 42` → `"Value: 42"`).

## Logical Operators

| Operator | Alias | Description |
|----------|-------|-------------|
| `AND`    | `&&`  | Logical AND |
| `OR`     | `||`  | Logical OR  |
| `NOT`    | `!!`  | Logical NOT |

## Comparison Operators

Used for comparing values of the same type.

| Operator | Description              |
|----------|--------------------------|
| `==`     | Equal to                 |
| `!=`     | Not equal to             |
| `<`      | Less than                |
| `>`      | Greater than             |
| `<=`     | Less than or equal to    |
| `>=`     | Greater than or equal to |

## Set Operators

| Word                   | Symbol | Description         |
|------------------------|--------|---------------------|
| `UNION`                | `∪`    | Set Union           |
| `INTERSECTION`         | `∩`    | Set Intersection    |
| `DIFFERENCE`           | `\`    | Set Difference      |
| `SYMMETRIC_DIFFERENCE` | `⊕`    | Symmetric Difference|

## Operator Precedence (Highest to Lowest)

1. Function calls, method calls, indexing `[]`
2. Unary negation `-`, `!!` (NOT)
3. Exponentiation `^`
4. Multiplication `*`, Division `/`, Modulo `%`
5. Set operations `UNION`, `INTERSECTION`, `DIFFERENCE`, `SYM_DIFF`
6. Addition `+`, Subtraction `-`, Concatenation `++`
7. Comparisons `HAS`, `>`, `<`, `>=`, `<=`
8. Equality `==`, `!=`
9. Logical `AND`, `&&`
10. Logical `OR`, `||`
