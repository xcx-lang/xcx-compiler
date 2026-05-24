# PAX Package Manager Manual

PAX is the official package manager for XCX, integrated directly into the `xcx` binary. It manages dependencies, project scaffolding, and build automation.

## Project Configuration: `project.pax`

Every PAX project must have a `project.pax` file in its root directory. It uses a custom declarative format.

```pax
---
PAX Project Configuration
*---
/
    name :: "my_project",
    deps :: [
        "user/repo",
        "https://example.com/lib.xcx"
    ]
/
```

- **name**: Logical name of the project.
- **deps**: List of dependencies. Supports GitHub shortcuts (`user/repo`) and direct URLs.

## Command Reference

PAX is invoked via `xcx pax <command>`.

| Command                  | Description                                          |
|--------------------------|------------------------------------------------------|
| `xcx pax new <name>`     | Generates a new project structure.                   |
| `xcx pax clone <package>`| Downloads a published package as a local project.    |
| `xcx pax install`        | Fetches dependencies into the `lib/` directory.      |
| `xcx pax add <dep>`      | Adds a dependency and installs it immediately.       |
| `xcx pax remove <name>`  | Removes a dependency from `project.pax`.             |
| `xcx pax search <query>` | Searches the registry for available packages.        |
| `xcx pax run [path]`     | Executes the project (entry: `src/main.xcx`).        |

### `xcx pax clone <package>`

Downloads a published package from the PAX registry into a new local directory, ready to run or modify.

```sh
xcx pax clone snake_game
xcx pax clone beta/snake_game
```

- The argument is the package name as listed in the registry (optionally prefixed with `author/`).
- Creates a directory named after the package in the current working directory.
- Copies the full project structure: `project.pax`, `src/`, and any other files declared in `files`.
- Does **not** install dependencies automatically — run `xcx pax install` inside the cloned directory afterwards.

```sh
xcx pax clone snake_game
cd snake_game
xcx pax install
xcx pax run
```

## Directory Structure

A standard PAX project follows this layout:
- `project.pax`: Configuration.
- `src/`: Source code (main entry: `main.xcx`).
- `lib/`: Downloaded dependencies (managed by PAX).
- `tests/`: Project-specific tests.
