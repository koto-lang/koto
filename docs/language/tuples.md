# Tuples

Tuples in Koto are similiar to lists, 
but are designed for sequences of values that have a fixed structure.

Unlike lists, tuples can't be resized after creation, 
and values that are contained in the tuple can't be replaced.

Tuples are declared with a series of expressions separated by commas.

```koto
x = 100, 'x', -1
print! x
check! (100, 'x', -1)
```

Parentheses can be used for grouping to avoid ambiguity.

```koto
print! (1, 2, 3), (4, 5, 6)
check! ((1, 2, 3), (4, 5, 6))
```

Access tuple elements by index using square brackets, starting from `0`.

```koto
print! x = 'a', 10
check! ('a', 10)
print! x[0]
check! a
print! x[1]
check! 10

print! y = 'b', 20
check! ('b', 20)
print! x, y
check! (('a', 10), ('b', 20))
```

The `+` operator allows tuples to be joined together, 
creating a new tuple that combines their elements.

```koto
a = 1, 2, 3
b = a + (4, 5, 6)
print! b
check! (1, 2, 3, 4, 5, 6)
```

## Creating Empty Tuples 

An empty pair of parentheses in Koto resolves to `null`.
If an empty tuple is needed then use a single `,` inside parentheses.

```koto
# An empty pair of parentheses resolves to null
print! () 
check! null

# A comma inside parentheses creates a tuple 
print! (,) 
check! ()
```

## Tuple Mutability

While tuples have a fixed structure, mutable values contained in the tuple 
(like lists) can still be modified.

```koto
# A Tuple containing two lists
x = ([1, 2, 3], [4, 5, 6])

# Modify the second list in the tuple
x[1][0] = 99
print! x
check! ([1, 2, 3], [99, 5, 6])
```
