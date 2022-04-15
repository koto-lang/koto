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

## contains

```kototype
|Tuple, Value| -> Bool
```

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

```kototype
|Tuple| -> Value
```

Returns the first value in the tuple, or Null if the tuple is empty.

### Example

```koto
x = 99, -1, 42
x.first()
# 99

[].to_tuple().first()
# Null
```

## get

```kototype
|Tuple, Number| -> Value
```
```kototype
|Tuple, Number, Value| -> Value
```

Gets the Nth value in the tuple.
If the tuple doesn't contain a value at that position then the provided default
value is returned. If no default value is provided then Null is returned.

### Example

```koto
x = 99, -1, 42

x.get 1
# -1

x.get -1
# Null

x.get 5, "abc"
# abc
```

## last

```kototype
|Tuple| -> Value
```

Returns the last value in the tuple, or Null if the tuple is empty.

### Example

```koto
x = 99, -1, 42
x.last()
# 42

[].to_tuple().last()
# Null
```

## size

```kototype
|Tuple| -> Number
```

Returns the number of values contained in the tuple.

### Example

```koto
x = (10, 20, 30, 40, 50)
x.size()
# 5
```

## sort_copy

```kototype
|Tuple| -> Tuple
```

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

```kototype
|Tuple| -> List
```

Returns a copy of the tuple's data as a list.

### Example

```koto
(1, 2, 3).to_list()
# [1, 2, 3]
```
