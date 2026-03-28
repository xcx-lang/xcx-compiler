# XCX 2.2 Standard Library and Modules

## Built-in Modules

### crypto

Cryptography utilities.

| Method                              | Returns | Description                                    |
|-------------------------------------|---------|------------------------------------------------|
| `crypto.hash(password, "bcrypt")`   | `s`     | Hashes password using bcrypt                   |
| `crypto.hash(password, "argon2")`   | `s`     | Hashes password using argon2 (recommended)     |
| `crypto.verify(password, hash, algo)` | `b`   | Returns `true` if password matches hash        |
| `crypto.token(length)`              | `s`     | Generates random hex token of given length     |

```xcx
s: hash  = crypto.hash(password, "bcrypt");
s: hash2 = crypto.hash(password, "argon2");
b: valid = crypto.verify(password, hash2, "argon2");
s: token = crypto.token(32);
```

### store (File I/O)

All paths must be **relative** to the project root. Absolute paths or path traversal (`..`) trigger `halt.fatal`.

| Method              | Signature                 | Returns | Description                                  |
|---------------------|---------------------------|---------|----------------------------------------------|
| `store.write(p, c)` | `(s, s) → b`              | `b`     | Overwrites file. Creates directories if needed. |
| `store.read(p)`     | `(s) → s`                 | `s`     | Returns file contents; `halt.fatal` if missing. |
| `store.append(p, c)`| `(s, s) → b`              | `b`     | Appends to file. Creates if missing.         |
| `store.exists(p)`   | `(s) → b`                 | `b`     | Checks existence. No side effects.           |
| `store.delete(p)`   | `(s) → b`                 | `b`     | Removes file or directory (recursive).       |

```xcx
store.write("log.txt", "First line");
store.append("log.txt", "\nSecond line");
s: content = store.read("log.txt");
if (store.exists("lock.pid")) then;
    >! "Already running";
end;
```

### env

| Method          | Signature      | Returns    | Description                                               |
|-----------------|----------------|------------|-----------------------------------------------------------|
| `env.get(name)` | `(s) → s`      | `s`        | Returns env variable value; `halt.error` if not set       |
| `env.args()`    | `() → array:s` | `array:s`  | Returns CLI arguments passed to the program as an array   |

```xcx
s: db_url = env.get("DATABASE_URL");

array:s: args = env.args();
for arg in args do;
    >! arg;
end;
```

### random

`random.choice from` picks a random element from a **set**. It works exclusively with set types (`set:N`, `set:Z`, `set:Q`, `set:S`, `set:B`, `set:C`).

> [!IMPORTANT]
> `random.choice from` does **not** work with arrays. Use a set if you need random selection.

```xcx
set:N: pool {1,,10};
i: picked = random.choice from pool;

set:S: names {"Alice", "Bob", "Charlie"};
s: winner = random.choice from names;
```

### date (module)

```xcx
date: now = date.now();
```

---

## Module System

### include

Merges code from another file into the current namespace.

```xcx
include "utils.xcx";
include "math.xcx" as m;

m.PI;
m.sqrt(16.0);
```

Without an alias — all symbols are available directly in the current namespace. With an alias — all top-level symbols are prefixed: `alias.symbol`.

Cyclic dependencies are detected and rejected at compile time.