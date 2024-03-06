# Generators

Generators are iterators that are made by calling _generator functions_,
which are any functions that contain a `yield` expression. 

The generator is paused each time `yield` is encountered, 
waiting for the caller to continue execution.

```koto
my_first_generator = ||
  yield 1
  yield 2

x = my_first_generator()
print! x.next()
check! IteratorOutput(1)
print! x.next()
check! IteratorOutput(2)
print! x.next()
check! null
```

Generator functions can accept arguments like any other function, 
and each time they're called a new generator is created.

As with any other iterable value, the [`iterator`][iterator] module's functions 
are made available to generators.

```koto
make_generator = |x|
  for y in 1..=3
    yield x + y 

print! make_generator(0).to_tuple()
check! (1, 2, 3)
print! make_generator(10)
  .keep |n| n % 2 == 1
  .to_list()
check! [11, 13]
```

## Custom Iterator Adaptors

Generators can also serve as _iterator adaptors_ by modifying the output of 
another iterator. 

Inserting a generator into the [`iterator`][iterator] module makes it available 
in any iterator chain.

```koto
# Make an iterator adaptor that yields every 
# other value from the adapted iterator
iterator.every_other = ||
  n = 0
  # When the generator is created, self is initialized with the previous
  # iterator in the chain, allowing its output to be adapted.
  for output in self
    # If n is even, then yield a value
    if n % 2 == 0
      yield output
    n += 1

print! 1..10
  .each |n| n * 10
  .every_other() # Skip over every other value in the iterator chain
  .to_list()
check! [10, 30, 50, 70, 90]
```

[iterator]: ../core/iterator
