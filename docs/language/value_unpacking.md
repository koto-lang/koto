# Value Unpacking

Multiple values can be assigned at once by separating the names with commas.

```koto
a, b = 10, 20
print! a, b
check! (10, 20)

my_tuple = 1, 2
x, y = my_tuple
print! y, x
check! (2, 1)
```

This works with lists too.

```koto
a, b = [10, 20, 30, 40, 50]
print! b, a
check! (20, 10)
```

