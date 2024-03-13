# Lists

Lists in Koto are created with square brackets (`[]`) and can contain a mix of
different value types.

Access list elements by _index_ using square brackets, starting from `0`.

```koto
x = ['a', 99, true]
print! x[0]
check! a
print! x[1]
check! 99

x[2] = false
print! x[2]
check! false
```

Once a list has been created, its underlying data is shared between other
instances of the same list. 
Changes to one instance of the list are reflected in the other.

```koto
# Assign a list to x
x = [10, 20, 30]

# Assign another instance of the list to y
y = x

# Modify the list through y
y[1] = 99

# The change to y is also reflected in x
print! x 
check! [10, 99, 30]
```

The `+` operator allows lists to be joined together, creating a new list that
combines their elements.

```koto
a = ['a', 'b', 'c']
b = a + [1, 2, 3]
print! b
check! ['a', 'b', 'c', 1, 2, 3]
```
