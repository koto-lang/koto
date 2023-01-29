# Value Unpacking

Multiple values can be assigned at once by separating the names with commas.

```koto
a, b = 10, 20
print! a, b
check! (10, 20)
```

If there's a single value on the right-hand side of the assignment, 
then it gets _unpacked_ into the assignment targets.

```koto
my_tuple = 1, 2
x, y = my_tuple
print! y, x
check! (2, 1)
```

If there aren't enough values to unpack, then `null` is assigned to the extra
assignment targets.
 
```koto
a, b, c = [-1, -2]
print! a, b, c
check! (-1, -2, null)
```

Unpacking works with any iterable value, including adapted iterators.

```koto
a, b, c = 1..10
print! a, b, c
check! (1, 2, 3)

a, b, c = (1..10).each |x| x * 10
print! a, b, c
check! (10, 20, 30)
```
