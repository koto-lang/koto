# tuple

Tuples in Koto are fixed contiguous arrays of values.

In contrast to Lists (which contains data that can modified),
once a tuple is created its data can't be modified.

Nested Lists and Maps in the Tuple can themselves be modified,
but the Tuple itself can be thought of as 'read-only'.

## Creating a Tuple

Tuples are created with comma-separated values:

```koto
x = "hello", -1, 99, [1, 2, 3]
# ("hello", -1, 99, [1, 2, 3])

x[2]
# 99

x[3]
# [1, 2, 3]
```

Parentheses are used when necessary for disambiguation:

```koto
x, y = (1, 2, 3), (4, 5, 6)
# ((1, 2, 3), (4, 5, 6))

x[1], y[2]
# (2, 6)
```

# Reference

- [contains](#contains)
- [deep_copy](#deep_copy)
- [first](#first)
- [get](#get)
- [iter](#iter)
- [last](#last)
- [size](#size)
- [sort_copy](#sort_copy)
- [to_list](#to_list)

## contains

`|Tuple, Value| -> Bool`

Returns `true` if the tuple contains a value that matches the input value.

Matching is performed with the `==` equality operator.

### Example

```koto
(1, "hello", [99. -1]).contains "hello"
# true

("goodbye", 123).contains "hello"
# false
```

## deep_copy

## first

`|Tuple| -> Value`

Returns the first value in the tuple, or `()` if the tuple is empty.

### Example

```koto
x = 99, -1, 42
x.first()
# 99

[].to_tuple().first()
# ()
```

## get

`|Tuple, Number| -> Value`

Gets the Nth value in the tuple, or `()` if the tuple doesn't contain a value at
that position.

### Example

```koto
(99, -1, 42).get 1
# -1

(99, -1, 42).get 5
# ()
```

## iter

`|Tuple| -> Iterator`

Returns an iterator that iterates over the tuple's values.

Tuples are iterable, so it's not necessary to call `.iter()` to get access to
iterator operations, but it can be useful sometimes to make a standalone
iterator for manual iteration.

### Example

```koto
x = (2, 3, 4).iter()
x.skip(1)
x.next()
# 3
```

## last

`|Tuple| -> Value`

Returns the last value in the tuple, or `()` if the tuple is empty.

### Example

```koto
x = 99, -1, 42
x.last()
# 42

[].to_tuple().last()
# ()
```

## size

`|Tuple| -> Number`

Returns the number of values contained in the tuple.

### Example

```koto
x = (10, 20, 30, 40, 50)
x.size()
# 5
```

## sort_copy

`|Tuple| -> Tuple`

Returns a sorted copy of the tuple.

### Example

```koto
x = (1, -1, 99, 42)
y = x.sort_copy()
y
# (-1, 1, 42, 99)

x # x remains untouched
# (1, -1, 99, 42)
```

## to_list

`|Tuple| -> List`

Returns a copy of the tuple's data as a list.

### Example

```koto
(1, 2, 3).to_list()
# [1, 2, 3]
```
