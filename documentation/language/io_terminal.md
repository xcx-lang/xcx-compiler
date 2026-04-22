# XCX 3.0 I/O and System Commands

## Console Output (`>!`)

The `>!` operator prints values to `stdout`, followed by a newline.

```xcx
>! "Hello";
>! 42;
>! "Path: " + path;
```

**Escape sequences** like `\n` (newline), `\t` (tab), and `\r` (carriage return) are supported.

## User Input (`>?`)

The `>?` operator reads a line from `stdin` and attempts to parse it into the target variable.

```xcx
i: age;
>! "Enter age:";
>? age;
```

## Delay (`@wait`)

Pauses VM execution for a specified number of milliseconds.

```xcx
@wait 1000; --- Waits for 1 second
```

> `@wait` is a **blocking** operation. It does not yield the fiber.

## Raw Input (`input`)

The `input` module allows reading single keys without waiting for Enter.

### Methods

| Call                | Returns | Description                                             |
|---------------------|---------|---------------------------------------------------------|
| `input.key()`       | `s`     | Returns key if available, `""` if not                   |
| `input.key() @wait` | `s`     | Waits for a key and returns it                          |
| `input.ready()`     | `b`     | `true` if a key is waiting in the buffer                |

### Key Constants

| Value           | Key             |
|-----------------|-----------------|
| `"UP"`          | Arrow Up        |
| `"DOWN"`        | Arrow Down      |
| `"LEFT"`        | Arrow Left      |
| `"RIGHT"`       | Arrow Right     |
| `"ENTER"`       | Enter           |
| `"ESC"`         | Escape          |
| `"BACKSPACE"`   | Backspace     |
| `"TAB"`         | Tab             |
| `"F1"` ... `"F12"` | F1–F12 |
| `"KEY_CTRL_C"`  | Ctrl+C          |
| `"KEY_CTRL_Z"`  | Ctrl+Z          |
| `"KEY_CTRL_S"`  | Ctrl+S          |

Regular characters are returned directly: `"a"`, `"Z"`, `"5"`, `" "`.

### Example

s: k = input.key();
if (k == "UP") then;
    y = y - 1;
end;

--- wait for specific key
s: confirm = input.key() @wait;
if (confirm == "q") then;
    return;
end;
```

## Terminal Commands (`.terminal`)

Directly interact with the system environment or current process.

| Directive              | Description                                      |
|------------------------|--------------------------------------------------|
| `.terminal !clear`     | Clears the screen                                |
| `.terminal !exit`      | Terminates the VM process                        |
| `.terminal !run s`     | Runs another XCX file in a new process           |
| `.terminal !raw`       | Raw mode — no echo, no buffering                 |
| `.terminal !normal`    | Restores normal terminal mode                    |
| `.terminal !cursor on` | Shows the cursor                                 |
| `.terminal !cursor off`| Hides the cursor                                 |
| `.terminal !move x y`  | Moves the cursor to position `x`, `y` (`i`)      |
| `.terminal !write expr`| Prints `expr` without a trailing newline         |

### Example — Game Loop

```xcx
.terminal !raw;
.terminal !cursor off;
.terminal !clear;

while (true) do;
    s: k = input.key();
    if (k == "ESC") then; break; end;
    if (k == "UP") then; y = y - 1; end;
    .terminal !move x y;
    .terminal !write "@";
    @wait 16;
end;

.terminal !cursor on;
.terminal !normal;
```

## Error Handling and Constraints

- **`input.key()` in normal mode**: Returns `""` and prints a warning alert.
- **`@wait` on `input.ready()`**: Compilation error.
- **`.terminal` directives**: Are not expressions and cannot be assigned to variables.
- **`.terminal !move`**: Arguments must be integers (`i`).
- **Terminal Availability**: The VM will halt with a fatal error if console handles are redirected or unavailable.
