# XCX 2.2 String Methods

String objects in XCX are immutable. Methods return a **new string** and do not modify the original.

## Properties

- `.length`: Returns the number of Unicode code points in the string (e.g., `"zażółć".length` is 6). **Used without parentheses.**

## Methods

| Method               | Signature    | Description                                      |
|----------------------|--------------|--------------------------------------------------|
| `.upper()`           | `() → s`     | Converts all characters to uppercase.            |
| `.lower()`           | `() → s`     | Converts all characters to lowercase.            |
| `.trim()`            | `() → s`     | Removes leading/trailing whitespace.             |
| `.replace(f, t)`     | `(s, s) → s` | Replaces all occurrences of `f` with `t`.        |
| `.slice(s, e)`       | `(i, i) → s` | Returns substring from index `s` up to `e`.      |
| `.indexOf(s)`        | `(s) → i`    | Returns index of first occurrence, or -1.        |
| `.lastIndexOf(s)`    | `(s) → i`    | Returns index of last occurrence, or -1.         |
| `.startsWith(s)`     | `(s) → b`    | Returns true if string starts with `s`.          |
| `.endsWith(s)`       | `(s) → b`    | Returns true if string ends with `s`.            |
| `.toInt()`           | `() → i`     | Parses string to Integer; `halt.error` if fail.  |
| `.toFloat()`         | `() → f`     | Parses string to Float; `halt.error` if fail.    |

## Examples

```xcx
s: raw = "  XCX-Language  ";
s: clean = raw.trim().lower().replace("-", "_"); --- "xcx_language"

i: start = "Programming".indexOf("gram");       --- 3
s: part = "Programming".slice(0, 4);            --- "Prog"

i: age = "25".toInt();
```
