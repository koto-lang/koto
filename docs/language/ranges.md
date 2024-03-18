# Ranges

Ranges of integers can be created with `..` or `..=`.

`..` creates a _non-inclusive_ range, 
which defines a range up to but _not including_ the end of the range.

```koto
# Create a range from 10 to 20, not including 20
print! r = 10..20
check! 10..20
print! r.start()
check! 10
print! r.end()
check! 20
print! r.contains 20
check! false
```

`..=` creates an _inclusive_ range, which includes the end of the range.

```koto
# Create a range from 10 to 20, including 20
print! r = 10..=20
check! 10..=20
print! r.contains 20
check! true
```

If a value is missing from either side of the range operator then an _unbounded_
range is created.

```koto
# Create an unbounded range starting from 10
r = 10..
print! r.start()
check! 10
print! r.end()
check! null
 
# Create an unbounded range up to and including 10
r = ..=100
print! r.start()
check! null
print! r.end()
check! 100
```

_Bounded_ ranges are declared as iterable, 
so they can be used in for loops and with the [`iterator`][iterator] module.

```koto
for x in 1..=3
  print x
check! 1
check! 2
check! 3

print! (0..5).to_list()
check! [0, 1, 2, 3, 4]
```

## Slices

Ranges can be used to create a _slice_ of a container's data.

```koto
x = (10, 20, 30, 40, 50)
print! x[1..=3] 
check! (20, 30, 40)
```

For immutable containers like tuples and strings, 
slices share the original value's data, with no copies being made.

For mutable containers like lists, creating a slice makes a copy of the sliced 
portion of the underlying data.

```koto
x = 'abcdef'
# No copies are made when a string is sliced
print! y = x[3..6]
check! def

a = [1, 2, 3]
# When a list is sliced, the sliced elements get copied into a new list
print! b = a[0..2]
check! [1, 2]
print! b[0] = 42
check! 42
print! a[0]
check! 1
```

When creating a slice with an unbounded range, 
if the start of the range if ommitted then the slice starts from the beginning 
of the container. 
If the end of the range is ommitted, then the slice includes all remaining 
elements in the container.

```koto
z = 'Hëllø'.to_tuple()
print! z[..2]
check! ('H', 'ë')
print! z[2..]
check! ('l', 'l', 'ø')
```

[iterator]: ../core_lib/iterator
