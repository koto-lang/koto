# Value Unpacking

Multiple assignments can be performed in a single expression by separating the 
variable names with commas.

```koto
a, b = 10, 20
print! a, b
check! (10, 20)
```

If there's a single value being assigned, and the value is iterable, 
then it gets _unpacked_ into the target variables.

```koto
my_tuple = 1, 2
x, y = my_tuple
print! y, x
check! (2, 1)
```

Unpacking works with any iterable value, including adapted iterators.

```koto
a, b, c = 1..10
print! a, b, c
check! (1, 2, 3)

x, y, z = (a, b, c).each |x| x * 10
print! x, y, z
check! (10, 20, 30)
```

If the value being unpacked doesn't contain enough values for the assignment,
then `null` is assigned to any remaining variables.
 
```koto
a, b, c = [-1, -2]
print! a, b, c
check! (-1, -2, null)

x, y, z = 42
print! x, y, z
check! (42, null, null)
```

