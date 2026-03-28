# XCX 2.2 Date and Time

## Creation

Dates are created using the `date()` function or `date.now()`.

```xcx
date: d1 = date("2024-12-25");               --- YYYY-MM-DD
date: d2 = date("25/12/2024", "DD/MM/YYYY"); --- Custom format
date: now = date.now();                      --- Current system time
```

## Properties

Date objects expose read-only integer fields:

- `.year`
- `.month` (1-12)
- `.day` (1-31)
- `.hour` (0-23)
- `.minute` (0-59)
- `.second` (0-59)

## Arithmetic and Comparison

Dates support addition/subtraction of days and comparison against other dates.

```xcx
date: tomorrow = some_date + 1;
date: yesterday = some_date - 1;
i: days_between = christmas - halloween;  --- Result in integer days
b: is_before = christmas < new_year;
```

## Formatting

```xcx
s: s1 = now.format();                    --- Default: "YYYY-MM-DD HH:mm:ss"
s: s2 = now.format("DD/MM/YYYY HH:mm");  --- Custom output
```

**Tokens:** `YYYY` (Year), `MM` (Month 01-12), `DD` (Day 01-31), `HH` (Hour 00-23), `mm` (Minute), `ss` (Second), `SSS` or `ms` (Milliseconds), `M` and `D` (No leading zero).
