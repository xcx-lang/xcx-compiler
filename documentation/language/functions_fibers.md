# XCX 3.0 Functions and Fibers

> [!IMPORTANT]
> **Global Scope Rule**: Functions and fibers must be declared at the **top level** of a file. They cannot be nested inside `if`/`while`/`for` blocks or other functions/fibers in a way that affects scoping — all function and fiber names are registered globally during compilation.

## Functions

### Definition Styles

Parameters follow the `type: name` pattern (e.g., `i: count`).

```xcx
--- No parameters, no return value
func hello() {
    >! "Hello!";
};

--- No parameters, with return value
func get_pi(-> f) {
    return 3.14159;
};

--- With parameters, no return value
func greet(s: who) {
    >! "Hello, " + who + "!";
};

--- With parameters and return value
func add(i: x, i: y -> i) {
    return x + y;
};
```

### Calling Functions

```xcx
hello();
f: pi = get_pi();
greet("Alice");
i: result = add(3, 4);
```

### Recursion

Max depth: **800 frames**. Exceeding this triggers `halt.error`.

```xcx
func factorial(i: n -> i) {
    if (n <= 1) then;
        return 1;
    else;
        return n * factorial(n - 1);
    end;
};
>! factorial(5);   --- 120
```

---

## Fibers (Coroutines)

Fibers can suspend execution with `yield` and be resumed. There are two variants: **typed** (yield returns a value) and **void** (yield without a value).

### Definition

```xcx
--- Typed Fiber: each yield returns a value of type T
fiber generator(i: max -> i) {
    for i in 1 to max do;
        yield i;
    end;
    return 0;
};

--- Void Fiber: yield without a value
fiber logger(s: msg) {
    if (msg == "") then; return; end;
    >! msg;
    yield;
};
```

### Instantiation

```xcx
fiber:i: gen  = generator(10);   --- typed
fiber:  log   = logger("init");  --- void
```

### Fiber Methods

| Method      | Works on   | Returns | Description                              |
|-------------|------------|---------|------------------------------------------|
| `.next()`   | `fiber:T`  | `T`     | Resumes; returns next yielded value      |
| `.run()`    | `fiber:`   | `b`     | Resumes void fiber one step              |
| `.isDone()` | both       | `b`     | `true` when fiber has finished           |
| `.close()`  | both       | `b`     | Forcefully terminates the fiber          |

Calling `.next()` when `isDone() == true` triggers `halt.alert` (R306).

### yield

```xcx
--- In a typed fiber
yield true;
yield score + 10;

--- In a void fiber
yield;
```

### return in Fibers

1. **Typed fiber** (`fiber ... -> T`): Requires `return expr;` with a value of type `T`. A bare `return;` without a value is a compile error (S210).
2. **Void fiber**: Uses `return;` to exit early.

```xcx
fiber:i: gen(i: max -> i) {
    for i in 1 to max do;
        if (i == 3) then;
            return 99;   --- Valid: terminates and returns 99
        end;
        yield i;
    end;
    return 0;            --- Required at end of typed fiber
};

fiber logger(s: msg) {
    if (msg == "") then;
        return;          --- Valid for void fiber
    end;
    >! msg;
};
```

### for Loop over a Fiber

```xcx
fiber:i: f = generator(5);
for val in f do;
    >! val;
end;
```

Equivalent to `while (!f.isDone()) { val = f.next(); ... }`. `break` inside the loop automatically calls `.close()`.

### Fiber as Function Parameter

```xcx
func run_checks(fiber:b: f) {
    while (f.isDone() == false) do;
        b: result = f.next();
        if (result == false) then;
            halt.alert >! "Step failed";
            return;
        end;
    end;
};

fiber:b: checks = login_pipeline(user, pass);
run_checks(checks);
```

### yield from (Delegation)

Delegates execution to a sub-fiber. The compiler desugars this into a loop that yields every value from the sub-fiber.

```xcx
fiber count(i: n -> i) {
    i: x = 0;
    while (x < n) do;
        yield x;
        x = x + 1;
    end;
    return 0;
};

fiber delegator(-> i) {
    fiber:i: sub = count(3);
    yield from sub;   --- yields: 0, 1, 2
    yield 100;        --- then yields: 100
    return 0;
};
```

### Manual Delegation

```xcx
fiber pipeline(s: user, s: pass -> b) {
    fiber:b: step1 = check_user(user);
    yield step1.next();

    fiber:b: step2 = check_pass(user, pass);
    yield step2.next();

    return false;
};
```
