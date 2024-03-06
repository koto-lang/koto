# Iterators

The elements of a sequence can be accessed sequentially with an _iterator_,
created using the `.iter()` function.

An iterator yields values via [`.next()`][next] until the end of the sequence is
reached, when `null` is returned.

```koto
i = [10, 20].iter()

print! i.next()
check! IteratorOutput(10)
print! i.next()
check! IteratorOutput(20)
print! i.next()
check! null
```

## Iterator Generators

The [`iterator` module][iterator] contains iterator _generators_ like
[`once`][once] and [`repeat`][repeat] that generate output values 
[_lazily_][lazy] during iteration.

```koto
# Create an iterator that repeats ! twice
i = iterator.repeat('!', 2)
print! i.next()
check! IteratorOutput(!)
print! i.next()
check! IteratorOutput(!)
print! i.next()
check! null
```


## Iterator Adaptors

The output of an iterator can be modified using _adaptors_ from the 
[`iterator` module](../core/iterator).

```koto
# Create an iterator that keeps any value above 3
x = [1, 2, 3, 4, 5].keep |n| n > 3

print! x.next()
check! IteratorOutput(4)
print! x.next()
check! IteratorOutput(5)
print! x.next()
check! null
```

## Using iterators with `for`

`for` loops accept any iterable value as input, including adapted iterators.

```koto
for x in 'abacad'.keep |c| c != 'a'
  print x
check! b
check! c
check! d
```

## Iterator Chains

Iterator adaptors can be passed into other adaptors, creating _iterator chains_
that act as data processing pipelines.

```koto
i = (1, 2, 3, 4, 5)
  .skip 1
  .each |n| n * 10
  .keep |n| n <= 40
  .intersperse '--'

for x in i
  print x
check! 20
check! --
check! 30
check! --
check! 40
```

## Iterator Consumers

Iterators can be also be _consumed_ using functions like
[`.to_list()`][to_list] and [`.to_tuple()`][to_tuple], 
allowing the output of an iterator to be easily captured in a container.

```koto
print! [1, 2, 3]
  .each |n| n * 2
  .to_tuple()
check! (2, 4, 6)

print! (1, 2, 3, 4)
  .keep |n| n % 2 == 0
  .each |n| n * 11
  .to_list()
check! [22, 44]
```

[lazy]: https://en.wikipedia.org/wiki/Lazy_evaluation
[iterator]: ../core/iterator
[next]: ../core/iterator#next
[once]: ../core/iterator#once
[repeat]: ../core/iterator#repeat
[to_list]: ../core/iterator#to_list
[to_tuple]: ../core/iterator#to_tuple
