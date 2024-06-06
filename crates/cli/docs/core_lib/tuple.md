# tuple

## contains

```kototype
|Tuple, value: Any| -> Bool
```

Returns `true` if the tuple contains a value that matches the input value.

Matching is performed with the `==` equality operator.

### Example

```koto
print! (1, "hello", [99, -1]).contains "hello"
check! true

print! ("goodbye", 123).contains "hello"
check! false
```

## first

```kototype
|Tuple| -> Any
```

Returns the first value in the tuple, or Null if the tuple is empty.

### Example

```koto
x = 99, -1, 42
print! x.first()
check! 99

print! (,).first()
check! null
```

## get

```kototype
|Tuple, index: Number| -> Any
```
```kototype
|Tuple, index: Number, default: Any| -> Any
```

Gets the Nth value in the tuple.
If the tuple doesn't contain a value at that position then the provided `default`
value is returned. If no default value is provided then Null is returned.

### Example

```koto
x = 99, -1, 42

print! x.get 1
check! -1

print! x.get -1
check! null

print! x.get 5, "abc"
check! abc
```

## last

```kototype
|Tuple| -> Any
```

Returns the last value in the tuple, or Null if the tuple is empty.

### Example

```koto
x = 99, -1, 42
print! x.last()
check! 42

print! (,).last()
check! null
```

## sort_copy

```kototype
|Tuple| -> Tuple
```

Returns a sorted copy of the tuple.

```kototype
|List, key: |Any| -> Any| -> List
```

Returns a sorted copy of the tuple, based on the output of calling a `key` 
function for each of the tuple's elements. 

The key function's result is cached, so it's only called once per value.

### Example

```koto
x = (1, -1, 99, 42)
y = x.sort_copy()
print! y
check! (-1, 1, 42, 99)

print! x # x remains untouched
check! (1, -1, 99, 42)

# Sort in reverse order by using a key function
print! x.sort_copy |n| -n
check! (99, 42, 1, -1)
```

## to_list

```kototype
|Tuple| -> List
```

Returns a copy of the tuple's data as a list.

### Example

```koto
print! (1, 2, 3).to_list()
check! [1, 2, 3]
```
