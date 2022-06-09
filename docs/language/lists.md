# Lists

Lists are declared with `[]` braces and can contain any value types.

Entries in a List can be accessed by index (starting from `0`) by using `[]`
braces.

```koto
x = ['a', 99, true]
print! x[0]
check! a
print! x[1]
check! 99

x[2] = false
print! x[2]
check! false

y = [['a', 'b', 'c'], ['x', 'y', 'z']]
print! y[0][1] 
check! b
print! y[1][2] 
check! z
```

Once a List has been created, its data is shared between instances of the List.

```koto
x = [10, 20, 30]
y = x
y[1] = 99
print! x # x and y share the same data
check! [10, 99, 30]
```
