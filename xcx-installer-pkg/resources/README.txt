XCX 3.0 — Runtime & Compiler

XCX is an experimental backend programming language with a built-in runtime.
This installer sets up the compiler and basic tooling on your system.

--------------------------------------------------

INSTALLATION

1. Run: xcx-setup.exe
2. Follow the installer steps
3. Open a new terminal (cmd or PowerShell)
4. Verify installation:

   xcx --version

If the command is not recognized, restart your terminal or system.

--------------------------------------------------

USAGE

Run a file:

   xcx file.xcx

Start interactive mode (REPL):

   xcx

Example:

   >! "Hello, world!";

--------------------------------------------------

WHAT IS INCLUDED

- xcx.exe           → compiler + runtime
- PAX               → package manager (preview)
- Basic libraries   → JSON, HTTP, SQLite, crypto, file I/O
- File associations → .xcx files (if enabled during install)

--------------------------------------------------

PROJECT STATUS

XCX 3.0 is under active development.

- Suitable for: small tools, experiments, learning
- Not suitable for: production systems

APIs and behavior may change between versions.

--------------------------------------------------

KNOWN NOTES

- Currently supported platform: Windows
- JIT optimizations apply mainly to loops
- Some features (fibers, database) have known limitations

--------------------------------------------------

DOCUMENTATION

Online:
https:/xcxlang.com/docs

Source documentation may be included separately.

--------------------------------------------------

SUPPORT

Issues and bugs:
https://github.com/xcx-lang/xcx-compiler/issues

--------------------------------------------------

XCX is a personal project focused on runtime design and language architecture.
