# Generators

Custom iterators can be made with _generator functions_, which are any functions that contain a `yield` expression. 

The iterator is paused each time `yield` is encountered, waiting for the caller to continue execution.

```koto
my_first_generator = ||
  yield 1
  yield 2
  yield 3

x = my_first_generator()
print! x.next()
check! 1
print! x.next()
check! 2
print! x.next()
check! 3
print! x.next()
check! null
```

Generator functions can have arguments like any other function, and calling them creates an iterator that has access to the `iterator` core library module.

```koto
my_generator = |x|
  for y in 1..=3
    yield x + y 

print! my_generator(0).to_list()
check! [1, 2, 3]
print! my_generator(10).to_tuple()
check! (11, 12, 13)
```

## Iterator adaptors

A generator that modifies another iterator's output is known as an _iterator adaptor_. 

Inserting an adaptor into the `iterator` module makes it available in any iterator chain.

```koto
# Make an iterator adaptor that yields 
# every other value from the adapted iterator
iterator.every_other = ||
  n = 0
  loop
    # When the generator is created, self is initialized with the previous
    # iterator in the chain, allowing its output to be adapted.
    match self.next()
      # Exit when there are no more values produced by the iterator
      null then 
        return
      # If n is even, then yield a value
      value if n % 2 == 0 then 
        yield value
    n += 1

print! 1..10
  .each |n| n * 10
  .every_other()
  .to_list()
check! [10, 30, 50, 70, 90]
```
