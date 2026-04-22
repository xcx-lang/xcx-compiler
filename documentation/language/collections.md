//==language/collections.md==\\
# XCX 3.0 Collections

## Arrays

```xcx
array:i: nums {10, 20, 30};
nums.size();           --- 3
nums.get(0);           --- 10
nums.push(40);         --- adds 40 to the end
i: last = nums.pop();  --- removes and returns last element
nums.sort();           --- sorts in-place
nums.reverse();        --- reverses in-place
nums.show();           --- prints contents to terminal
```

### Array Methods

| Method            | Signature    | Returns | Description                                                         |
|-------------------|--------------|---------|---------------------------------------------------------------------|
| `.size()`         | `() → i`     | `i`     | Number of elements                                                  |
| `.get(i)`         | `(i) → T`    | `T`     | Element at position `i` (0-indexed); `halt.error` if out of bounds  |
| `.push(val)`      | `(T) → b`    | `b`     | Appends element to the end                                          |
| `.pop()`          | `() → T`     | `T`     | Removes and returns the last element                                |
| `.insert(i, val)` | `(i, T) → b` | `b`     | Inserts at position `i`, shifts rest; `halt.error` if out of bounds |
| `.update(i, val)` | `(i, T) → b` | `b`     | Overwrites element at position `i`; `halt.error` if out of bounds   |
| `.delete(i)`      | `(i) → b`    | `b`     | Removes element at position `i`; `halt.error` if out of bounds      |
| `.find(val)`      | `(T) → i`    | `i`     | Index of first occurrence, or `-1`                                  |
| `.contains(val)`  | `(T) → b`    | `b`     | Checks if value exists                                              |
| `.isEmpty()`      | `() → b`     | `b`     | `true` if empty                                                     |
| `.clear()`        | `() → b`     | `b`     | Removes all elements                                                |
| `.sort()`         | `() → b`     | `b`     | Sorts ascending (in-place)                                          |
| `.reverse()`      | `() → b`     | `b`     | Reverses order (in-place)                                           |
| `.toStr()`        | `() → s`     | `s`     | Serializes array to a JSON-formatted string                         |
| `.toJson()`       | `() → json`  | `json`  | Converts array to a native JSON structure                           |
| `.show()`         | `() → b`     | `b`     | Prints contents to terminal                                         |

```xcx
array:i: nums {5, 2, 8, 1};
nums.sort();            --- {1, 2, 5, 8}
nums.reverse();         --- {8, 5, 2, 1}
nums.push(99);          --- {8, 5, 2, 1, 99}
i: last = nums.pop();   --- last = 99, nums = {8, 5, 2, 1}
nums.insert(1, 15);     --- inserts 15 at position 1
nums.update(0, 5);      --- sets element 0 to 5
nums.delete(3);         --- removes element at position 3
b: found = nums.contains(5);
i: idx   = nums.find(5);
b: empty = nums.isEmpty();
```

---

## Sets

### Domains

| Symbol | Type              | Example                       |
|--------|-------------------|-------------------------------|
| `N`    | Natural (≥ 0)     | `set:N: s {0, 1, 2}`          |
| `Z`    | Integer           | `set:Z: s {-3, 0, 3}`         |
| `Q`    | Rational (Float)  | `set:Q: s {0.5, 1.0}`         |
| `S`    | String            | `set:S: s {"a", "b"}`         |
| `B`    | Boolean           | `set:B: s {true, false}`      |
| `C`    | Character         | `set:C: s {"A",,"Z"}`         |

### Initialization

Sets can be initialized with explicit values or ranges. Ranges are **inclusive** on both sides.

```xcx
set:N: small  {1,,5};                  --- {1, 2, 3, 4, 5}
set:N: evens  {0,,100 @step 2};        --- {0, 2, 4, ...}
set:Q: thirds {0.0,,1.0 @step 0.33};
set:C: letters {"A",,"Z"};            --- all uppercase letters
```

Sets **automatically deduplicate** elements.

### Set Operations

```xcx
set:N: setA {1,,5};
set:N: setB {3,,7};

set:N: u  = setA UNION setB;
set:N: i  = setA INTERSECTION setB;
set:N: d  = setA DIFFERENCE setB;
set:N: sd = setA SYMMETRIC_DIFFERENCE setB;

--- Unicode symbols are equivalent
setA ∪ setB
setA ∩ setB
setA \ setB
setA ⊕ setB
```

### Set Methods

| Method          | Signature | Returns | Description                                |
|-----------------|-----------|---------|--------------------------------------------|
| `.size()`       | `() → i`  | `i`     | Number of elements                         |
| `.isEmpty()`    | `() → b`  | `b`     | `true` if empty                            |
| `.contains(v)`  | `(T) → b` | `b`     | Checks membership                          |
| `.add(v)`       | `(T) → b` | `b`     | Adds element (ignores duplicate)           |
| `.remove(v)`    | `(T) → b` | `b`     | Removes element (no-op if not present)     |
| `.clear()`      | `() → b`  | `b`     | Removes all elements                       |
| `.show()`       | `() → b`  | `b`     | Prints `{elem, elem, ...}` to terminal     |

### Random Selection and Iteration

```xcx
--- Random selection from a set:
i: picked_set = random.choice from small;

--- Random selection from an array:
array:i: nums {1, 2, 3, 4, 5};
i: picked_arr = random.choice from nums;
```

Picking from an empty set or array returns `false`.

### Iteration

```xcx
for p in small do;
    >! p;
end;
```

---

## Maps

```xcx
map: ages {
    schema = [s <-> i]
    data = [ "alice" :: 30, "bob" :: 25 ]
};

--- Empty Map
map: scores {
    schema = [s <-> i]   --- both separators are equivalent (<-> and <=>)
    data = [EMPTY]
};
```

### Map Methods

| Method           | Signature       | Returns   | Description                               |
|------------------|-----------------|-----------|-------------------------------------------|
| `.size()`        | `() → i`        | `i`       | Number of key-value pairs                 |
| `.get(key)`      | `(K) → V`       | `V`       | Returns value; `halt.error` if key missing|
| `.contains(key)` | `(K) → b`       | `b`       | Checks if key exists                      |
| `.insert(k, v)`  | `(K, V) → b`    | `b`       | Inserts or overwrites                     |
| `.remove(key)`   | `(K) → b`       | `b`       | Removes pair; `false` if key missing      |
| `.keys()`        | `() → array:K`  | `array:K` | Returns array of keys                     |
| `.values()`      | `() → array:V`  | `array:V` | Returns array of values                   |
| `.clear()`       | `() → b`        | `b`       | Removes all pairs                         |
| `.toStr()`       | `() → s`        | `s`       | Serializes map to a JSON-formatted string |
| `.show()`        | `() → b`        | `b`       | Prints map contents to terminal           |
| `.toJson()`      | `() → json`     | `json`    | Serializes map to a JSON object           |

Map keys are converted to strings in the resulting JSON object.

### Map Serialization (toJson)

#### Signature
`.toJson() → json`

#### Description
Serializes the map to a JSON object. All keys are converted to strings using their `.toString()` representation to satisfy JSON object key requirements.

```xcx
map: scores {
    schema = [s <-> i]
    data = [ "alice" :: 100, "bob" :: 85 ]
};
json: j = scores.toJson();
```

**JSON Output:**
```json
{
    "alice": 100,
    "bob": 85
}
```

#### Behavior for Special Cases
- **Empty Map**: Returns an empty object `{}`.
- **Nested Structures**: If a map contains arrays, other maps, or tables, they are recursively serialized to their JSON equivalents.
- **Key Conversion**: All map keys are converted to strings in the resulting JSON object using their standard `.toString()` representation.

#### Type Mapping
| XCX Type | JSON Type |
|----------|-----------|
| `i`      | `number`  |
| `f`      | `number`  |
| `s`      | `string`  |
| `b`      | `boolean` |
| `date`   | `string` (format `"YYYY-MM-DD HH:mm:ss"`) |
| `json`   | (unchanged) |

Always use `.contains()` before `.get()`:

```xcx
if (ages.contains("alice")) then;
    >! ages.get("alice");
end;
```

---

## Tables

Relational data structures with optional auto-increment columns.

```xcx
table: products {
    columns = [ id :: i @auto, name :: s, price :: f ]
    rows = [ ("Laptop", 2999.99), ("Phone", 1499.50) ]
};

--- Empty Table
table: logs {
    columns = [ id :: i @auto, msg :: s ]
    rows = [EMPTY]
};
```

The `@auto` modifier on an `i` column creates an auto-incremented ID — it is skipped in `.insert()` and `.add()`.

> [!NOTE]
> Additional column attributes (`@pk`, `@unique`, `@optional`, `@default(v)`, `@fk(t.col)`) are used when connecting a table to a database. See [Database Documentation](database.md) for details.

### Row Access

```xcx
products[0].name    --- "Laptop" (sugar for .get(0))
products[1].price   --- 1499.50
```

### Table Methods

| Method               | Signature               | Returns | Description                                      |
|----------------------|-------------------------|---------|--------------------------------------------------|
| `.count()`           | `() → i`                | `i`     | Number of rows                                   |
| `.get(i)`            | `(i) → row`             | `row`   | Row at index `i`                                 |
| `.insert(vals...)`   | `(T...) → b`            | `b`     | Adds row (skips `@auto` columns)                 |
| `.add(vals...)`      | `(T...) → b`            | `b`     | Alias for `.insert()` — identical behavior       |
| `.update(i, vals)`   | `(i, [T...]) → b`       | `b`     | Replaces row values; `@auto` columns preserved   |
| `.delete(i)`         | `(i) → b`               | `b`     | Removes row at index `i`                         |
| `.where(pred)`       | `(expr) → table`        | `table` | Filters — returns a new table                    |
| `.join(t, pred)`     | `(table, pred) → table` | `table` | Inner join with another table                    |
| `.toJson()`          | `() → json`             | `json`  | Serializes all rows to a JSON array of objects   |
| `.show()`            | `() → b`                | `b`     | Prints table in ASCII format                     |

### Named Arguments for `.add()` and `.insert()`

When a table has database column attributes, values can be passed by column name instead of position. Named arguments are **optional** — positional calls remain fully valid.

```xcx
table: users {
    columns = [
        id    :: i @auto @pk,
        name  :: s @unique,
        age   :: i,
        phone :: s @optional,
        role  :: s @default("user")
    ]
    rows = [EMPTY]
};

--- Positional (backward compatible)
users.add("Alice", 25, "", "user");

--- Named
users.add(name = "Alice", age = 25, phone = "", role = "user");

--- Mixed — positional args must come first
users.add("Alice", age = 25, role = "admin");
```

**Namespace separation.** The left side of `=` is always the column name. The right side is an expression from the local scope. These are two independent namespaces — no conflict:

```xcx
s: name = "Alice";
users.add(name = name, age = 25);
--- left "name"  = column users.name
--- right "name" = local variable
```

Rules: positional args must precede named; `@auto` columns can never be passed; omitting a required (non-`@optional`, non-`@default`) column is a compile error; duplicate column names in the same call are a compile error. See [Database Documentation](database.md) for the full specification.

### Filtering (where)

```xcx
--- Shorthand syntax (column names usable directly)
table: expensive = products.where(price > 1000.0);
table: named     = products.where(name HAS "Pro");

--- Lambda
table: r = products.where(row -> row.price > 1000.0);

--- Chaining
table: result = products
    .where(price > 1000.0)
    .where(name HAS "Pro");
```

> [!IMPORTANT]
> **Name Conflicts in `.where()` (S301)**: Column names take precedence over local variables inside predicates. If a local variable has the same name as a column, rename the variable to avoid a compile error.
>
> ```xcx
> --- Wrong (conflict: 'token' exists both as column and parameter)
> fiber verify(s: token) {
>     table: sess = db.sessions.where(token == token);
> };
>
> --- Correct
> fiber verify(s: t) {
>     table: sess = db.sessions.where(token == t);
> };
> ```

### Joins

```xcx
--- Key-based join
table: report = users.join(orders, "id", "user_id");

--- Lambda join
table: custom = tableA.join(tableB, (a, b) -> a.id == b.ref_id);
```

When joined tables share a column name (other than the join key), the resulting column is prefixed with `{table_name}_`.

### Serialization (toJson)

#### Signature
`.toJson() → json`

#### Description
Serializes all rows of a table to a JSON array, where each row becomes an object with keys corresponding to column names. `@auto` columns are included in the result.

#### Format
Always returns a JSON array (`[...]`). An empty table returns `[]`.

```xcx
table: products {
    columns = [ id :: i @auto, name :: s, price :: f ]
    rows = [ ("Laptop", 2999.99), ("Phone", 1499.50) ]
};

json: result = products.toJson();
```

**JSON Output:**
```json
[
    {"id": 1, "name": "Laptop", "price": 2999.99},
    {"id": 2, "name": "Phone",  "price": 1499.50}
]
```

#### Type Mapping
| XCX Type        | JSON Type                                 |
|-----------------|-------------------------------------------|
| `i` / `int`     | `number`                                  |
| `f` / `float`   | `number`                                  |
| `s` / `str`     | `string`                                  |
| `b` / `bool`    | `boolean`                                 |
| `date`          | `string` (format `"YYYY-MM-DD HH:mm:ss"`) |

#### Behavior for Special Cases
- **Empty Table**: Returns an empty array `[]`.
- **Filtered Table**: Only rows currently in the table (after `.where()`) are serialized.
- **Joined Table**: All columns from the joined result are included.
