# XCX 2.2 I/O and System Commands

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

> [!WARNING]
> **Dynamic Typing at Input**: If the user inputs data that doesn't match the variable's original type (e.g., typing "abc" into an `i`), the variable's type will change to `s` at runtime.

## Delay (`@wait`)

Pauses VM execution for a specified number of milliseconds.

```xcx
@wait 1000; --- Waits for 1 second
```

> `@wait` is a **blocking** operation. It does not yield the fiber.

## Terminal Commands (`.terminal`)

Interact directly with the system environment or current process.

| Command             | Description                                    | Returns |
|---------------------|------------------------------------------------|---------|
| `.terminal !clear`  | Clears the terminal screen.                    | `b`     |
| `.terminal !exit`   | Terminates the VM process immediately.         | -       |
| `.terminal !run s`  | Executes another XCX file in a new process.    | `b`     |

```xcx
if (.terminal !run "tests.xcx") then;
    >! "Tests passed!";
end;
```
