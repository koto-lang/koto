# Generators

Custom iterators can be made with `generator functions`, 
which are any functions that contain a `yield` expression. 

```koto
f = ||
  yield 1
  yield 2
  yield 3

x = f()
print! x.next()
check! 1
print! x.next()
check! 2
print! x.next()
check! 3
print! x.next()
check! null
```

Generator functions can be called with arguments like any other function, 
and their resulting generators have access to the `iterator` module.

```koto
my_generator = |x|
  for y in 1..=3
    yield x + y 

print! my_generator(0).to_list()
check! [1, 2, 3]
print! my_generator(10).to_tuple()
check! (11, 12, 13)
```

A generator that takes an iterator as an argument acts an
iterator adaptor. 

Inserting it into the `iterator` module makes it available
in any iterator chain.

```koto
iterator.every_other = |iter|
  n = 0
  loop
    match iter.next()
      null then 
        return
      value if n % 2 == 0 then 
        yield value
    n += 1

print! (1..=5)
  .each |n| n * 10
  .every_other()
  .to_list()
check! [10, 30, 50]
```

