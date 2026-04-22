# XCX 3.0 Control Flow

## Conditional Statements (if/elseif/else)

```xcx
if (condition) then;
    --- block
elseif (other_condition) then;
    --- block
else;
    --- block
end;
```

**Aliases:**

| Keyword   | Aliases        |
|-----------|----------------|
| `elseif`  | `elif`, `elf`  |
| `else`    | `els`          |

All forms are equivalent and can be mixed:

```xcx
i: score = 75;
if (score >= 90) then;
    >! "A";
elif (score >= 75) then;
    >! "B";
elf (score >= 60) then;
    >! "C";
els;
    >! "F";
end;
```

---

## Loops

### Range For

Range is **inclusive** on both sides.

```xcx
for i in 1 to 3 do;
    >! i;
end;
--- prints: 1, 2, 3
```

### Stepped For (`@step`)

```xcx
for j in 0 to 6 @step 2 do;
    >! j;
end;
--- prints: 0, 2, 4, 6
```

### Collection Iteration

Works on arrays, sets, and fibers. The loop variable receives the **element**, not the index.

```xcx
--- Array
array:i: nums {10, 20, 30};
for el in nums do;
    >! el;
end;

--- Set
set:N: primes {2, 3, 5, 7, 11};
for p in primes do;
    >! p;
end;

--- Fiber
fiber:i: f = gen(5);
for val in f do;
    >! val;
end;
```

### While Loop

```xcx
i: cnt = 0;
while (cnt < 3) do;
    cnt = cnt + 1;
    >! cnt;
end;
```

### Break and Continue

`break` exits the current loop. `continue` skips to the next iteration. Both affect only the **immediately enclosing loop**.

```xcx
for n in 1 to 5 do;
    if (n % 2 == 0) then; continue; end;
    if (n == 5) then; break; end;
    >! n;
end;
```

> [!NOTE]
> `break` inside a `for` loop over a fiber automatically calls `.close()` on that fiber.
