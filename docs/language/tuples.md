# Tuples

Tuples are declared with `()` parentheses containing expressions separated by commas. 

```koto
x = (-1, 'abc', true)
print! x[1]
check! abc
```

To create an empty Tuple, or a Tuple with a single entry, use a trailing `,` inside the parentheses.

```koto
print! ()
check! null
print! (1)
check! 1

print! (,)
check! ()
print! (1,)
check! (1)
```

In simple expressions the `()` parentheses are optional.

```koto
print! 1, 2, 3
check! (1, 2, 3)

x = "a", 10
y = "b", 20
print! x, y
check! (('a', 10), ('b', 20))
```

Tuples behave like Lists with a fixed size and with entries that can't be replaced. 

Although Tuples have a fixed structure after they're created, mutable values in a Tuple (like Lists and Maps) can still be modified.

```koto
# Create a Tuple containing two Lists
x = ([1, 2, 3], [4, 5, 6])
# Modify the second List in the Tuple
x[1][0] = 99
print! x
check! ([1, 2, 3], [99, 5, 6])
```
