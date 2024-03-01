# Iterators

The entries of a container can be accessed in order via an Iterator,
created with the `.iter()` function.

The iterator yields values via `.next()`, until the end of the sequence is
reached and `null` is returned.

```koto
i = [10, 20].iter()

print! i.next()
check! IteratorOutput(10)
print! i.next()
check! IteratorOutput(20)
print! i.next()
check! null
```

## Iterator Adaptors

Iterators can be _adapted_ using adaptors from the [`iterator` module](../../core/iterator).
Note that iterator adaptors will accept any iterable value (which includes all containers),
so it's not necessary to call `.iter()` first.

```koto
x = [1, 2, 3, 4, 5].keep |n| n > 3

print! x.next()
check! IteratorOutput(4)
print! x.next()
check! IteratorOutput(5)
print! x.next()
check! null
```
## Iterator Chains

Iterator adaptors can be passed into other adaptors, creating _iterator chains_
that can be as long as you like.

```koto
print! x = (1, 2, 3, 4, 5)
  .skip 2
  .each |n| n * 10
  .keep |n| n < 50
  .intersperse 'x'
check! Iterator
print! x.next()
check! IteratorOutput(30)
print! x.next()
check! IteratorOutput(x)
print! x.next()
check! IteratorOutput(40)
print! x.next()
check! null
```

## Iterator Consumers

Iterators can be also be _consumed_ using functions like
`.to_list()` and `.to_tuple()`.

```koto
print! [1, 2, 3]
  .each |n| n * 2
  .to_tuple()
check! (2, 4, 6)

print! (11, 22, 33, 44)
  .keep |n| n % 2 == 0
  .each |n| n / 11
  .each number.to_int
  .to_list()
check! [2, 4]
```
