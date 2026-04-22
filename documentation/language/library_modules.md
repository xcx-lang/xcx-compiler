# XCX 3.0 Standard Library and Modules

## Built-in Modules

### crypto

Cryptography utilities.

| Method                              | Returns | Description                                    |
|-------------------------------------|---------|------------------------------------------------|
| `crypto.hash(data, "bcrypt")`       | `s`     | Hashes password using bcrypt                   |
| `crypto.hash(data, "argon2")`       | `s`     | Hashes password using argon2 (recommended)     |
| `crypto.hash(data, "base64_encode")`| `s`     | Encodes binary data/string to Base64 string    |
| `crypto.hash(data, "base64_decode")`| `s`     | Decodes Base64 string back to binary/string    |
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
| `store.list(p)`     | `(s) → array:s`           | `array:s`| Returns list of files and folders.          |
| `store.isDir(p)`    | `(s) → b`                 | `b`     | `true` if path is a directory.               |
| `store.size(p)`     | `(s) → i`                 | `i`     | File size in bytes.                          |
| `store.mkdir(p)`    | `(s) → b`                 | `b`     | Creates directory (recursive).               |
| `store.glob(pat)`   | `(s) → array:s`           | `array:s`| Returns files matching glob pattern.         |
| `store.zip(s, t)`   | `(s, s) → b`              | `b`     | Archives source to target zip.               |
| `store.unzip(z, d)` | `(s, s) → b`              | `b`     | Extracts zip to destination.                 |

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

| Method                            | Signature           | Returns | Description                                                                 |
|-----------------------------------|---------------------|---------|-----------------------------------------------------------------------------|
| `random.choice from col`          | `(set/array) → T`   | `T`     | Picks a random element from the provided set or array.                      |
| `random.int(min, max @step num)`  | `(i, i, @i) → i`    | `i`     | Picks a random integer in range `[min, max]`. Default step: `1`.            |
| `random.float(min, max @step num)`| `(f, f, @f) → f`    | `f`     | Picks a random float in range `[min, max]`. Default step: `0.5`.            |


```xcx
set:N: pool {1,,10};
i: picked = random.choice from pool;

--- Works on arrays too
array:s: words {"hello", "world"};
s: w = random.choice from words;

--- Picks 1, 3, 5, 7, or 9
i: odd = random.int(1, 10 @step 2);

--- Picks 0.0, 0.5, 1.0, 1.5, or 2.0
f: weight = random.float(0.0, 2.0);

--- Picks 0.0, 0.25, 0.5, 0.75, 1.0
f: precision = random.float(0.0, 1.0 @step 0.25);
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
