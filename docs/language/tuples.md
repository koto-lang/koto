# Tuples

Tuples are like Lists, except they have a fixed size and their entries can't be replaced. 

Tuples are declared with `()` parentheses containing expressions separated by commas. 

```koto
x = (-1, 'abc', true)
print! x[1]
check! abc
```

In simple expressions the `()` parentheses are optional.

```koto
print! 1, 2, 3
check! (1, 2, 3)

print! x = 'a', 10
check! ('a', 10)
print! y = 'b', 20
check! ('b', 20)
print! x, y
check! (('a', 10), ('b', 20))
```

To create an empty Tuple, or a Tuple with a single entry, use a trailing `,` inside the parentheses.

```koto
# An empty pair of parentheses resolves to Null
print! () 
check! null

# A single value in parentheses simply provides the value
print! (1) 
check! 1

# A comma inside parentheses creates a Tuple 
print! (,) 
check! ()
print! (1,)
check! (1)
```

Although Tuples have a fixed structure, mutable values in a Tuple (e.g. Lists and Maps) can still be modified.

```koto
# A Tuple containing two Lists
x = ([1, 2, 3], [4, 5, 6])

# Modify the second List in the Tuple
x[1][0] = 99
print! x
check! ([1, 2, 3], [99, 5, 6])
```
