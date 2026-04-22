# XCX 3.0 Error Handling (Halt)

XCX uses a structured `halt` system for managing runtime conditions and errors.

## Halt Levels

| Level         | Behavior                                          | Use Case                      |
|---------------|---------------------------------------------------|-------------------------------|
| `halt.alert`  | Prints a message; execution continues.            | Logging, non-critical warnings|
| `halt.error`  | Prints to stderr; aborts the current frame.       | Recoverable logic errors      |
| `halt.fatal`  | Prints a message; terminates the VM immediately. | Critical failure, security breach |

## Examples

```xcx
halt.alert >! "Cache missed, fetching from DB...";

if (divisor == 0) then;
    halt.error >! "Division by zero!";
    return 0; --- Execution returns to caller from the recovery point
end;

if (NOT db.is_healthy()) then;
    halt.fatal >! "Database corrupted. Emergency shutdown.";
end;
```

## Semantic and Runtime Panics

Certain invalid operations result in an automatic **panic** (equivalent to `halt.fatal` or `halt.error` depending on context):
- **Division by zero** (arithmetic)
- **JSON Parse Failure**: Invalid string in `json.parse()` results in an immediate VM exit.
- **Path Traversal**: Using `..` in `store` methods or absolute paths outside the project root triggers `halt.fatal`.
- **Recursion Depth**: Exceeding 800 frames triggers `halt.error`.
