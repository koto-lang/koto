# iterator

## all

```kototype
|Iterable, test: |Any| -> Bool| -> Bool
```

Checks the Iterable's values against a test function.

The test function should return `true` if the value passes the test, otherwise
it should return `false`. 

`all` will return `true` if _all_ values pass the test, otherwise it will return
`false`.

`all` stops running as soon as it finds a value that fails the test.

### Example

```koto
print! (1..9).all |x| x > 0
check! true

print! ('', '', 'foo').all string.is_empty
check! false

print! [10, 20, 30]
  .each |x| x / 10
  .all |x| x < 10
check! true
```

### See Also

- [`iterator.any`](#any)

## any

```kototype
|Iterable, test: |Any| -> Bool| -> Bool
```

Checks the Iterable's values against a test function.

The test function should return `true` if the value passes the test, otherwise
it should return `false`. 

`any` will return `true` if _any_ of the values pass the test, 
otherwise it will return `false`.

`any` stops running as soon as it finds a passing test.

### Example

```koto
print! (1..9).any |x| x == 5
check! true

print! ('', '', 'foo').any string.is_empty
check! true

print! [10, 20, 30]
  .each |x| x / 10
  .any |x| x == 2
check! true
```

### See Also

- [`iterator.all`](#all)

## chain

```kototype
|first: Iterable, second: Iterable| -> Iterator
```

`chain` returns an iterator that iterates over the output of the first iterator,
followed by the output of the second iterator.

### Example

```koto
print! [1, 2]
  .chain 'abc'
  .to_tuple()
check! (1, 2, 'a', 'b', 'c')
```

## chunks

```kototype
|Iterable, size: Number| -> Iterator
```

Returns an iterator that splits up the input data into chunks of size `N`,
where each chunk is provided as a Tuple.
The final chunk may have fewer than `N` elements.

### Example

```koto
print! 1..=10
  .chunks 3
  .to_list()
check! [(1, 2, 3), (4, 5, 6), (7, 8, 9), (10)]
```

## consume

```kototype
|Iterable| -> Null
```

Consumes the output of the iterator.

```kototype
|Iterable, |Any| -> Any| -> Null
```

Consumes the output of the iterator, calling the provided function with each
iterator output value.

### Example

```koto
result = []
1..=10
  .keep |n| n % 2 == 0
  .each |n| result.push n
  .consume()
print! result
check! [2, 4, 6, 8, 10]

# Alternatively, calling consume with a function is equivalent to having an
# `each` / `consume` chain
result = []
1..=10
  .keep |n| n % 2 == 1
  .consume |n| result.push n
print! result
check! [1, 3, 5, 7, 9]
```

## count

```kototype
|Iterable| -> Number
```

Counts the number of items yielded from the iterator.

### Example

```koto
print! (5..15).count()
check! 10

print! 0..100
  .keep |x| x % 2 == 0
  .count()
check! 50
```

## cycle

```kototype
|Iterable| -> Iterator
```

Takes an Iterable and returns a new iterator that endlessly repeats the
iterable's output.

The iterable's output gets cached, which may result in a large amount of memory
being used if the cycle has a long length.

### Example

```koto
print! (1, 2, 3)
  .cycle()
  .take 10
  .to_list()
check! [1, 2, 3, 1, 2, 3, 1, 2, 3, 1]
```

## each

```kototype
|Iterable, function: |Any| -> Any| -> Iterator
```

Takes an `Iterable` and a `Function`, and returns a new iterator that provides
the result of calling the function with each value in the iterable.

### Example

```koto
print! (2, 3, 4)
  .each |x| x * 2
  .to_list()
check! [4, 6, 8]
```

## enumerate

```kototype
|Iterable| -> Iterator
```

Returns an iterator that provides each value along with an associated index.

### Example

```koto
print! ('a', 'b', 'c').enumerate().to_list()
check! [(0, 'a'), (1, 'b'), (2, 'c')]
```

## find

```kototype
|Iterable, test: |Any| -> Bool| -> Any
```

Returns the first value in the iterable that passes the test function.

The function is called for each value in the iterator, and should return either
`true` if the value is a match, or `false` if it's not.

The first matching value will cause iteration to stop.

If no match is found then `null` is returned.

### Example

```koto
print! (10..20).find |x| x > 14 and x < 16
check! 15

print! (10..20).find |x| x > 100
check! null
```

## flatten

```kototype
|Iterable| -> Iterator
```

Returns the output of the input iterator, with any nested iterable values
flattened out.

Note that only one level of flattening is performed, so any double-nested
containers will still be present in the output.

### Example

```koto
print! [(2, 4), [6, 8, (10, 12)]]
  .flatten()
  .to_list()
check! [2, 4, 6, 8, (10, 12)]
```

### See Also

- [`iterator.find`](#find)

## fold

```kototype
|
  input: Iterable, 
  initial_value: Any, 
  accumulator: |accumulated: Any, next: Any| -> Any
| -> Any
```

Returns the result of 'folding' the iterator's values into an accumulator
function.

The function takes the accumulated value and the next iterator value,
and then returns the result of folding the value into the accumulator.

The first argument is an initial accumulated value that gets passed to the
function along with the first value from the iterator.

The result is then the final accumulated value.

This operation is also known in other languages as `reduce`, `accumulate`,
`inject`, `fold left`, along with other names.

### Example

```koto
print! ('a', 'b', 'c')
  .fold [], |result, x| 
    result.push x
    result.push '-'
check! ['a', '-', 'b', '-', 'c', '-']
```

### See Also

- [`iterator.product`](#product)
- [`iterator.sum`](#sum)

## generate

```kototype
|generator: || -> Any| -> Iterator
```

Provides an iterator that yields the result of repeatedly calling the `generator`
function. Note that this version of `generate` won't terminate and will iterate
endlessly.

```kototype
|n: Number, generator: || -> Any| -> Any
```

Provides an iterator that yields the result of calling the `generator`
function `n` times.

### Example

```koto
from iterator import generate

state = {x: 0}
f = || state.x += 1

print! generate(f)
  .take(5)
  .to_list()
check! [1, 2, 3, 4, 5]

print! generate(3, f).to_tuple()
check! (6, 7, 8)
```

### See Also

- [`iterator.repeat`](#repeat)

## intersperse

```kototype
|Iterable, value: Any| -> Iterator
```

Returns an iterator that yields a copy of the provided value between each
adjacent pair of output values.

```kototype
|Iterable, generator: || -> Any| -> Iterator
```

Returns an iterator that yields the result of calling the provided function
between each adjacent pair of output values.

### Example

```koto
print! ('a', 'b', 'c').intersperse('-').to_string()
check! a-b-c

separators = (1, 2, 3).iter()
print! ('a', 'b', 'c')
  .intersperse || separators.next().get()
  .to_tuple(),
check! ('a', 1, 'b', 2, 'c')
```

## iter

```kototype
|Iterable| -> Iterator
```

Returns an iterator that yields the provided iterable's values.

Iterable values will be automatically accepted by most iterator operations,
so it's usually not necessary to call `.iter()`, however it can be usefult
sometimes to make a standalone iterator for manual iteration.

Note that calling `.iter` with an `Iterator` will return the iterator without
modification. If a copy of the iterator is needed then see `koto.copy` and
`koto.deep_copy`.

### Example

```koto
i = (1..10).iter()
i.skip 5
print! i.next().get()
check! 6
```

### See Also

- [`koto.copy`](./koto.md#copy)
- [`koto.deep_copy`](./koto.md#deep_copy)

## keep

```kototype
|Iterable, test: |Any| -> Bool| -> Iterator
```

Returns an iterator that keeps only the values that pass a test function.

The function is called for each value in the iterator, and should return either
`true` if the value should be kept in the iterator output, or `false` if it
should be discarded.

### Example

```koto
print! 0..10
  .keep |x| x % 2 == 0
  .to_tuple()
check! (0, 2, 4, 6, 8)
```

## last

```kototype
|Iterable| -> Any
```

Consumes the iterator, returning the last yielded value.

### Example

```koto
print! (1..100).take(5).last()
check! 5

print! (0..0).last()
check! null
```

## max

```kototype
|Iterable| -> Any
```

Returns the maximum value found in the iterable.

```kototype
|Iterable, key: |Any| -> Any| -> Any
```

Returns the maximum value found in the iterable, based on first calling a 'key'
function with the value, and then using the resulting keys for the comparisons.

A `<` 'less than' comparison is performed between each value and the maximum
found so far, until all values in the iterator have been compared.

### Example

```koto
print! (8, -3, 99, -1).max()
check! 99
```

### See Also

- [`iterator.min`](#min)
- [`iterator.min_max`](#min-max)

## min

```kototype
|Iterable| -> Any
```

Returns the minimum value found in the iterable.

```kototype
|Iterable, key: |Any| -> Any| -> Any
```

Returns the minimum value found in the iterable, based on first calling a 'key'
function with the value, and then using the resulting keys for the comparisons.

A `<` 'less than' comparison is performed between each value and the minimum
found so far, until all values in the iterator have been compared.

### Example

```koto
print! (8, -3, 99, -1).min()
check! -3
```

### See Also

- [`iterator.max`](#max)
- [`iterator.min_max`](#min-max)

## min_max

```kototype
|Iterable| -> (Any, Any)
```

Returns the minimum and maximum values found in the iterable.

```kototype
|Iterable, key: |Any| -> Any| -> Any
```

Returns the minimum and maximum values found in the iterable, based on first
calling a 'key' function with the value, and then using the resulting keys for
the comparisons.

A `<` 'less than' comparison is performed between each value and both the
minimum and maximum found so far, until all values in the iterator have been
compared.

### Example

```koto
print! (8, -3, 99, -1).min_max()
check! (-3, 99)
```

### See Also

- [`iterator.max`](#max)
- [`iterator.min`](#min)

## next

```kototype
|Iterable| -> IteratorOutput
```

Returns the next value from the iterator wrapped in an 
[`IteratorOutput`](#iteratoroutput), 
or `null` if the iterator has been exhausted.

### Example

```koto
x = (1, null, 'x').iter()
print! x.next()
check! IteratorOutput(1)
print! x.next()
check! IteratorOutput(null)
print! x.next()
check! IteratorOutput(x)
print! x.next()
check! null

# Call .get() to access the value from an IteratorOutput
print! 'abc'.next().get()
check! a
```

### See Also

- [`iterator.next_back`](#next-back)

## next_back

```kototype
|Iterable| -> IteratorOutput
```

Returns the next value from the end of the iterator wrapped in an 
[`IteratorOutput`](#iteratoroutput), 
or `null` if the iterator has been exhausted.

This only works with iterators that have a defined end, so attempting to call
`next_back` on endless iterators like [`iterator.generate`](#generate) will 
result in an error.

### Example

```koto
x = (1..=5).iter()
print! x.next_back()
check! IteratorOutput(5)
print! x.next_back()
check! IteratorOutput(4)

# calls to next and next_back can be mixed together
print! x.next()
check! IteratorOutput(1)
print! x.next_back()
check! IteratorOutput(3)
print! x.next_back()
check! IteratorOutput(2)

# 1 has already been produced by the iterator, so it's now exhausted
print! x.next_back()
check! null
```

### See Also

- [`iterator.next`](#next)
- [`iterator.reversed`](#reversed)

## once

```kototype
|Any| -> Iterator
```

Returns an iterator that yields the given value a single time.

### Example

```koto
print! iterator.once(99)
  .chain('abc')
  .to_tuple()
check! (99, 'a', 'b', 'c')
```

### See Also

- [`iterator.generate`](#generate)
- [`iterator.repeat`](#repeat)

## peekable

```kototype
|Iterable| -> Peekable
```

Wraps the given iterable value in a peekable iterator.

### Peekable.peek

Returns the next value from the iterator without advancing it. 
The peeked value is cached until the iterator is advanced.

#### Example

```koto
x = 'abc'.peekable()
print! x.peek()
check! IteratorOutput(a)
print! x.peek()
check! IteratorOutput(a)
print! x.next()
check! IteratorOutput(a)
print! x.peek()
check! IteratorOutput(b)
print! x.next(), x.next()
check! (IteratorOutput(b), IteratorOutput(c))
print! x.peek()
check! null
```

#### See Also

- [`iterator.next`](#next)

### Peekable.peek_back

Returns the next value from the end of the iterator without advancing it. 
The peeked value is cached until the iterator is advanced.

#### Example

```koto
x = 'abc'.peekable()
print! x.peek_back()
check! IteratorOutput(c)
print! x.next_back()
check! IteratorOutput(c)
print! x.peek()
check! IteratorOutput(a)
print! x.peek_back()
check! IteratorOutput(b)
print! x.next_back(), x.next_back()
check! (IteratorOutput(b), IteratorOutput(a))
print! x.peek_back()
check! null
```

#### See Also

- [`iterator.next_back`](#next-back)

## position

```kototype
|Iterable, test: |Any| -> Bool| -> Any
```

Returns the position of the first value in the iterable that passes the test
function.

The function is called for each value in the iterator, and should return either
`true` if the value is a match, or `false` if it's not.

The first matching value will cause iteration to stop, and the number of
steps taken to reach the matched value is returned as the result.

If no match is found then `null` is returned.

### Example

```koto
print! (10..20).position |x| x == 15
check! 5

print! (10..20).position |x| x == 99
check! null
```

### See Also

- [`iterator.find`](#find)

## product

```kototype
|Iterable| -> Any
```

Returns the result of multiplying each value in the iterable together.

### Example

```koto
print! (2, 3, 4).product()
check! 24
```

### See also

- [`iterator.fold`](#fold)
- [`iterator.sum`](#sum)

## repeat

```kototype
|Any| -> Iterator
```
```kototype
|Any, repeats: Number| -> Iterator
```

Provides an iterator that repeats the provided value. 
A number of repeats can be optionally provided as the second argument.

### Example

```koto
print! iterator.repeat(42)
  .take(5)
  .to_list()
check! [42, 42, 42, 42, 42]

print! iterator.repeat('x', 3).to_tuple()
check! ('x', 'x', 'x')
```

### See Also

- [`iterator.generate`](#generate)
- [`iterator.once`](#once)

## reversed

```kototype
|Iterator| -> Iterator
```

Reverses the order of the iterator's output.

This only works with iterators that have a defined end, so attempting to reverse
endless iterators like [`iterator.generate`](#generate) will result in an error.

### Example

```koto
print! 'Héllö'.reversed().to_tuple()
check! ('ö', 'l', 'l', 'é', 'H')

print! (1..=10).reversed().skip(5).to_tuple()
check! (5, 4, 3, 2, 1)
```

## skip

```kototype
|Iterable, steps: Number| -> Iterator
```

Skips over a number of steps in the iterator.

### Example

```koto
print! (100..200).skip(50).next().get()
check! 150
```

### See also

- [`iterator.step`](#step)
- [`iterator.take`](#take)

## step

```kototype
|Iterable, step_size: Number| -> Iterator
```

Steps over the iterable's output by the provided step size.

### Example

```koto
print! (0..10).step(3).to_tuple()
check! (0, 3, 6, 9)

print! 'Héllö'.step(2).to_string()
check! Hlö
```

### See also

- [`iterator.skip`](#skip)

## sum

```kototype
|Iterable| -> Any
```

Returns the result of adding each value in the iterable together.

### Example

```koto
print! (2, 3, 4).sum()
check! 9
```

### See also

- [`iterator.fold`](#fold)
- [`iterator.product`](#product)

## take

```kototype
|Iterable, count: Number| -> Iterator
```

Provides an iterator that yields a number of values from the input before
finishing.

```kototype
|Iterable, test: |Any| -> Bool| -> Iterator
```

Provides an iterator that yields values from the input while they pass a
test function.

The test function should return `true` if the iterator should continue to yield
values, and `false` if the iterator should stop yielding values.


### Example

```koto
print! (100..200).take(3).to_tuple()
check! (100, 101, 102)

print! 'hey!'.take(|c| c != '!').to_string()
check! hey
```

### See also

- [`iterator.skip`](#skip)

## to_list

```kototype
|Iterable| -> List
```

Consumes all values coming from the iterator and places them in a list.

### Example

```koto
print! ('a', 42, (-1, -2)).to_list()
check! ['a', 42, (-1, -2)]
```

### See also

- [`iterator.to_map`](#to-map)
- [`iterator.to_string`](#to-string)
- [`iterator.to_tuple`](#to-tuple)

## to_map

```kototype
|Iterable| -> Map
```

Consumes all values coming from the iterator and places them in a map.

If a value is a tuple, then the first element in the tuple will be inserted as
the key for the map entry, and the second element will be inserted as the value.

If the value is anything other than a tuple, then it will be inserted as the map
key, with `null` as the entry's value.

### Example

```koto
print! ('a', 'b', 'c').to_map()
check! {a: null, b: null, c: null}

print! ('a', 'bbb', 'cc')
  .each |x| x, size x
  .to_map()
check! {a: 1, bbb: 3, cc: 2}
```

### See also

- [`iterator.to_list`](#to-list)
- [`iterator.to_string`](#to-string)
- [`iterator.to_tuple`](#to-tuple)

## to_string

```kototype
|Iterable| -> String
```

Consumes all values coming from the iterator and produces a string containing
the formatted values.

### Example

```koto
print! ('x', 'y', 'z').to_string()
check! xyz

print! (1, 2, 3).intersperse('-').to_string()
check! 1-2-3
```

### See also

- [`iterator.to_list`](#to-list)
- [`iterator.to_map`](#to-map)
- [`iterator.to_tuple`](#to-tuple)

## to_tuple

```kototype
|Iterable| -> Tuple
```

Consumes all values coming from the iterator and places them in a tuple.

### Example

```koto
print! ('a', 42, (-1, -2)).to_tuple()
check! ('a', 42, (-1, -2))
```

### See also

- [`iterator.to_list`](#to-list)
- [`iterator.to_map`](#to-map)
- [`iterator.to_string`](#to-string)

## windows

```kototype
|Iterable, size: Number| -> Iterator
```

Returns an iterator that splits up the input data into overlapping windows of
the specified `size`, where each window is provided as a Tuple.

If the input has fewer elements than the window size, then no windows will be
produced.

### Example

```koto
print! 1..=5
  .windows 3
  .to_list(),
check! [(1, 2, 3), (2, 3, 4), (3, 4, 5)]
```

## zip

```kototype
|first: Iterable, second: Iterable| -> Iterator
```

Combines the values in two iterables into an iterator that provides
corresponding pairs of values, one at a time from each input iterable.

### Example

```koto
print! (1, 2, 3)
  .zip ('a', 'b', 'c')
  .to_list()
check! [(1, 'a'), (2, 'b'), (3, 'c')]
```

## IteratorOutput

A wrapper for a single item of iterator output.

This exists to allow functions like [`iterator.next`](#next) to return `null` to
indicate that the iterator has been exhausted, 
while also allowing `null` to appear in the iterator's output.


## IteratorOutput.get

```kototype
|IteratorOutput| -> Any
```

Returns the wrapped iterator output value.

### Example

```koto
print! x = 'abc'.next()
check! IteratorOutput(a)
print! x.get()
check! a
```
