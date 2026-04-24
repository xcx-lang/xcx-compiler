![XCX Banner](https://raw.githubusercontent.com/xcx-lang/xcx-vscode/main/images/banner.png)

![Rust](https://img.shields.io/badge/built%20with-Rust-orange)
![License](https://img.shields.io/badge/license-Apache%202.0-blue)
![Version](https://img.shields.io/badge/version-3.0.0-brightgreen)
![Platform](https://img.shields.io/badge/platform-Windows-lightgrey)
![GitHub Stars](https://img.shields.io/github/stars/xcx-lang/xcx-compiler?style=flat)
![GitHub Issues](https://img.shields.io/github/issues/xcx-lang/xcx-compiler)
![Last Commit](https://img.shields.io/github/last-commit/xcx-lang/xcx-compiler)
![Repo Size](https://img.shields.io/github/repo-size/xcx-lang/xcx-compiler)

> XCX 3.0 is an active project under development. If you run into something unexpected, [open an issue](https://github.com/xcx-lang/xcx-compiler/issues). XCX 4.0 is planned with a redesigned architecture.

---

## Why XCX exists

Most backend languages make you choose between two bad options: high-level languages that are productive but drag in frameworks, ORMs, and config files you didn't ask for — or low-level languages that give you control but make a simple HTTP endpoint feel like work.

XCX is an experiment in a third path: a statically typed language where HTTP, SQLite, JSON, crypto, and file I/O are part of the language itself, not libraries you bolt on. No `package.json`. No ORM. No middleware boilerplate. You write logic; the runtime handles the rest.

It started in December 2025 as a question — *can an AI generate a working language runtime from scratch?* — and went through a Python prototype, a C rewrite, and finally a Rust implementation that became XCX 3.0. One contributor so far. The architecture got complex along the way, which is why 4.0 is planned. But the core idea holds up.

---

## What XCX looks like

```xcx
fiber handle_login(json: req -> json) {
    json: body; req.bind("body", body);

    s: username; s: password;
    body.bind("username", username);
    body.bind("password", password);

    s: hash  = crypto.hash(password, "argon2");
    s: token = crypto.token(32);

    json: resp <<< {"ok": true, "token": ""} >>>;
    resp.set("token", token);
    yield net.respond(200, resp);
};

serve: api {
    port   = 8080,
    routes = ["POST /login" :: handle_login, "*" :: handle_404]
};
```

**HTTP server with SQLite in ~20 lines:**

```xcx
database: app { engine = "sqlite", path = "app.db" };

table: users {
    columns = [ id :: i @auto @pk, name :: s @unique, age :: i ]
    rows = [EMPTY]
};

fiber handle_users(json: req -> json) {
    table: all = yield app.fetch(users);
    yield net.respond(200, all.toJson());
};

fiber handle_create(json: req -> json) {
    json: body; req.bind("body", body);
    s: name; body.bind("name", name);
    i: age;  body.bind("age", age);

    yield app.insert(users, name = name, age = age) as saved;

    json: resp <<< {"ok": true, "id": 0} >>>;
    resp.set("id", saved.insertId);
    yield net.respond(201, resp);
};

serve: api {
    port = 8080,
    routes = ["GET /users" :: handle_users, "POST /users" :: handle_create]
};
```

---

## How XCX compares to alternatives

The honest picture — what you gain and what you give up:

| | Go | Node.js | Python | XCX |
|---|---|---|---|---|
| HTTP server | stdlib + router | Express / Fastify | Flask / FastAPI | built-in `serve:` |
| Database | GORM / sqlx | Prisma / Knex | SQLAlchemy | built-in `database:` |
| JSON | `encoding/json` + structs | native | `json` module | first-class type `json` |
| Crypto | `crypto` stdlib | `bcrypt` npm package | `bcrypt` pip | built-in `crypto.*` |
| Type safety | strong | optional (TS) | optional (mypy) | static, compile-time |
| Concurrency model | goroutines | event loop | async/await | cooperative fibers |
| Ecosystem | large | very large | very large | minimal (early stage) |
| Windows support | yes | yes | yes | only platform currently |

XCX is not trying to replace Go or Node. It occupies a different space: small backend services and tools where you want zero dependency setup and a language that knows what you're building. The trade-off is an early-stage ecosystem and a single supported platform.

---

## Performance (current state)

Benchmarks run on Windows 11, Ryzen 5 5600X, 16GB RAM. XCX uses a register-based VM with a tracing JIT (Cranelift) that kicks in automatically on hot loops after ~50 iterations.
> ⚠️ These benchmarks reflect the **current state of XCX 3.0**, not the target performance.
> The runtime, VM, and JIT are still under active development and will change significantly.
> 
> The goal of this section is **transparency**, not competition.

| Language | Loop (100M) | Fibonacci (30) | Sieve | JSON |
|---|---|---|---|---|
| Rust | 29.52ms | 1.79ms | 0.12ms | N/A |
| Java | 34.1ms | 2.2ms | 2.1ms | N/A |
| Go | 84.42ms | 3.27ms | 0.10ms | 60.46ms |
| Nim | 89ms | 18ms | 0.2ms | 58.9ms |
| C++ | 84.76ms | 1.03ms | 0.09ms | N/A |
| C | 85.09ms | 1.01ms | 0.10ms | N/A |
| V | 89.45ms | 1.32ms | 0.16ms | N/A |
| Crystal | 90.9ms | 2.96ms | 0.29ms | N/A |
| Node.js | 358.89ms | 6.54ms | 2.28ms | 8.12ms |
| LuaJIT | 378ms | 9.1ms | 0.8ms | N/A |
| **XCX 3.0** | **521ms** | **60ms** | **5ms** | **118ms** |
| PHP | 3219.35ms | 80.33ms | 4.21ms | 10.83ms |
| Lua | 5766ms | 82.8ms | 7ms | N/A |
| Python | 11094.20ms | 100.65ms | 3.72ms | 38.15ms |
| R | 23327ms | 580ms | 3ms | N/A |

XCX is not yet performance-competitive with compiled languages in general workloads.

Loop and Sieve are competitive. Fibonacci dropped from 601ms to 60ms after recent optimizations. JSON is slower than the scripting languages that have native JSON support, which is a known area for improvement. These numbers reflect the current 3.0 architecture.

---

## Architecture

XCX compiles source code through a multi-stage pipeline, all implemented in Rust (~5k lines):

```
Source (.xcx)
  → Lexer        byte scanner on &[u8], no allocation, manual UTF-8 handling
  → Pratt Parser top-down operator precedence, one-token lookahead
  → Expander     resolves include directives, alias prefixing
  → Sema         type checker, symbol table, collects all errors before codegen
  → Compiler     two-pass, emits register-based bytecode + source spans
  → VM           register VM, NaN-boxed 64-bit values, Arc ref counting
  → JIT          Cranelift tracing JIT, hot loops compiled to native machine code
```

**NaN-boxing** — every value fits in a single `u64`. Scalars (int, float, bool, date) require zero heap allocation. Pointers to heap objects (strings, arrays, JSON, tables, fibers) live in the lower 48 bits. The JIT exploits this: incrementing a NaN-boxed integer is a single `iadd_imm` on the full 64-bit word — the tag bits in the high end are unaffected.

**Fibers** — cooperative coroutines backed by saved `Vec<Value>` state. Not OS threads. Suspend/resume moves the locals vector without copying. Each HTTP handler runs as a fiber; the server spawns N OS worker threads, each with its own executor. Globals are shared via `Arc<RwLock<Vec<Value>>>`.

**JIT** — backward jumps (loop edges) are counted per instruction pointer. After 50 iterations, trace recording starts. The trace is specialized for the runtime types seen (integer guards, float guards), then compiled by Cranelift to native code. A failed guard falls back to the interpreter at the correct IP. Function calls and string operations are not currently JIT-compiled.

Full compiler internals: [`documentation/compiler/`](documentation/compiler/)

---
## Project status

XCX is currently developed by a single contributor.

The language is usable for small backend tools and experimental services, but it is not production-ready for large systems. The project is primarily focused on runtime design and architecture at this stage.

The current implementation reflects multiple iterations (Python → C → Rust), which resulted in some architectural complexity — most notably in the VM and fiber execution model.

**What works well:** HTTP servers, SQLite integration, JSON handling, file I/O, cooperative concurrency, interactive terminal programs, and numeric workloads that benefit from JIT-optimized loops.

**Known rough edges:** Recursive function performance (no JIT coverage), a fiber scoping workaround required on Windows (see [`database.md`](documentation/language/database.md)), and VM complexity that can make certain edge-case bugs difficult to isolate.

The ecosystem is minimal and evolving. APIs and internal behavior may change across minor versions.

Contributions are welcome — bug reports and pull requests are appreciated. There is no formal contribution process yet; for larger changes, please open an issue first.


---

## Roadmap

### XCX 3.x — stabilization

The 3.x line is not planned for new language features. Focus is on:

- Bug fixes reported by users
- Better error messages and diagnostics
- PAX package manager stabilization and registry
- More example projects
- VS Code extension improvements
- Documentation gaps

### XCX 4.0 — architectural rewrite (planned, no timeline)

4.0 is a planned significant redesign. The current VM and fiber model carry technical debt that makes correctness and performance improvements increasingly difficult. Goals:

- Redesigned VM architecture, resolving current complexity
- Native fiber scoping fix (the Windows workaround goes away)
- Improved JIT coverage, including function calls
- Cross-platform support (Linux, macOS)
- No planned changes to XCX language syntax — 3.0 code should continue to work

---

## Getting started

**1. Download** the installer from [Releases]([https://github.com/xcx-lang/xcx-compiler/releases](https://github.com/xcxlang-org/xcx/releases)): `xcx-setup.exe`

This adds `xcx` to your PATH. To uninstall: `xcx-uninstall.exe`.

**2. Hello world** — save as `hello.xcx`:

```xcx
>! "Hello, world!";
```

```bash
xcx hello.xcx
```

**3. Try the REPL:**

```bash
xcx
xcx> i: x = 2 ^ 10;
xcx> >! x;
1024
xcx> !exit
```

**4. Minimal HTTP server** — save as `server.xcx`:

```xcx
fiber handle(json: req -> json) {
    yield net.respond(200, <<< {"ok": true} >>>);
};

serve: api { port = 8080, routes = ["*" :: handle] };
```

```bash
xcx server.xcx
# GET http://localhost:8080 → {"ok":true}
```

---

## Core features

**Static typing** — `i`, `f`, `s`, `b`, `date`, `json`, `array:T`, `set:N/Z/Q/S/B/C`, `map`, `table`. Wrong types, missing fields, and unsafe queries are caught at compile time.

**Fibers** — cooperative coroutines with `yield`, `yield from`, and typed return values. Every HTTP handler is a fiber.

**Native SQL** — declare a `table:`, connect a `database:`, call `sync()`. No ORM, no migrations file, no config. SQLite out of the box.

**JSON as a first-class type** — raw literals `<<< {} >>>`, `.bind()`, `.set()`, `.inject()`. JSON is how you talk to the outside world.

**Built-in HTTP** — client (`net.get/post/put/delete`) and server (`serve:`). Routes, handlers, CORS, and status codes — all in the language.

**Crypto and file I/O** — `crypto.hash`, `crypto.verify`, `crypto.token`, `store.read/write/append/glob/zip`.

**Terminal + interactive input** — raw mode, cursor control, non-blocking key input. Enough to build games, editors, and CLI tools.

**PAX package manager** — `xcx pax install pkg`. Own registry, preview in 3.0.

---

## Building from source

Requires **Rust 1.75+**.

```bash
git clone https://github.com/xcx-lang/xcx-compiler
cd xcx-compiler
cargo build --release
```

Binary: `target/release/xcx`

---

## Editor support

VS Code extension: [xcx-lang/xcx-vscode]([https://github.com/xcx-lang/xcx-vscode](https://github.com/xcxlang-org/xcx-vscode))

Syntax highlighting, snippets, `.xcx` and `.pax` support.

```bash
code --install-extension xcx-vscode/xcx-vscode-1.0.0.vsix
```

---

## Documentation

Full docs at **[xcxlang.com](https://xcxlang.com)**

### Language

| Topic | File |
|---|---|
| Types and variables | [`types.md`](documentation/language/types.md), [`variables.md`](documentation/language/variables.md) |
| Syntax basics | [`syntax.md`](documentation/language/syntax.md) |
| Operators | [`operators.md`](documentation/language/operators.md) |
| Control flow | [`control_flow.md`](documentation/language/control_flow.md) |
| Functions and fibers | [`functions_fibers.md`](documentation/language/functions_fibers.md) |
| Collections | [`collections.md`](documentation/language/collections.md) |
| String methods | [`string_methods.md`](documentation/language/string_methods.md) |
| JSON and HTTP | [`json_http.md`](documentation/language/json_http.md) |
| Database | [`database.md`](documentation/language/database.md) |
| Dates | [`dates.md`](documentation/language/dates.md) |
| I/O and terminal | [`io_terminal.md`](documentation/language/io_terminal.md) |
| Standard library | [`library_modules.md`](documentation/language/library_modules.md) |
| Error handling | [`errors_halt.md`](documentation/language/errors_halt.md) |

### Compiler internals

| Topic | File |
|---|---|
| Overview | [`README.md`](documentation/compiler/README.md) |
| Language spec | [`language.md`](documentation/compiler/language.md) |
| Lexer | [`lexer.md`](documentation/compiler/lexer.md) |
| Parser | [`parser.md`](documentation/compiler/parser.md) |
| Semantic analysis | [`sema.md`](documentation/compiler/sema.md) |
| Expander | [`expander.md`](documentation/compiler/expander.md) |
| Backend | [`backend.md`](documentation/compiler/backend.md) |
| JIT | [`jit.md`](documentation/compiler/jit.md) |

### Package manager

| Topic | File |
|---|---|
| PAX manual | [`pax_manual.md`](documentation/pax/pax_manual.md) |

---

## License

Apache 2.0 — see [LICENSE](LICENSE)
