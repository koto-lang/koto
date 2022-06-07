# Tuples

Tuples are declared with `()` parentheses. 

```koto
x = (-1, 'abc', true)
print! x[1]
check! abc
```

To create an empty Tuple or a Tuple with a single entry, use a trailing `,` inside the parentheses.

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
check! (("a", 10), ("b", 20))
```

Tuples behave like Lists with a fixed size and with entries that can't be replaced, 
however Lists and Maps that are contained in a Tuple can be modified due to
their interior mutability.

```koto
x = ([1, 2, 3], [4, 5, 6])
x[1][0] = 99
print! x[1]
check! [99, 5, 6]
```
