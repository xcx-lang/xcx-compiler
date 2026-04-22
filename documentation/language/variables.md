# XCX 3.0 Variables and Constants

## Variable Declarations

Variables are declared using the `type: name = value;` syntax.

```xcx
i: age = 25;
f: pi = 3.14159;
s: name = "XCX";
b: is_ready = true;
```

Variables can also be declared without an initial value, in which case they take the default value for their type (e.g., `0` for `i`, `""` for `s`).

## Reassignment

Once declared, variables can be reassigned using the `=` operator.

```xcx
i: count = 0;
count = count + 1;
```

## Constants

Constants are declared using the `const` keyword. They must be initialized at declaration and cannot be changed afterwards.

```xcx
const i: MAX_CONNECTIONS = 1024;
const s: VERSION = "2.0.0";
```

## No Variable Shadowing

XCX does **not** support variable shadowing. Declaring a variable with the same name in any scope — including a nested block — is a **compile-time error** (`RedefinedVariable`).

```xcx
i: x = 10;
if (true) then;
    i: x = 20;   --- COMPILE ERROR: RedefinedVariable
end;
```

If you need to change a value inside a block, use reassignment instead of redeclaration:

```xcx
i: x = 10;
if (true) then;
    x = 20;      --- OK: reassignment to existing variable
    >! x;        --- 20
end;
>! x;            --- 20 (the global variable was modified)
```
