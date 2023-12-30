# Ranges

Ranges of integers can be created with `..` or `..=`.

`..` creates a _non-inclusive_ range, which defines a range from the start 
_up to but not including_ the end of the range.

```koto
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
print! r = 100..=200
check! 100..=200
print! r.contains 200
check! true
```

If a value is missing from either side of the range operator then an _unbounded_
range is created.

```koto
r = 10..
print! r.start()
check! 10
print! r.end()
check! null
 
r = ..=100
print! r.start()
check! null
print! r.end()
check! 100
```

_Bounded_ ranges are iterable, so can be used in for loops, and with the 
`iterator` module.

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

For tuples and strings, slices share the original container's data, which
avoids making copies of the elements in the slice. For lists (which contain 
mutable data), copies of the slices elements are made.

If a range doesn't have a defined start, then the slice starts from the
beginning of the container's data. Similarly, if a range doesn't have a defined
end, then the slice includes elements up to the end of the container's data.

```koto
z = 'Hëllø'
print! z[..2]
check! Hë
print! z[2..]
check! llø
```
