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

Ranges are iterable, so can be used in for loops, and with the `iterator` module.

```koto
for x in 1..=3
  print x
check! 1
check! 2
check! 3

print! (0..5).to_list()
check! [0, 1, 2, 3, 4]
```

