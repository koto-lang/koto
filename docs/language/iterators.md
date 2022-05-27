# Iterators

The entries of a container can be accessed in order via an Iterator,
created with the `.iter()` function.

The iterator yields values via `.next()`, until the end of the sequence is
reached and `null` is returned.

```koto
i = [10, 20].iter()
print! i.next()
check! 10
print! i.next()
check! 20
print! i.next()
check! null
```

Iterators can be _adapted_ using adaptors from the
[`iterator` module](../../core/iterator).
Iterator adaptors will accept any iterable value (which includes all containers),
so it's not necessary to call `.iter()` first.

```koto
x = [1, 2, 3, 4, 5].keep |n| n > 3
print! x.next()
check! 4
print! x.next()
check! 5
print! x.next()
check! null
```

Iterators can be also be _consumed_ using functions like
`.to_list()` and `.to_tuple()`.

```koto
print! [1, 2, 3]
  .each |n| n * 2
  .to_tuple()
check! (2, 4, 6)

print! (11, 22, 33, 44)
  .keep |n| n % 2 == 0
  .to_list()
check! [22, 44]
```

