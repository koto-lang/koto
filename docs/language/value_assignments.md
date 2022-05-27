# Value Assignments

Values are assigned with `=`, and can be freely reassigned.

```koto
x = 42
print! x
check! 42

x = true
print! x
check! true
```

Arithmetic assignment operators are available, e.g. `x *= y` is shorthand for 
`x = x * y`.

```koto
a = 100
a += 11
print! a
check! 111
```

