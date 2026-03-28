# XCX 2.2 Data Types

## Simple Types

| Symbol | Type         | Default      | Example                    |
|--------|-------------|--------------|----------------------------|
| `i`    | Integer     | `0`          | `42`, `-7`, `0`            |
| `f`    | Float       | `0.0`        | `3.14`, `-0.5`, `2.0`      |
| `s`    | String      | `""`         | `"hello"`, `""`            |
| `b`    | Boolean     | `false`      | `true`, `false`            |
| `date` | Date        | `1970-01-01` | `date("2024-12-25")`       |
| `json` | JSON Object | `null`       | `<<< {"key": "value"} >>>` |

## Complex Types

| Symbol    | Type                     | Declaration Example                          |
|-----------|-------------------------|----------------------------------------------|
| `array:T` | Array of elements T     | `array:i: nums {1, 2, 3}`                    |
| `set:N`   | Set of Natural numbers  | `set:N: s {1,,10}`                           |
| `set:Z`   | Set of Integers         | `set:Z: s {-2, 0, 2}`                        |
| `set:Q`   | Set of Rational (float) | `set:Q: s {0.5, 1.0, 1.5}`                   |
| `set:S`   | Set of Strings          | `set:S: s {"a", "b"}`                        |
| `set:B`   | Set of Booleans         | `set:B: s {true, false}`                     |
| `set:C`   | Set of Characters       | `set:C: s {"A",,"Z"}`                        |
| `map`     | Key-Value Map           | `map: m { schema = [s <-> i] data = [...] }` |
| `table`   | Relational Table        | `table: t { columns = [...] rows = [...] }`  |
| `fiber:T` | Typed Fiber             | `fiber:b: f = my_fiber(arg)`                 |
| `fiber:`  | Void Fiber              | `fiber: f = my_void_fiber(arg)`              |

## Default Values

```xcx
i: def_int;     --- 0
f: def_float;   --- 0.0
s: def_str;     --- ""
b: def_bool;    --- false
```

## Type Casting

```xcx
f: x = 3.7;
i: n = i(x);       --- 3 (truncate toward zero)

i: m = 42;
f: y = f(m);       --- 42.0

i: num = 99;
s: str = s(num);   --- "99"
```

> [!NOTE]
> `b → i` conversion is **intentionally blocked**. `true + 1` is a type error to prevent logical bugs common in C-family languages.
