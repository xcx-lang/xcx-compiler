//==language/database.md==\\
# XCX 3.0 Database

XCX 3.0 introduces native relational database support via the `database:` type and a set of built-in methods. Version 3.0 supports **SQLite** only.

---

## Table of Contents

1. [Connection Declaration (`database:`)](#1-connection-declaration-database)
2. [Table Column Attributes](#2-table-column-attributes)
3. [Named Arguments](#3-named-arguments)
4. [Result Capture (`as`)](#4-result-capture-as)
5. [Fiber Scoping Limitations (Windows)](#5-fiber-scoping-limitations-windows)
6. [API Reference](#6-api-reference)
   - 5.1 [DDL](#51-ddl)
   - 5.2 [Write Operations](#52-write-operations)
   - 5.3 [Read Operations](#53-read-operations)
   - 5.4 [Delete Operations](#54-delete-operations)
   - 5.5 [Transactions](#55-transactions)
   - 5.6 [Other](#56-other)
6. [Type Mapping (XCX ↔ SQL)](#6-type-mapping-xcx--sql)
7. [Security Constraints](#7-security-constraints)
8. [Full Example](#8-full-example)

---

## 1. Connection Declaration (`database:`)

```xcx
database: app {
    engine  = "sqlite",
    path    = "app.db"
};
```

### Fields

| Field      | Type | Required | Default | Description                                      |
|------------|------|----------|---------|--------------------------------------------------|
| `engine`   | `s`  | yes      | —       | Database engine. Currently only `"sqlite"`.      |
| `path`     | `s`  | yes      | —       | Path to the `.db` file (relative to project root).|
| `timeout`  | `i`  | no       | `5000`  | Operation timeout in ms.                         |
| `readonly` | `b`  | no       | `false` | Read-only mode.                                  |

### Behavior

- Connections are **lazy** — opened on first use.
- On connection failure (e.g. missing permissions) `halt.error` is triggered automatically.
- SQLite creates the database file if it does not exist (unless `readonly = true`).
- Multiple simultaneous connections are allowed — each has its own name.

```xcx
database: main { engine = "sqlite", path = "main.db" };
database: logs { engine = "sqlite", path = "logs.db" };

yield main.sync(users);
yield logs.sync(events);
```

### I/O Model

All DB operations that touch disk or the driver are I/O operations:

| Context        | Behavior              |
|----------------|-----------------------|
| Inside a fiber | require `yield`       |
| Outside a fiber| block synchronously   |

This applies to **read** (`fetch`, `query`, `queryRaw`), **write** (`push`, `save`, `insert`, `exec`), **delete** (`remove`, `exec`), and **DDL** (`sync`, `drop`) operations.

**Exception — methods without `yield`.** Methods that only operate on transaction state or metadata do not require `yield` in any context:

- `db.has()`, `db.close()`, `db.isOpen()`
- `db.begin()`, `db.commit()`, `db.rollback()`

> `begin()`, `commit()` and `rollback()` do not touch data — they only manage transaction state in the driver.

---

## 2. Table Column Attributes

These attributes extend the `table:` block declaration and are used by the database module.

### Attributes

| Attribute       | Description                                                        | SQL Equivalent      |
|-----------------|--------------------------------------------------------------------|---------------------|
| `@pk`           | Primary key. Can be combined with `@auto`.                        | `PRIMARY KEY`       |
| `@unique`       | Value must be unique within the column.                            | `UNIQUE`            |
| `@optional`     | Column may be `NULL` in SQL. XCX returns the type's default value on read. | `NULL`        |
| `@default(v)`   | Default column value in SQL.                                       | `DEFAULT v`         |
| `@fk(t.col)`    | Foreign key pointing to column `col` of table `t`.                | `REFERENCES t(col)` |

### Example

```xcx
table: users {
    columns = [
        id      :: i @auto @pk,
        name    :: s @unique,
        age     :: i,
        phone   :: s @optional,
        role    :: s @default("user")
    ]
    rows = [EMPTY]
};

table: posts {
    columns = [
        id      :: i @auto @pk,
        user_id :: i @fk(users.id),
        title   :: s,
        body    :: s @optional
    ]
    rows = [EMPTY]
};
```

### `@optional` Behavior

XCX has no `null` as a standalone type. `@optional` columns in SQL may store `NULL`, but when read into XCX the value is automatically replaced with the type's default:

| XCX Type | Value when SQL NULL |
|----------|---------------------|
| `i`      | `0`                 |
| `f`      | `0.0`               |
| `s`      | `""`                |
| `b`      | `false`             |

> **Note:** `@optional` only signals `NULL` acceptability on the SQL side. XCX does not preserve the distinction between `NULL` and the default value — that information is lost on read. If your application logic requires distinguishing these cases, store a flag in a separate column.

### Column Omission Rules for `add()` and `insert()`

| Column Attribute        | Can be omitted? | What goes into SQL             |
|-------------------------|-----------------|--------------------------------|
| `@auto`                 | yes (always)    | auto-generated value           |
| `@default(v)`           | yes             | value `v` from `DEFAULT`       |
| `@optional`             | yes             | `NULL`                         |
| `@optional @default(v)` | yes             | value `v` from `DEFAULT`       |
| no attributes           | **no**          | compile error                  |

> `@auto` columns can never be provided explicitly — neither positionally nor via named argument. Attempting to do so is a **compile error**.

---

## 3. Named Arguments

Named arguments allow passing values to `table.add()`, `table.insert()`, and `db.insert()` **by column name** rather than by position. They are an optional, additive feature — all existing positional calls remain fully valid.

Named arguments work **only** for table insert operations. They do not apply to user-defined functions, fibers, or other built-in methods. Column names are statically known to the compiler from the `table:` schema.

### Syntax

```xcx
--- Positional (backward compatible)
users.add("Alice", 25, "", "user");

--- Named
users.add(name = "Alice", age = 25, phone = "", role = "user");

--- With db.insert()
yield app.insert(users, name = "Alice", age = 25) as saved;
```

**Namespace separation.** The left side of `=` is always the column name. The right side is an expression from the local scope. These are two independent namespaces — no conflict:

```xcx
s: name = "Alice";
users.add(name = name, age = 25);
--- left "name"  = column users.name
--- right "name" = local variable
```

**Evaluation order.** Right-hand expressions are evaluated left to right in the order written:

```xcx
users.add(name = get_name(), age = get_age());
--- get_name() is called before get_age()
```

### Mixing Positional and Named

Positional and named arguments can be mixed. **Positional arguments must precede named arguments.**

```xcx
--- OK — positional before named
users.add("Alice", age = 25, role = "admin");

--- COMPILE ERROR — named before positional
users.add(name = "Alice", 25, "admin");
```

**Assignment rule.** Positional arguments are assigned to columns left-to-right in declaration order, **skipping `@auto`**, until they run out. Named arguments fill the remaining columns by name.

```xcx
--- "Alice" → name, rest named
users.add("Alice", age = 25, role = "admin");
--- phone omitted → NULL (@optional)

--- "Alice", 25 → name, age; role named, phone omitted
users.add("Alice", 25, role = "mod");
```

Positional arguments assign sequentially **without skipping**. Skipping a middle column is only possible via a named argument or by omitting it entirely (if optional):

```xcx
--- COMPILE ERROR — compiler assigns: name="Alice", age="" — type mismatch
--- there is no way to "skip" age positionally
users.add("Alice", "", "user");
```

Providing the same column both positionally and by name is a **compile error**:

```xcx
--- COMPILE ERROR — name provided twice
users.add("Alice", name = "Bob", age = 25);
```

### Compile-Time Rules

| Rule | Behavior |
|------|----------|
| `@auto` column provided | compile error |
| Unknown column name | compile error |
| Duplicate column name | compile error |
| Required column omitted | compile error |
| Named arg before positional arg | compile error |

**Completeness table:**

| Column Attribute        | Can be omitted? | Value when omitted                   |
|-------------------------|-----------------|--------------------------------------|
| `@auto`                 | yes (always)    | auto-generated value                 |
| `@default(v)`           | yes             | value `v` from `DEFAULT`             |
| `@optional`             | yes             | `NULL` (→ XCX default on read)       |
| `@optional @default(v)` | yes             | value `v` from `DEFAULT`             |
| no attributes           | **no**          | compile error                        |

```xcx
--- OK — role has @default("user"), phone is @optional
users.add(name = "Alice", age = 25);

--- COMPILE ERROR — age is required
users.add(name = "Alice");
```

---

## 4. Result Capture (`as`)

`as` is a general XCX mechanism for capturing the result of a block operation:

```xcx
--- HTTP request
net.request { method = "GET", url = "..." } as resp;

--- DB operations
yield app.insert(users, name = "Alice", age = 25) as saved;
yield app.exec("DELETE FROM users WHERE id = ?", [5]) as res;
```

### Write/Delete Result Object

Operations `db.insert()`, `db.save()`, `db.push()`, and `db.exec()` return an object with two fields:

| Field      | Type | Description                                                         |
|------------|------|---------------------------------------------------------------------|
| `affected` | `i`  | Number of rows changed / inserted by the operation                  |
| `insertId` | `i`  | ID of the last inserted record (`@auto @pk`); `0` if no insert      |

```xcx
yield app.insert(users, name = "Alice", age = 25) as saved;
>! "New user ID: " + s(saved.insertId);

yield app.exec("DELETE FROM users WHERE id = ?", [5]) as res;
>! "Deleted: " + s(res.affected);
```

Using `as` is optional — if the result is not needed it can be omitted:

```xcx
yield app.save(users);
```

### `affected` and `insertId` Semantics

| Operation                  | `affected`                      | `insertId`                        |
|----------------------------|---------------------------------|-----------------------------------|
| `insert()` — single row    | `1`                             | ID of inserted row                |
| `push()` — multiple rows   | number of inserted rows         | ID of last inserted row           |
| `save()` — upsert (insert) | `1`                             | ID of inserted row                |
| `save()` — upsert (update) | `1`                             | `0`                               |
| `exec(DELETE ...)`         | number of deleted rows          | `0`                               |
| `exec(UPDATE ...)`         | number of updated rows          | `0`                               |

> On operation failure (constraint violation, no connection, `readonly`) `halt.error` is triggered and execution does not reach the code using the `as` result. The result object is always valid if the code can see it.

---

## 5. Fiber Scoping Limitations (Windows)

In XCX 3.0, specifically on Windows environments, initializing a fiber-local variable directly with a database result can occasionally lead to an `Undefined variable` [S101] error in subsequent lines.

### Recommended Workaround

To ensure absolute visibility across conditional blocks within a fiber, it is recommended to **declare the variable type first** and then assign the database result in a separate statement.

```xcx
fiber run(-> b) {
    --- Recommended pattern
    b: exists_before;
    exists_before = db.has(logs);
    
    if (exists_before) then;
        >! "Table exists";
    end;
}
```

This pattern ensures that the variable is correctly registered in the symbol table before it is accessed by the analyzer. This limitation is planned for a native architectural resolution in XCX 4.0.

---

## 6. API Reference

### 5.1 DDL

```xcx
--- Creates the SQL table if it does not exist (based on XCX table schema)
yield app.sync(users);

--- Drops the SQL table
yield app.drop(users);

--- Checks if table exists in the database (no data touched — no yield)
b: exists = app.has(users);
```

### 5.2 Write Operations

```xcx
--- INSERT all rows from a local XCX table into SQL
yield app.push(users);

--- INSERT OR UPDATE (upsert) — requires @pk; no @pk = compile error
--- Implementation: INSERT INTO ... ON CONFLICT(pk) DO UPDATE SET ...
yield app.save(users);

--- INSERT a single row — positional or named
yield app.insert(users, "Alice", 25) as saved;
yield app.insert(users, name = "Alice", age = 25) as saved;
```

On error (violation of `@unique`, `@fk`, no connection) the operation triggers `halt.error`.

### 5.3 Read Operations

```xcx
--- SELECT * — returns a native XCX table
table: all_users = yield app.fetch(users);

--- SELECT with XCX-style filter (compiles to SQL WHERE)
table: adults = yield app.fetch(users).where(age > 18);

--- SELECT with raw SQL — requires schema hint as first argument
table: result = yield app.query(users, "SELECT * FROM users WHERE age > ?", [18]);

--- Raw SQL without schema — always returns a JSON array
json: raw = yield app.queryRaw("SELECT COUNT(*) as n FROM users");
--- raw = [{"n": 42}]

--- Get the first row
json: row = yield app.queryRaw("SELECT COUNT(*) as n FROM users").first();
--- row = {"n": 42}
--- halt.error if result is empty
```

#### `.first()` on `queryRaw` Result

`.first()` returns the first element of the JSON array as a single `json` object. It triggers `halt.error` if the query returned no rows. Typical use is for aggregate queries (`COUNT`, `SUM`, `MAX`) and lookups that return at most one row.

```xcx
json: row = yield app.queryRaw("SELECT COUNT(*) as n FROM users").first();
i: total; row.bind("n", total);
>! "Users: " + s(total);
```

If you are not certain the result will be non-empty, check the size before using `.first()`:

```xcx
json: results = yield app.queryRaw("SELECT * FROM users WHERE age > ?", [100]);
if (results.size() > 0) then;
    json: row = results.get(0);
end;
```

#### Supported Operators in `.where()`

The `.where()` filter compiles to a SQL `WHERE` clause. A limited subset of XCX expressions is supported:

| Category      | Supported                        |
|---------------|----------------------------------|
| Comparisons   | `==`, `!=`, `<`, `<=`, `>`, `>=` |
| Logical       | `AND`, `OR`, `NOT`               |
| Values        | literals, local variables        |
| Left operands | column names only                |

Function calls, string methods, and nested expressions are not supported in `.where()` with `db.fetch()`. Using an unsupported expression is a **compile error**.

```xcx
--- OK
table: r = yield app.fetch(users).where(age > 18 AND role == "admin");

--- COMPILE ERROR — string method not allowed in where() with fetch
table: r = yield app.fetch(users).where(name.lower() == "alice");
```

Chaining `.where()` joins conditions with `AND`:

```xcx
table: r = yield app.fetch(users)
    .where(age > 18)
    .where(role == "admin");
--- SQL equivalent: WHERE age > 18 AND role = 'admin'
```

#### `db.fetch()` and Local `rows`

`db.fetch()` reads only data from the SQL database — local `rows` declared in the `table:` block are ignored. The local table definition serves only as a schema hint.

```xcx
table: users {
    columns = [ id :: i @auto @pk, name :: s ]
    rows = [ ("Alice") ]   --- these rows do NOT go into fetch
};

yield app.sync(users);

--- Returns only rows stored in app.db
table: all = yield app.fetch(users);
```

To insert local `rows` into the database, use `db.push(users)` before `db.fetch()`.

### 5.4 Delete Operations

```xcx
--- DELETE with filter — .where() is required (D401)
yield app.remove(users).where(age < 18);

--- Delete all rows
yield app.truncate(users);

--- Raw DELETE with parameters
yield app.exec("DELETE FROM users WHERE id = ?", [5]) as res;
```

> **`db.remove()` without `.where()` is a compile error (D401).** To delete all rows, use `db.truncate(users)` explicitly.

### 5.5 Transactions

```xcx
app.begin();
yield app.save(users);
yield app.save(posts);
app.commit();
```

> If any operation inside the transaction triggers `halt.error`, the transaction is **automatically rolled back** before the frame is aborted. The connection returns to the state before `begin()`.

`rollback()` is for explicitly cancelling a transaction in conditional logic before an error occurs:

```xcx
app.begin();
b: valid = validate_users(users);
if (NOT valid) then;
    app.rollback();
end;
yield app.save(users);
app.commit();
```

### 5.6 Other

```xcx
app.close();
b: alive = app.isOpen();
```

---

## 6. Type Mapping (XCX ↔ SQL)

| XCX Type | SQL Type (SQLite)              |
|----------|--------------------------------|
| `i`      | `INTEGER`                      |
| `f`      | `REAL`                         |
| `s`      | `TEXT`                         |
| `b`      | `INTEGER` (0 / 1)              |
| `date`   | `TEXT` (`YYYY-MM-DD HH:mm:ss`) |

---

## 7. Security Constraints

| Constraint                          | Behavior                                                   |
|-------------------------------------|------------------------------------------------------------|
| Path containing `..`                | `halt.fatal` — path traversal                              |
| Absolute path                       | `halt.fatal`                                               |
| Raw SQL with unvalidated input      | Developer is responsible for sanitization — use `?` params |
| `readonly = true` + DML operation   | `halt.error`                                               |
| `save()` on table without `@pk`     | compile error                                              |
| `remove()` without `.where()`       | compile error (D401)                                       |

Always use prepared statements with `?`:

```xcx
--- WRONG
app.exec("DELETE FROM users WHERE name = '" + name + "'");

--- CORRECT
app.exec("DELETE FROM users WHERE name = ?", [name]);
```

---

## 8. Full Example

```xcx
database: app {
    engine = "sqlite",
    path   = "data.db"
};

table: users {
    columns = [
        id    :: i @auto @pk,
        name  :: s @unique,
        email :: s @unique,
        age   :: i,
        phone :: s @optional
    ]
    rows = [EMPTY]
};

yield app.sync(users);

fiber handle_get_users(json: req -> json) {
    table: all = yield app.fetch(users);
    yield net.respond(200, all.toJson());
};

fiber handle_create_user(json: req -> json) {
    json: body;
    req.bind("body", body);

    s: name;  body.bind("name", name);
    s: email; body.bind("email", email);
    i: age;   body.bind("age", age);

    --- Named arguments — readable and order-independent
    yield app.insert(users, name = name, email = email, age = age) as saved;

    json: resp <<< {"ok": true, "id": 0} >>>;
    resp.set("id", saved.insertId);
    yield net.respond(201, resp);
};

serve: api {
    port   = 8080,
    routes = [
        "GET  /users" :: handle_get_users,
        "POST /users" :: handle_create_user
    ]
};
```
