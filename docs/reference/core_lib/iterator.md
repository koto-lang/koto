# iterator

Iterators in Koto provide access to sequences of data, yielding values via
`.next()`, until the end of the sequence is reached and the empty value `()`
is returned.

## Iterable Values

Values that can produce iterable sequences are referred to as `Iterable`.

`Iterables` include:
- Iterators (naturally!)
- Lists
- Maps
  - The map's key/value pairs are provided as a Tuple.
- Ranges
- Strings
- Tuples

The contents of the `iterator` module are made available to all `Iterable`s.

### Example

```koto
# Starting with a List
[1, 2, 3]
  # Calling iterator.each with the List as the implicit first argument
  .each |x| x * 2
  # Calling iterator.to_list with the Iterator resulting from iterator.each
  .to_list()
# [2, 4, 6]
```

## Loops

`for`, `while`, and `until` loops take any `Iterable` and then provide its
output values for each iteration of the loop.

```koto
for x in (2, 3, 4).each |n| n * 2
  io.print "-> {}", x
# -> 4
# -> 6
# -> 8
```

# Reference

- [all](#all)
- [any](#any)
- [chain](#chain)
- [consume](#consume)
- [copy](#copy)
- [count](#count)
- [cycle](#cycle)
- [each](#each)
- [enumerate](#enumerate)
- [find](#find)
- [fold](#fold)
- [intersperse](#intersperse)
- [iter](#iter)
- [keep](#keep)
- [last](#last)
- [max](#max)
- [min](#min)
- [min_max](#min_max)
- [next](#next)
- [position](#position)
- [product](#product)
- [skip](#skip)
- [sum](#sum)
- [take](#take)
- [to_list](#to_list)
- [to_map](#to_map)
- [to_num2](#to_num2)
- [to_num4](#to_num4)
- [to_string](#to_string)
- [to_tuple](#to_tuple)
- [zip](#zip)

## all

`|Iterable, Function(|Value| -> Bool)| -> Bool`

Checks the Iterable's values against a test Function.
The Function should return `true` or `false`, and then `all` returns `true`
if all values pass the test.

`all` stops running as soon as it finds a failing test, and then `false` is
returned.

### Example

```koto
(1..9).all |x| x > 0
# true

("", "", "foo").all string.is_empty
# false

[10, 20, 30]
  .each |x| x / 10
  .all |x| x < 10
# true
```

## any

`|Iterable, |Value| -> Bool| -> Bool`

Checks the Iterable's values against a test Function.
The Function should return `true` or `false`, and then `any` returns `true`
if any of the values pass the test.

`any` stops running as soon as it finds a passing test.

### Example

```koto
(1..9).any |x| x == 5
# true

("", "", "foo").any string.is_empty
# true

[10, 20, 30]
  .each |x| x / 10
  .any |x| x == 2
# true
```

## chain

`|Iterable, Iterable| -> Iterator`

`chain` returns an iterator that iterates over the output of the first iterator,
followed by the output of the second iterator.

### Example

```koto
[1, 2].chain([3, 4, 5]).to_tuple()
# (1, 2, 3, 4, 5)
```

## consume

`|Iterable| -> ()`

Consumes the output of the iterator.

`|Iterable, Function| -> ()`

Consumes the output of the iterator, calling the provided function with each
iterator output value.

### Example

```koto
result = []
(1..=10)
  .keep |n| n % 2 == 0
  .each |n| result.push n
  .consume()
result
# [2, 4, 6, 8, 10]

# Alternatively, calling consume with a function is equivalent to having an
# `each` / `consume` chain
result = []
(1..=10)
  .keep |n| n % 2 == 1
  .consume |n| result.push n
result
# [1, 3, 5, 7, 9]
```

## copy

`|Iterator| -> Iterator`

Returns an iterator that shares the same iterable data, but with a unique
iteration position (which is part of an iterator's shared state by default).

### Note

If the iterator is a generator then some effort will be made to make the
generator's copy provide the same output as the original, however this isn't
guaranteeed to be successful. Specifically, the value stack of the copied VM
will be scanned for iterators, and each iterator will have a copy made.
Iterators that may be used in other ways by the generator (e.g. stored in
containers or function captures) won't be copied and will still have shared
state.

### Example

```koto
x = (1..=10).iter()
y = x # y shares the same iteration position as x.
z = x.copy() # z shares the same iteration data (the range 1..=10),
             # but has a unique iteration position.

x.next()
# 1
x.next()
# 2
y.next() # y shares x's iteration position.
# 3
z.next() # z's iteration hasn't been impacted by the advancing of x and y.
# 1
```

## count

`|Iterable| -> Number`

Counts the number of items yielded from the iterator.

### Example

```koto
(5..=15).count()
# 10

(0..100)
  .keep |x| x % 2 == 0
  .count()
# 50
```

## cycle

`|Iterable| -> Iterator`

Takes an Iterable and returns a new iterator that endlessly repeats the output
of the iterable.

### Example

```koto
(1, 2, 3)
  .cycle()
  .take(10)
  .to_list()
# [1, 2, 3, 1, 2, 3, 1, 2, 3, 1]
```

## each

`|Iterable, |Value| -> Value| -> Iterator`

Takes an Iterable and a Function, and returns a new iterator that provides the
result of calling the function with each value in the iterable.

### Example

```koto
(2, 3, 4)
  .each |x| x * 2
  .to_list()
# [4, 6, 8]
```

## enumerate

`|Iterable| -> Iterator`

Returns an iterator that provides each value along with an associated index.

### Example

```koto
("a", "b", "c").enumerate().to_list()
# [(0, "a"), (1, "b"), (2, "c")]
```

## find

`|Iterable, |Value| -> Bool| -> Value`

Returns the first value in the iterable that passes the test function.

The function is called for each value in the iterator, and returns either `true`
if the value is a match, or `false` if it's not.

The first matching value will cause iteration to stop.

If no match is found then `()` is returned.

### Example

```koto
(10..20).find |x| x > 14 and x < 16
# 15

(10..20).find |x| x > 100
# ()
```

### See Also

- [`iterator.find`](#find)

## fold

`|Iterable, Value, |Value, Value| -> Value| -> Value`

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
("a", "b", "c").fold "", |result, x| result += x + "-"
# a-b-c-
```

### See Also

- [`iterator.product`](#product)
- [`iterator.sum`](#sum)

## intersperse

`|Iterable, Value| -> Iterator`

Returns an iterator that yields a copy of the provided value between each
adjacent pair of output values.

`|Iterable, || -> Value| -> Iterator`

Returns an iterator that yields the result of calling the provided function
between each adjacent pair of output values.

### Example

```koto
("a", "b", "c").intersperse("-").to_string()
# "a-b-c"

separators = (1, 2, 3).iter()
("a", "b", "c")
  .intersperse || separators.next()
  .to_tuple(),
# ("a", 1, "b", 2, "c")
```

## iter

`|Iterable| -> Iterator`

Returns an iterator that yields the provided iterable's values.

Iterable values will be automatically accepted by most iterator operations,
so it's usually not necessary to call `.iter()`, however it can be usefult
sometimes to make a standalone iterator for manual iteration.

Note that calling `.iter` with an `Iterator` will return the iterator without
modification. If a copy of the iterator is needed then use `.copy()`.

### Example

```koto
i = (1..10).iter()
i.skip 5
i.next()
# 6
```

### See Also

- [`iterator.copy`](#copy)

## keep

`|Iterable, |Value| -> Bool| -> Iterator`

Returns an iterator that keeps only the values that pass a test function.

The function is called for each value in the iterator, and returns either `true`
if the value should be kept in the iterator output, or `false` if it should be
discarded.

### Example

```koto
(0..10).keep(|x| x % 2 == 0).to_tuple()
# (0, 2, 4, 6, 8)
```

## last

`|Iterable| -> Value`

Consumes the iterator, returning the last yielded value.

### Example

```koto
(1..100).take(5).last()
# 5

(0..0).last()
# ()
```

## max

`|Iterable| -> Value`

Returns the maximum value found in the iterable.

`|Iterable, |Value| -> Value| -> Value`

Returns the maximum value found in the iterable, based on first calling a 'key'
function with the value, and then using the resulting keys for the comparisons.

A `<` 'less than' comparison is performed between each value and the maximum
found so far, until all values in the iterator have been compared.

### Example

```koto
(8, -3, 99, -1).max()
# 99
```

### See Also

- [`iterator.min`](#min)
- [`iterator.min_max`](#min_max)

## min

`|Iterable| -> Value`

Returns the minimum value found in the iterable.

`|Iterable, |Value| -> Value| -> Value`

Returns the minimum value found in the iterable, based on first calling a 'key'
function with the value, and then using the resulting keys for the comparisons.

A `<` 'less than' comparison is performed between each value and the minimum
found so far, until all values in the iterator have been compared.

### Example

```koto
(8, -3, 99, -1).min()
# -3
```

### See Also

- [`iterator.max`](#max)
- [`iterator.min_max`](#min_max)

## min_max

`|Iterable| -> (Value, Value)`

Returns the minimum and maximum values found in the iterable.

`|Iterable, |Value| -> Value| -> Value`

Returns the minimum and maximum values found in the iterable, based on first
calling a 'key' function with the value, and then using the resulting keys for
the comparisons.

A `<` 'less than' comparison is performed between each value and both the
minimum and maximum found so far, until all values in the iterator have been
compared.

### Example

```koto
(8, -3, 99, -1).min_max()
# (-3, 99)
```

### See Also

- [`iterator.max`](#max)
- [`iterator.min`](#min)

## next

`|Iterator| -> Value`

Returns the next value from the iterator.

### Example

```koto
x = (1, 2).iter()
x.next()
# 1
x.next()
# 2
x.next()
# ()
```

## position

`|Iterable, |Value| -> Bool| -> Value`

Returns the position of the first value in the iterable that passes the test
function.

The function is called for each value in the iterator, and returns either `true`
if the value is a match, or `false` if it's not.

The first matching value will cause iteration to stop, and the number of
steps taken to reach the matched value is returned as the result.

If no match is found then `()` is returned.

### Example

```koto
(10..20).position |x| x == 15
# 5

(10..20).position |x| x == 99
# ()
```

### See Also

- [`iterator.find`](#find)

## product

`|Iterable| -> Value`

Returns the result of multiplying each value in the iterable together.

### Example

```koto
(2, 3, 4).product()
# 24
```

### See also

- [`iterator.fold`](#fold)
- [`iterator.sum`](#sum)

## skip

`|Iterable, Number| -> Iterator`

Skips over a number of steps in the iterator.

### Example

```koto
(100..200).skip(50).next()
# 150
```

### See also

- [`iterator.take`](#take)

## sum

`|Iterable| -> Value`

Returns the result of adding each value in the iterable together.

### Example

```koto
(2, 3, 4).sum()
# 9
```

### See also

- [`iterator.fold`](#fold)
- [`iterator.product`](#product)

## take

`|Iterable, Number| -> Iterator`

Provides an iterator that consumes a number of values from the input before
finishing.

### Example

```koto
(100..200).take(3).to_tuple()
# (100, 101, 102)
```

### See also

- [`iterator.skip`](#skip)

## to_list

`|Iterable| -> List`

Consumes all values coming from the iterator and places them in a list.

### Example

```koto
("a", 42, (-1, -2)).to_list()
# ["a", 42, (-1, -2)]
```

### See also

- [`iterator.to_map`](#to_map)
- [`iterator.to_string`](#to_string)
- [`iterator.to_tuple`](#to_tuple)

## to_map

`|Iterable| -> Map`

Consumes all values coming from the iterator and places them in a map.

If a value is a tuple, then the first element in the tuple will be inserted as
the key for the map entry, and the second element will be inserted as the value.

If the value is anything other than a tuple, then it will be inserted as the map
key, with `()` as the entry's value.

### Example

```koto
("a", "b", "c").to_map()
# {"a": (), "b": (), "c": ()}

("a", "bbb", "cc")
  .each |x| x, x.size()
  .to_map()
# {"a": 1, "bbb": 3, "cc": 2}
```

### See also

- [`iterator.to_list`](#to_list)
- [`iterator.to_string`](#to_string)
- [`iterator.to_tuple`](#to_tuple)

## to_num2

`|Iterable| -> Num2`

Consumes up to 2 values from the iterator and places them in a Num2.

### Example

```koto
[1].to_num2()
# num2(1, 0)
(1..10).keep(|n| n % 2 == 0).to_num2()
# num2(2, 4)
```

### See also

- [`iterator.to_num4`](#to_num4)

## to_num4

`|Iterable| -> Num4`

Consumes up to 4 values from the iterator and places them in a Num2.

### Example

```koto
[1].to_num4()
# num2(1, 0, 0, 0)
(1..10).keep(|n| n % 2 == 0).to_num4()
# num2(2, 4, 6, 8)
```

### See also

- [`iterator.to_num4`](#to_num4)

## to_string

`|Iterable| -> String`

Consumes all values coming from the iterator and produces a string containing
the formatted values.

### Example

```koto
("x", "y", "z").to_string()
# "xyz"

(1, 2, 3).intersperse("-").to_string()
# "1-2-3"
```

### See also

- [`iterator.to_list`](#to_list)
- [`iterator.to_map`](#to_map)
- [`iterator.to_tuple`](#to_tuple)

## to_tuple

`|Iterable| -> Tuple`

Consumes all values coming from the iterator and places them in a tuple.

### Example

```koto
("a", 42, (-1, -2)).to_list()
# ["a", 42, (-1, -2)]
```

### See also

- [`iterator.to_list`](#to_list)
- [`iterator.to_map`](#to_map)
- [`iterator.to_string`](#to_string)

## zip

`|Iterable, Iterable| -> Iterator`

Combines the values in two iterables into an iterator that provides
corresponding pairs of values, one at a time from each input iterable.

### Example

```koto
(1, 2, 3).zip(("a", "b", "c")).to_list()
# [(1, "a"), (2, "b"), (3, "c")]
```
