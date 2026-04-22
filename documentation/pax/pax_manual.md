# PAX Package Manager Manual (v3.0)

PAX is the official package manager for XCX, providing project scaffolding, dependency management with version pinning, and a professional registry integration.

## Project Configuration: `project.pax`

Every PAX project is centered around a `project.pax` file. It uses a custom declarative format supporting professional metadata.

```pax
---
PAX Project Configuration
*---
/
    name        :: "my_project",
    version     :: "1.0.0",
    author      :: "DeveloperName",
    description :: "A quick description of the project library.",
    main        :: "src/app.xcx",          --- Custom entry point (optional)
    tags        :: ["math", "utility"],
    deps        :: [
        "mathlib@1.2.0",            --- Pin to specific version
        "user/repo",                --- GitHub shortcut (latest)
        "https://domain.com/lib.xcx" --- Direct URL
    ]
/
```

### Core Metadata:
- **name**: Unique package identifier.
- **version**: Semantic version (e.g., "1.0.0").
- **author**: Developer name (synced with registry).
- **main**: Path to the primary source file (default: `src/main.xcx`).
- **deps**: List of dependencies. Supported formats:
    - `name`: Fetches latest from `pax.xcxlang.com`.
    - `name@version`: Fetches specific version.
    - `user/repo`: GitHub shortcut.

## Command Reference

### Core Commands
| Command                | Description                                          |
|------------------------|------------------------------------------------------|
| `xcx pax new <name>`    | Generates a professional project structure.           |
| `xcx pax install`        | Installs deps (minimum library files only).         |
| `xcx pax clone <name>`   | Clones the entire package repository/structure.      |
| `xcx pax add <dep>`      | Adds a dep (supports `@version`) and installs it.     |
| `xcx pax remove <name>`  | Removes a package from `project.pax` and `pax.lock`.  |
| `xcx pax search <query>` | Searches the PAX Registry (`pax.xcxlang.com`).      |
| `xcx pax run [path]`    | Executes the project (entry defined in `main`).      |

### Registry Commands (Authentication Required)
| Command                | Description                                          |
|------------------------|------------------------------------------------------|
| `xcx pax login <token>` | Saves your API token for publishing.                 |
| `xcx pax logout`        | Clears the stored session.                           |
| `xcx pax whoami`        | Verifies your registry account and role.             |
| `xcx pax publish`       | Pushes your project manifest to the registry.        |

## Deterministic Builds: `pax.lock`

When you run `install` or `add`, PAX generates a `pax.lock` file. This file "locks" your dependencies to specific versions and source files, ensuring that everyone on your team has exactly the same environment. **Always commit your `pax.lock` to version control.**

## Directory Structure
- `project.pax`: Main configuration.
- `pax.lock`: Dependency lockfile.
- `src/`: Source code.
- `lib/`: Downloaded dependencies (standard format: `lib/[package_name]/`).
- `README.md`: Project documentation.
