# XCX Language Reference — v3.0

> Complete reference of XCX language syntax and semantics.

---

## Table of Contents

1. [Data Types](#data-types)
2. [Variables and Constants](#variables-and-constants)
3. [Operators](#operators)
4. [Control Flow](#control-flow)
5. [Functions](#functions)
6. [Fibers (Coroutines)](#fibers-coroutines)
7. [Collections](#collections)
8. [JSON](#json)
9. [Tables](#tables)
10. [Database (SQLite)](#database-sqlite)
11. [Networking (HTTP)](#networking-http)
12. [Storage (Files)](#storage-files)
13. [Terminal](#terminal)
14. [Date and Time](#date-and-time)
15. [Randomness](#randomness)
16. [Cryptography](#cryptography)
17. [Diagnostics and Halt](#diagnostics-and-halt)
18. [Include](#include)

---

## Data Types

| XCX Type | Description | Example |
|---|---|---|
| `i` | 48-bit Integer | `i: age = 25;` |
| `f` | 64-bit Float | `f: pi = 3.14;` |
| `s` | UTF-8 String | `s: name = "Alice";` |
| `b` | Boolean | `b: flag = true;` |
| `date` | Date/Time (ms) | `date: d = date("2024-01-01");` |
| `json` | Any JSON | `json: data = <<<{}>>>;` |
| `array:T` | Typed Array | `array:i nums = [1, 2, 3];` |
| `set:N/Q/Z/S/C/B` | Set | `set:N mySet = set:N { 1,,100 };` |
| `map:K<->V` | Map | `map:s<->i scores = ["Alice" :: 100];` |
| `table` | Relational Table | `table: ...` |
| `fiber:T` | Coroutine | `fiber:i f = myFiber(10);` |
| `database` | SQLite Connection | `database: db = database { ... };` |

### Type Casting

```xcx
i(x)   --- to integer
f(x)   --- to float
s(x)   --- to string
b(x)   --- to boolean
```

---

## Variables and Constants

```xcx
--- Declaration with explicit type
i: count = 0;
s: greeting = "Hello";

--- Constant (cannot be reassigned)
const f: PI = 3.14159;

--- Type Inference
var result = 42;        --- type inferred as int
var name = "Bob";       --- type inferred as string
```

### Assignment

```xcx
count = count + 1;
greeting = "World";
```

---

## Operators

### Arithmetic

| Operator | Description | Example |
|---|---|---|
| `+` | Addition / String concatenation | `a + b` |
| `-` | Subtraction | `a - b` |
| `*` | Multiplication | `a * b` |
| `/` | Division | `a / b` |
| `%` | Modulo | `a % b` |
| `^` | Exponentiation | `a ^ b` |
| `++` | Digit concatenation | `12 ++ 34` = `1234` |

### Comparisons

`==`, `!=`, `>`, `<`, `>=`, `<=`

### Logical

| Operator | Alternative | Description |
|---|---|---|
| `AND` | `&&` | Logical and |
| `OR` | `||` | Logical or |
| `NOT` | `!!` | Logical not |
| `HAS` | — | Containment (`"abc" HAS "b"`) |

### Set Operators

| Operator | Symbol | Unicode |
|---|---|---|
| `UNION` | | `∪` |
| `INTERSECTION` | | `∩` |
| `DIFFERENCE` | `\` | |
| `SYMMETRIC_DIFFERENCE` | | `⊕` |

### Date Operators

```xcx
date("2024-01-15") + 7    --- date + days
date("2024-01-15") - 7    --- date - days
date("2024-01-15") - date("2024-01-01")  --- difference in ms
```

---

## Control Flow

### If / ElseIf / Else

```xcx
if (condition) then;
    ...
elseif (other_condition) then;
    ...
else;
    ...
end;
```

### While

```xcx
while (condition) do;
    ...
end;
```

### For (Numeric Range)

```xcx
for i in 1 to 100 do;
    >! i;
end;

--- With step
for i in 0 to 100 @step 5 do;
    >! i;
end;
```

### For (Array / Set / Fiber)

```xcx
for item in myArray do;
    >! item;
end;

for elem in mySet do;
    >! elem;
end;

for val in myFiber do;
    >! val;
end;
```

### Break and Continue

```xcx
while (true) do;
    if (condition) then;
        break;
    end;
    if (skip) then;
        continue;
    end;
end;
```

---

## Functions

### Curly Brace Style (C-like)

```xcx
func add(i: a, i: b -> i) {
    return a + b;
}
```

### XCX Style (Keyword block)

```xcx
func:i: add(i: a, i: b) do;
    return a + b;
end;
```

### Lambdas

```xcx
var double = x -> x * 2;
var sum = (x, y) -> x + y;
```

### Calling

```xcx
i: result = add(3, 4);
```

---

## Fibers (Coroutines)

Fibers are cooperative coroutines — not OS threads.

### Fiber Definition

```xcx
fiber counter(i: start) {
    i: n = start;
    while (true) do;
        yield n;
        n = n + 1;
    end;
}
```

### Fiber Instantiation

```xcx
fiber:i: c = counter(1);
```

### Iterating over a Fiber

```xcx
for val in c do;
    >! val;
    if (val >= 10) then;
        break;
    end;
end;
```

### Manual Resuming

```xcx
i: val = c.next();
b: done = c.isDone();
c.close();
```

### Yield Delegation

```xcx
fiber pipeline(array:i data) {
    yield from processData(data);
}
```

### Void Fiber (no value)

```xcx
fiber doWork() {
    --- do work
    yield;  --- suspension without value
}

fiber:void: w = doWork();
w.run();
```

---

## Collections

### Array

```xcx
array:i nums = [1, 2, 3, 4, 5];
nums.push(6);
i: first = nums.get(0);
nums.set(0, 99);
nums.delete(0);
i: size = nums.size();
b: has = nums.contains(3);
nums.sort();
nums.reverse();
s: joined = nums.join(", ");
```

### Set

```xcx
--- Natural set with range
set:N primes = set:N { 2, 3, 5, 7, 11 };

--- Set with range
set:Z mySet = set:Z { -10,,10 };

--- Set with step
set:N evens = set:N { 2,,100 @step 2 };

--- Operations
primes.add(13);
primes.remove(2);
b: hasFive = primes.contains(5);
i: count = primes.size();

--- Set operations
set:N union = a UNION b;
set:N inter = a INTERSECTION b;
set:N diff  = a DIFFERENCE b;
set:N sym   = a SYMMETRIC_DIFFERENCE b;
```

### Map

```xcx
map:s<->i scores = ["Alice" :: 100, "Bob" :: 85];
scores.set("Charlie", 90);
i: aliceScore = scores.get("Alice");
scores.remove("Bob");
array:s keys = scores.keys();
```

---

## JSON

```xcx
--- JSON Parsing
json: data = json.parse(jsonString);

--- Raw block (inline JSON)
json: config = <<<{ "host": "localhost", "port": 8080 }>>>;

--- Field Access
json: host = data.host;
json: nested = data.get("/server/host");

--- Modification
data.set("/port", 9090);
data.push("/items", "newItem");

--- Binding to variables
s: hostname;
data.bind("/host", hostname);

--- Injecting into a table
data.inject(mapping, myTable);

--- Checking
b: exists = data.exists("/optional");
i: count = data.size();
```

---

## Tables

```xcx
--- Table Definition
table: users = table {
    columns = [
        id   :: i @auto,
        name :: s,
        age  :: i,
        email :: s
    ]
    rows = EMPTY
};

--- Inserting
users.insert("Alice", 30, "alice@example.com");
users.insert(name = "Bob", age = 25, email = "bob@example.com");

--- Querying
table: adults = users.where(row -> row.age >= 18);
table: result = users.join(orders, "id", "userId");

--- Displaying
users.show();

--- Rows
i: count = users.count();
json: row = users.get(0);

--- Updating
users.update(0, ["Alice Updated", 31, "alice@new.com"]);

--- Deleting
users.delete(0);

--- Conversion to JSON
json: usersJson = users.toJson();

--- Saving (@pk required for save())
table: products = table {
    columns = [id :: i @pk, name :: s, price :: f]
    rows = EMPTY
};
products.save("Laptop", 999.99);  --- insert or update
```

---

## Database (SQLite)

```xcx
--- Connection
database: db = database {
    engine = "sqlite",
    path = "myapp.db",
    users = users,
    products = products
};

--- Schema is automatically synchronized

--- Inserting
db.insert(users, "Alice", 30, "alice@example.com");

--- Querying
table: result = db.fetch(users);

--- Filtered Query
table: adults = db.fetch(users.where(row -> row.age >= 18));

--- Raw SQL
json: rows = db.queryRaw("SELECT * FROM users WHERE age > 25");

--- Parameterized SQL
json: res = db.exec("INSERT INTO logs (msg) VALUES (?)", ["event"]);

--- Transactions
db.begin();
db.insert(users, "Charlie", 20, "charlie@example.com");
db.commit();

--- Deleting (requires .where())
db.remove(users).where(row -> row.age < 18);

--- Schema
db.sync(users);
db.drop(users);
db.truncate(users);
```

---

## Networking (HTTP)

### Simple HTTP Requests

```xcx
--- GET
json: resp = net.get("https://api.example.com/data");

--- POST
json: resp = net.post("https://api.example.com/users", payload);

--- PUT, DELETE, PATCH
json: resp = net.put("https://api.example.com/users/1", data);
json: resp = net.delete("https://api.example.com/users/1");
```

### Full Request

```xcx
net.request {
    method = "POST",
    url = "https://api.example.com/users",
    headers = ["Authorization" :: "Bearer token123"],
    body = payload,
    timeout = 5000
} as resp;

--- Checking the response
b: ok = resp.ok;
i: status = resp.status;
json: body = resp.body;
```

### HTTP Server

```xcx
fiber getHandler(json: req) {
    s: path = req.url;
    json: response = <<<{ "message": "Hello World" }>>>;
    net.respond(200, response);
}

serve: myServer {
    port = 8080,
    host = "0.0.0.0",
    workers = 4,
    routes = [
        ["GET /"    :: getHandler],
        ["POST /api" :: postHandler]
    ]
};
```

---

## Storage (Files)

```xcx
--- Read / Write
s: content = store.read("data.txt");
store.write("output.txt", "Hello World");
store.append("log.txt", "new line\n");

--- Checking
b: exists = store.exists("file.txt");
i: size = store.size("file.txt");
b: isDir = store.isDir("path/");

--- Directory Operations
store.mkdir("newdir");
array:s files = store.list("mydir");
array:s matches = store.glob("*.xcx");

--- Deleting
store.delete("temp.txt");

--- Archiving
b: zipped = store.zip("folder", "archive.zip");
b: ok = store.unzip("archive.zip", "output/");
```

---

## Terminal

```xcx
--- Print without newline
.terminal!write "Hello ";
.terminal!write "World\n";

--- Clear screen
.terminal!clear;

--- Exit program
.terminal!exit;

--- Run system command
b: ok = .terminal!run "ls -la";

--- Raw mode (for interactive apps)
.terminal!raw;

--- Read key (non-blocking)
s: key = input.key();

--- Read key (blocking)
s: key = input.key() @wait;

--- Check input availability
b: ready = input.ready();

--- Cursor
.terminal!cursor on;
.terminal!cursor off;
.terminal!move 10 5;    --- column 10, line 5

--- Return to normal mode
.terminal!normal;
```

---

## Date and Time

```xcx
--- Current date
date: now = date.now();

--- Date Literal
date: d = date("2024-01-15");
date: d2 = date("15/01/2024", "DD/MM/YYYY");

--- Components
i: year = d.year();
i: month = d.month();
i: day = d.day();
i: hour = d.hour();
i: minute = d.minute();
i: second = d.second();

--- Formatting
s: formatted = d.format("YYYY-MM-DD");
s: withTime = d.format("YYYY-MM-DD HH:mm:ss");

--- Arithmetic
date: tomorrow = d + 1;
date: yesterday = d - 1;
i: diff = date("2024-12-31") - date("2024-01-01");
```

---

## Randomness

```xcx
--- Random integer (range)
i: n = random.int(1, 100);

--- Random integer with step
i: even = random.int(2, 100 @step 2);

--- Random float
f: x = random.float(0.0, 1.0);

--- Random float with step
f: y = random.float(0.0, 10.0 @step 0.5);

--- Random element from collection
array:s colors = ["red", "green", "blue"];
s: picked = random.choice from colors;

set:N nums = set:N { 1,,10 };
i: val = random.choice from nums;
```

---

## Cryptography

```xcx
--- Hashing
s: hash = crypto.hash(password, "bcrypt");
s: hash2 = crypto.hash(password, "argon2");

--- Verification
b: ok = crypto.verify(password, hash, "bcrypt");
b: ok2 = crypto.verify(password, hash2, "argon2");

--- Random token (hex)
s: token = crypto.token(32);   --- 32 hex characters

--- Base64
s: encoded = crypto.hash(data, "base64_encode");
s: decoded = crypto.hash(data, "base64_decode");
```

---

## Diagnostics and Halt

```xcx
--- Print (>!)
>! "Hello World";
>! age;
>! "Value: " + s(count);

--- Input (>?)
>? name;       --- reads into an existing variable
>? age;

--- Halt levels
halt.alert >! "Warning";          --- continues execution
halt.error >! "Logic Error";      --- stops frame
halt.fatal >! "Critical Error";   --- stops frame

--- Wait
@wait(1000);     --- wait 1 second (ms)
@wait 500;       --- alternative syntax
```

---

## Include

```xcx
--- Simple include (deduplicated — once)
include "utils.xcx";

--- Include with alias (all names get a prefix)
include "math.xcx" as math;

--- Usage after aliased include
f: result = math.sin(3.14);
f: cosVal = math.cos(0.0);
```

### Search Paths

1. Relative to the current file's directory
2. In the `lib/` directory (CWD and XCX library paths)

---

## Environment Variables and CLI

```xcx
--- Environment Variable
s: apiKey = env.get("API_KEY");

--- Command Line Arguments
array:s args = env.args();
s: firstArg = args.get(0);
```

---

## Comments

```xcx
--- Single-line comment

---
Multi-line
comment
*---
```
