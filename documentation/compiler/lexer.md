//==compiler/lexer.md==\\
# XCX Lexer (Scanner) — v2.2

The Lexer is responsible for converting the raw source byte stream into a stream of discrete tokens.

## Implementation Details

- **File**: `src/lexer/scanner.rs`
- **Technique**: Manual, eager, byte-by-byte scanning on `&[u8]`.
- **API**: Single method `next_token(&mut self, interner: &mut Interner) -> Token`, called on demand by the parser (not an iterator).
- **Lookahead**: Single-byte lookahead via `peek()`, two-byte via `peek_next()` / `peek_at(offset)`.

## Internal Structure

```rust
pub struct Scanner<'a> {
    source:   &'a [u8],   // borrowed reference to original source — no allocation
    pos:      usize,      // byte position
    char_pos: usize,      // Unicode character position (for Span.col)
    line:     usize,
    col:      usize,
}
```

`Scanner` borrows the source string as a byte slice. There is no upfront conversion to `Vec<char>` — all scanning is byte-level. `char_pos` is incremented only for bytes that are not UTF-8 continuation bytes (`10xxxxxx`), so Unicode character counts remain correct for Span reporting without decoding every character.

## Token Types

Tokens are defined in `src/lexer/token.rs` as the `TokenKind` enum. Each `Token` carries a `Span { line, col, len }` for error reporting. `len` is measured in Unicode characters (via `char_pos` deltas), not bytes.

Key categories:

| Category | Examples |
|---|---|
| Literals | `IntLiteral(i64)`, `FloatLiteral(f64)`, `StringLiteral(StringId)`, `True`, `False` |
| Type keywords | `TypeI`, `TypeF`, `TypeS`, `TypeB`, `Array`, `Set`, `Map`, `Table`, `Json`, `Date`, `Fiber` |
| Set type keywords | `TypeSetN`, `TypeSetQ`, `TypeSetZ`, `TypeSetS`, `TypeSetB`, `TypeSetC` |
| Control flow | `If`, `Then`, `ElseIf`, `Else`, `End`, `While`, `Do`, `For`, `In`, `To`, `Break`, `Continue` |
| Functions/Fibers | `Func`, `Return`, `Fiber`, `Yield` |
| Operators | `Plus`, `Minus`, `Star`, `Slash`, `Caret`, `PlusPlus`, `Has`, `And`, `Or`, `Not` |
| Set operators | `Union`, `Intersection`, `Difference`, `SymDifference` |
| Special punctuation | `GreaterBang` (`>!`), `GreaterQuestion` (`>?`), `DoubleColon` (`::`), `DoubleComma` (`,,`), `Bridge` (`<->`) |
| Builtins | `Net`, `Serve`, `Store`, `Halt`, `Terminal`, `Json`, `Date` |
| Special | `RawBlock(StringId)`, `AtStep`, `AtAuto`, `AtWait` |

## Special Scanning Features

### Raw Blocks
Delimited by `<<<` and `>>>`. Everything between is captured as a single `RawBlock(StringId)` token, used for inline JSON or multi-line string data.

```
<<<
  { "key": "value" }
>>>
```

Detection uses `self.source[self.pos..].starts_with(b"<<<")` — opened as part of `<` disambiguation in the main match arm. The raw content is captured byte-by-byte until `>>>` is found.

### Comments
XCX uses `---` as comment delimiter. Detection uses `self.source[self.pos..].starts_with(b"---")`:
- **Single-line**: `--- this is a comment` (non-whitespace content on same line after `---`)
- **Multi-line**: `---` followed by only whitespace until end of line opens a block, closed by `*---`

Multi-line close is detected by `self.source[self.pos..].starts_with(b"*---")`. The scanner peeks ahead after consuming `---` to decide which mode applies.

### Unicode Set Operators
The scanner recognises Unicode symbols via `starts_with` on their UTF-8 byte sequences:
- `∪` → `TokenKind::Union`
- `∩` → `TokenKind::Intersection`
- `\` (ASCII backslash) → `TokenKind::Difference`
- `⊕` → `TokenKind::SymDifference`

For multi-byte Unicode operators, `advance()` is called the appropriate number of extra times to consume the remaining continuation bytes after the leading byte (`c >= 128`).

### Else/ElseIf Disambiguation
The scanner peeks ahead after recognising `else` / `els` to check if the next word is `if` — if so, it collapses the two words into a single `ElseIf` token. The saved position (`after_ws_pos`, `after_ws_char_pos`, `after_ws_line`, `after_ws_col`) allows the scanner to backtrack if the next word is not `if`.

### `@` Directives
Tokens starting with `@` are scanned by consuming ASCII alphabetic bytes and matching the result:
- `@step` → `AtStep`
- `@auto` → `AtAuto`
- `@wait` → `AtWait`

Unknown `@` sequences produce `TokenKind::Unknown('@')`.

### Dot-Dot (`..`) → `To`
Two consecutive dots `..` are scanned as the `To` token (used in range expressions), distinct from a single `.` (`Dot`).

### Double-Comma (`,,`) → `DoubleComma`
Two consecutive commas `,,` are scanned as `DoubleComma` (used in set range literals: `set:N { 1,,10 }`).

### Identifier Scanning
`identifier()` captures a contiguous run of ASCII alphanumeric bytes, underscores, and bytes `>= 128` (to include UTF-8 identifiers). The captured byte range is converted via `std::str::from_utf8` and lowercased for keyword matching. Case-sensitive matches are checked first (e.g. `"N"`, `"Q"`, `"Z"` for set types, `"UNION"`, `"HAS"`, `"AND"` for uppercase keyword variants). Non-keyword identifiers are passed to `Interner::intern()`.

### Number Scanning
`number()` accumulates ASCII digit bytes. If a `.` followed by a digit is encountered, the token becomes a `FloatLiteral`. The byte range is converted via `str::from_utf8` and parsed with `.parse()`.

### String Scanning
`string()` processes escape sequences (`\n`, `\t`, `\r`, `\"`, `\\`, octal `\NNN`, hex `\xHH`) byte-by-byte, building a `Vec<u8>` which is then converted via `String::from_utf8`. The result is interned.

## String Allocation

All identifiers and string literals are passed through `Interner::intern()`, returning a `StringId (u32)`. The raw `String` is stored once in the interner's internal `Vec<String>`; the rest of the pipeline works with numeric IDs, eliminating heap comparisons during type checking and compilation.