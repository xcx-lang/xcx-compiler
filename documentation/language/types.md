//==language/types.md==\\
# XCX 3.0 Data Types

## Simple Types

| Symbol        | Type         | Default      | Example                    |
|---------------|-------------|--------------|----------------------------|
| `i` / `int`   | Integer     | `0`          | `42`, `-7`, `0`            |
| `f` / `float` | Float       | `0.0`        | `3.14`, `-0.5`, `2.0`      |
| `s` / `str`   | String      | `""`         | `"hello"`, `""`            |
| `b` / `bool`  | Boolean     | `false`      | `true`, `false`            |
| `date`        | Date        | `1970-01-01` | `date("2024-12-25")`       |
| `json`        | JSON Object | `null`       | `<<< {"key": "value"} >>>` |

## Complex Types

| Symbol       | Type                     | Declaration Example                          |
|--------------|-------------------------|----------------------------------------------|
| `array:i`    | Array of integers       | `array:i: nums {1, 2, 3}`                    |
| `array:f`    | Array of floats         | `array:f: vals {1.0, 2.5}`                   |
| `array:s`    | Array of strings        | `array:s: words {"a", "b"}`                  |
| `array:b`    | Array of booleans       | `array:b: flags {true, false}`               |
| `array:json` | Array of JSON objects   | `array:json: items`                          |
| `set:N`      | Set of Natural numbers  | `set:N: s {1,,10}`                           |
| `set:Z`      | Set of Integers         | `set:Z: s {-2, 0, 2}`                        |
| `set:Q`      | Set of Rational (float) | `set:Q: s {0.5, 1.0, 1.5}`                   |
| `set:S`      | Set of Strings          | `set:S: s {"a", "b"}`                        |
| `set:B`      | Set of Booleans         | `set:B: s {true, false}`                     |
| `set:C`      | Set of Characters       | `set:C: s {"A",,"Z"}`                        |
| `map`        | Key-Value Map           | `map: m { schema = [s <-> i] data = [...] }` |
| `table`      | Relational Table        | `table: t { columns = [...] rows = [...] }`  |
| `database:`  | Database Connection     | `database: db { engine = "sqlite", path = "app.db" }` |
| `fiber:T`    | Typed Fiber             | `fiber:b: f = my_fiber(arg)`                 |
| `fiber:`     | Void Fiber              | `fiber: f = my_void_fiber(arg)`              |

### array:json

`array:json` is an array storing elements of type `json`. Used primarily when working with HTTP responses that return arrays of JSON objects.

```xcx
--- Declare an empty JSON array
array:json: items;

--- Iterate over a JSON array (e.g. from a network response)
i: size = resp.body.size();
i: idx = 0;
while (idx < size) do;
    json: item = resp.body.get(idx);
    s: name; item.bind("name", name);
    >! name;
    idx = idx + 1;
end;

--- Add an element
json: obj <<< {"key": "value"} >>>;
items.push(obj);
```

Methods on `array:json` are identical to other array types (`.push()`, `.pop()`, `.get(i)`, `.size()`, etc.).

### database:

`database:` represents a connection to a relational database. Declared with a configuration block and used through its built-in methods. See [Database Documentation](database.md) for the full API.

```xcx
database: app {
    engine = "sqlite",
    path   = "app.db"
};

yield app.sync(users);
table: all = yield app.fetch(users);
```

## Default Values

```xcx
int:   def_int;     --- 0 (alias for i)
float: def_float;   --- 0.0 (alias for f)
str:   def_str;     --- "" (alias for s)
bool:  def_bool;    --- false (alias for b)
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
