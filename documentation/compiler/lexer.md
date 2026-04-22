# XCX Lexer (Scanner) — Documentation

> **File:** `src/lexer/scanner.rs`  
> **Technique:** Manual, eager byte-by-byte scanning on `&[u8]`

---

## Table of Contents

1. [Overview](#overview)
2. [Scanner Structure](#scanner-structure)
3. [API](#api)
4. [Token Types](#token-types)
5. [Special Scanning Features](#special-scanning-features)
6. [String Interning](#string-interning)

---

## Overview

The Lexer is responsible for converting a raw stream of source bytes into a stream of discrete tokens. It works on `&[u8]` (a reference to the original source bytes) without allocating a `Vec<char>`. Unicode handling is performed only where necessary.

---

## Scanner Structure

```rust
pub struct Scanner<'a> {
    source:   &'a [u8],   // borrowed source reference — no allocation
    pos:      usize,      // byte position
    char_pos: usize,      // Unicode character position (for Span.col)
    line:     usize,
    col:      usize,
}
```

`char_pos` is incremented only for bytes that are **not** UTF-8 continuation bytes (`10xxxxxx`), ensuring that the Unicode character count remains correct for Span reporting without decoding every character.

---

## API

```rust
// Main method — called on demand by the parser (not an iterator)
pub fn next_token(&mut self, interner: &mut Interner) -> Token
```

**Lookahead:**
- Single-byte: `peek()`
- Two-byte: `peek_next()` / `peek_at(offset)`

---

## Token Types

Tokens are defined in `src/lexer/token.rs` as the `TokenKind` enum. Each `Token` carries a `Span { line, col, len }` for error reporting. `len` is measured in Unicode characters (via `char_pos` deltas), not in bytes.

### Literals

| Variant | Description |
|---|---|
| `IntLiteral(i64)` | Integer constant |
| `FloatLiteral(f64)` | Floating-point constant |
| `StringLiteral(StringId)` | Interned string |
| `True` / `False` | Boolean literals |

### Type Keywords

| Token | XCX Type |
|---|---|
| `TypeI` | `i` — Integer (48-bit) |
| `TypeF` | `f` — Float (64-bit) |
| `TypeS` | `s` — String (UTF-8) |
| `TypeB` | `b` — Boolean |
| `Array`, `Set`, `Map` | Collection types |
| `Table`, `Json`, `Date` | Complex types |
| `Fiber` | Coroutine |
| `TypeSetN/Q/Z/S/B/C` | Set types (Natural/Rational/Integer/String/Bool/Char) |

### Control Flow

`If`, `Then`, `ElseIf`, `Else`, `End`, `While`, `Do`, `For`, `In`, `To`, `Break`, `Continue`

### Functions and Fiber

`Func`, `Return`, `Fiber`, `Yield`

### Operators

| Token | Symbol |
|---|---|
| `Plus` | `+` |
| `PlusPlus` | `++` (int concatenation) |
| `Minus` | `-` |
| `Star` | `*` |
| `Slash` | `/` |
| `Caret` | `^` (exponentiation) |
| `Has` | `HAS` |
| `And`, `Or`, `Not` | `AND`/`&&`, `OR`/`||`, `NOT`/`!!` |

### Set Operators

| Token | Symbol | Unicode |
|---|---|---|
| `Union` | `UNION` | `∪` |
| `Intersection` | `INTERSECTION` | `∩` |
| `Difference` | `DIFFERENCE` / `\` | — |
| `SymDifference` | `SYMMETRIC_DIFFERENCE` | `⊕` |

### Special Punctuation

| Token | Symbol | Use Case |
|---|---|---|
| `GreaterBang` | `>!` | Print |
| `GreaterQuestion` | `>?` | Input |
| `DoubleColon` | `::` | Key-value pair in a map |
| `DoubleComma` | `,,` | Set range (`set:N { 1,,10 }`) |
| `Bridge` | `<->` | Map type separator |

### Built-ins and Specials

`Net`, `Serve`, `Store`, `Halt`, `Terminal`, `Json`, `Date`, `Random`

### Specials

| Token | Description |
|---|---|
| `RawBlock(StringId)` | `<<<...>>>` content |
| `AtStep` | `@step` |
| `AtAuto` | `@auto` (auto-increment column) |
| `AtWait` | `@wait` |
| `AtPk` | `@pk` (primary key) |
| `Tag(StringId)` | `#tag` token |

---

## Special Scanning Features

### Raw Blocks
Delimited by `<<<` and `>>>`. Everything between them is captured as a single `RawBlock(StringId)` token, used for inline JSON or multi-line data.

```
<<<
  { "key": "value" }
>>>
```

### Comments
XCX uses `---` as a comment delimiter:

- **Single-line:** `--- this is a comment` (content other than a space on the same line after `---`)
- **Multi-line:** `---` followed only by spaces until the end of the line opens a block, closed by `*---`

```xcx
--- Single-line comment

---
This is a multi-line
comment
*---
```

### Unicode Set Operators
The scanner recognizes Unicode symbols via `starts_with` on their UTF-8 byte sequences. For multi-byte operators, `advance()` is called the appropriate number of additional times to consume the remaining continuation bytes.

### Else/ElseIf Disambiguation
After recognizing `else`/`els`, the scanner peeks forward to see if the next word is `if`. If so, it collapses the two words into a single `ElseIf` token. The saved position (`after_ws_pos`) allows backtracking if the next word is not `if`.

### `@` Directives
Tokens starting with `@` are scanned by consuming ASCII alpha bytes and matching the result:
- `@step` → `AtStep`
- `@auto` → `AtAuto`
- `@wait` → `AtWait`
- `@pk` → `AtPk`
- `@unique` → `AtUnique`
- `@optional` → `AtOptional`
- `@default` → `AtDefault`
- `@fk` → `AtFk`

### Dot-Dot (`..`) → `To`
Two consecutive dots `..` are scanned as a `To` token (used in range expressions), distinguishing them from a single `.` (`Dot`).

### Double-Comma (`,,`) → `DoubleComma`
Two consecutive commas `,,` are scanned as `DoubleComma` (used in set range literals: `set:N { 1,,10 }`).

### Identifier Scanning
`identifier()` captures a continuous sequence of ASCII alphanumeric bytes, underscores, and bytes `>= 128` (for UTF-8 identifiers). The captured byte range is converted via `std::str::from_utf8` and lower-cased for keyword matching.

**Case-sensitive matches** are checked first:
- `"N"`, `"Q"`, `"Z"` for set types
- `"UNION"`, `"HAS"`, `"AND"` for uppercase keyword variants

### Number Scanning
`number()` accumulates ASCII digit bytes. If a `.` followed by a digit is encountered, the token becomes a `FloatLiteral`.

### String Scanning
`string()` processes escape sequences (`\n`, `\t`, `\r`, `\"`, `\\`, octal `\NNN`, hex `\xHH`) byte-by-byte, building a `Vec<u8>` which is then converted via `String::from_utf8` and interned.

---

## String Interning

All identifiers and string literals are passed through `Interner::intern()`, which returns a `StringId (u32)`. The raw `String` is stored once in the interner's internal `Vec<String>`; the rest of the pipeline works with numerical IDs, eliminating heap comparisons during type checking and compilation.

```rust
Interner::intern("foo") → StringId(42)
Interner::lookup(StringId(42)) → "foo"
```

---

## Span

```rust
pub struct Span {
    pub line: usize,
    pub col:  usize,
    pub len:  usize,  // length in Unicode characters, not bytes
}
```

Each `Token` carries a `Span` used by the diagnostic system to display error highlights with character-level precision.
