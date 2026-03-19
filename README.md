# XCX 2.0

A statically typed, general-purpose programming language with a backend focus. XCX compiles to bytecode and runs on a custom stack-based virtual machine written in Rust.

```xcx
fiber handle_login(json: req -> json) {
    json: body;
    req.bind("body", body);

    s: username;
    s: password;
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
    routes = [
        "POST /login" :: handle_login,
        "*"           :: handle_404
    ]
};
```

---

## Features

- **Static typing** — `i`, `f`, `s`, `b`, `date`, `json`, `array:T`, `set:N/Z/Q/S/B/C`, `map`, `table`
- **Fibers** — cooperative coroutines with `yield`, `yield from`, and typed return values
- **Built-in HTTP** — `net.get/post/put/delete`, `net.request`, `serve:` server directive
- **Relational tables** — inline table declarations with `.where()`, `.join()`, `.insert()`
- **JSON as first-class type** — raw block literals `<<< {} >>>`, `.bind()`, `.set()`, `.inject()`
- **Math sets** — `set:N`, `set:Z`, `set:Q`, `set:C` with set operators `∪ ∩ \ ⊕`
- **Module system** — `include "file.xcx" as alias;` with circular dependency detection
- **Crypto** — `crypto.hash`, `crypto.verify`, `crypto.token` (bcrypt / argon2)
- **File I/O** — `store.read`, `store.write`, `store.append`, `store.exists`, `store.delete`
- **PAX** — built-in package manager (`xcx pax install`, `xcx pax add`)
- **REPL** — interactive mode via `xcx` with no arguments

---

## Installation

Download the latest installer from the [Releases](https://github.com/xcx-lang/xcx-compiler/releases) page and run:

```
xcx-setup.exe
```

This adds `xcx` to your PATH. To uninstall, run `xcx-uninstall.exe`.

---

## Usage

```bash
# Run a file
xcx main.xcx

# Interactive REPL
xcx

# Package manager
xcx pax install
xcx pax add username/library
xcx pax run

# Version info
xcx --version
```

---

## Building from Source

Requires **Rust 1.75+**.

```bash
git clone https://github.com/xcx-lang/xcx-compiler
cd xcx-compiler
cargo build --release
```

The binary will be at `target/release/xcx`.

---

## Project Structure

```
src/                    # Compiler source (Rust)
  lexer/                # Scanner and token definitions
  parser/               # Pratt parser and AST
  sema/                 # Type checker and symbol table
  backend/              # Bytecode compiler and VM
  diagnostic/           # Error reporter
lib/                    # Standard library (.xcx files)
documentation/
  language/             # Language reference
  compiler/             # Compiler internals
  pax/                  # PAX package manager manual
tests/                  # Test suites
```

---

## Editor Support

A **VS Code extension** is available at [xcx-lang/xcx-vscode](https://github.com/xcx-lang/xcx-vscode).

To install manually:
```bash
code --install-extension xcx-vscode/xcx-vscode-1.0.0.vsix
```

Features: syntax highlighting, snippets, `.xcx` and `.pax` file support.

---

## Documentation

Full documentation is available at **[xcx-lang.github.io]([https://xcx-lang.github.io](https://xcx-lang.github.io/xcx-site))** and locally in [`documentation/language/`](documentation/language/).

| Topic | File |
|---|---|
| Types and variables | `types.md`, `variables.md` |
| Control flow | `control_flow.md` |
| Functions and fibers | `functions_fibers.md` |
| Collections | `collections.md` |
| JSON and HTTP | `json_http.md` |
| Standard library | `library_modules.md` |
| String methods | `string_methods.md` |
| Error handling | `errors_halt.md` |

---

## License

See [`XCX_Ecosystem_v1.0.0/LICENSE.txt`](XCX_Ecosystem_v1.0.0/LICENSE.txt).
