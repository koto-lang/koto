# list

## clear

```kototype
|List| -> List
```

Clears the list by removing all of its elements, and returns the cleared list.

### Example

```koto
x = [1, 2, 3]
print! x.clear()
check! []
```

## contains

```kototype
|List, value: Any| -> Bool
```

Returns `true` if the list contains an element that matches the input `value`.

Matching is performed with the `==` equality operator.

### Example

```koto
print! [1, 'hello', (99, -1)].contains 'hello'
check! true
```

## extend

```kototype
|List, new_elements: Iterable| -> List
```

Extends the list with the output of the iterator, and returns the list.

### Example

```koto
x = [1, 2, 3]
print! x.extend 'abc'
check! [1, 2, 3, 'a', 'b', 'c']
print! x.last()
check! c
print! x.extend [10, 20, 30]
check! [1, 2, 3, 'a', 'b', 'c', 10, 20, 30]
print! x.last()
check! 30
```

### See also

- [`list.push`](#push)

## fill

```kototype
|List, value: Any| -> List
```

Fills the list with copies of the provided value, and returns the list.

### Example

```koto
x = [1, 2, 3]
print! x.fill 99
check! [99, 99, 99]
print! x
check! [99, 99, 99]
```

## first

```kototype
|List| -> Any
```

Returns the first value in the list, or Null if the list is empty.

### Example

```koto
print! [99, -1, 42].first()
check! 99

print! [].first()
check! null
```

### See also

- [`list.get`](#get)
- [`list.last`](#last)

## get

```kototype
|List, index: Number| -> Any
```
```kototype
|List, index: Number, default: Any| -> Any
```

Gets the element at the given `index` in the list.

If the list doesn't contain a value at that position then the provided `default`
value is returned. If no default value is provided then `null` is returned.

### Example

```koto
x = [99, -1, 42]

print! x.get 1
check! -1

print! x.get -1
check! null

print! x.get 5, 123
check! 123
```

### See also

- [`list.first`](#first)
- [`list.last`](#last)

## insert

```kototype
|List, position: Number, value: Any| -> List
```

Inserts the value into the list at the given index position, 
and returns the list.

Elements in the list at or after the given position will be shifted to make
space for the new value.

An error is thrown if `position` is negative or greater than the size of the
list.

### Example

```koto
x = [99, -1, 42]
print! x.insert 2, 'hello'
check! [99, -1, 'hello', 42]
print! x
check! [99, -1, 'hello', 42]
```

### See also

- [`list.remove`](#remove)

## is_empty

```kototype
|List| -> Bool
```

Returns `true` if the list has a size of zero, and `false` otherwise.

### Example

```koto
print! [].is_empty()
check! true

print! [1, 2, 3].is_empty()
check! false
```

## last

```kototype
|List| -> Any
```

Returns the last value in the list, or Null if the list is empty.

### Example

```koto
print! [99, -1, 42].last()
check! 42

print! [].last()
check! null
```

### See also

- [`list.first`](#first)
- [`list.get`](#get)

## pop

```kototype
|List| -> Any
```

Removes the last value from the list and returns it.

If the list is empty then Null is returned.

### Example

```koto
x = [99, -1, 42]
print! x.pop()
check! 42

print! x
check! [99, -1]

print! [].pop()
check! null
```

### See also

- [`list.push`](#push)

## push

```kototype
|List, value: Any| -> Any
```

Adds the `value` to the end of the list, and returns the list.

### Example

```koto
x = [99, -1]
print! x.push 'hello'
check! [99, -1, 'hello']
print! x
check! [99, -1, 'hello']
```

### See also

- [`list.pop`](#pop)

## remove

```kototype
|List, position: Number| -> Any
```

Removes the value at the given position from the list and returns it.

Throws an error if the position isn't a valid index in the list.

### Example

```koto
[99, -1, 42].remove 1
# [99, 42]
```

### See also

- [`list.insert`](#insert)

## resize

```kototype
|List, new_size: Number| -> List
```
```kototype
|List, new_size: Number, default: Any| -> List
```

Grows or shrinks the list to the specified size, and returns the list.
If the new size is larger, then copies of the `default` value (or `null` if no
value is provided) are used to fill the new space.

### Example

```koto
x = [1, 2]
print! x.resize 4, 'x'
check! [1, 2, 'x', 'x']

print! x.resize 3
check! [1, 2, 'x']

print! x.resize 4
check! [1, 2, 'x', null]
```

## resize_with

```kototype
|List, new_size: Number, generator: || -> Any| -> List
```

Grows or shrinks the list to the specified size, and returns the list.
If the new size is larger, then the provided function will be called repeatedly
to fill the remaining space, with the result of the function being added to the
end of the list.

### Example

```koto
new_entries = (5, 6, 7, 8).iter()
x = [1, 2]
print! x.resize_with 4, || new_entries.next().get()
check! [1, 2, 5, 6]

print! x.resize_with 2, || new_entries.next().get()
check! [1, 2]
```

## retain

```kototype
|List, test: Any| -> List
```

Retains matching values in the list (discarding values that don't match), and
returns the list.

If `test` is a function, then the function will be called with each of
the list's values, and if the function returns `true` then the value will be
retained, otherwise if the function returns `false` then the value will be
discarded.

If the test value is not a function, then the list's values will be compared
using the `==` equality operator, and then retained if they match.

### Example

```koto
x = (1..10).to_list()
print! x.retain |n| n < 5
check! [1, 2, 3, 4]
print! x
check! [1, 2, 3, 4]

x = [1, 3, 8, 3, 9, -1]
print! x.retain 3
check! [3, 3]
print! x
check! [3, 3]
```

## reverse

```kototype
|List| -> List
```

Reverses the order of the list's contents, and returns the list.

### Example

```koto
x = ['hello', -1, 99, 'world']
print! x.reverse()
check! ['world', 99, -1, 'hello']
print! x
check! ['world', 99, -1, 'hello']
```

## sort

```kototype
|List| -> List
```

Sorts the list in place, and returns the list.

```kototype
|List, key: |Any| -> Any| -> List
```

Sorts the list in place, based on the output of calling a `key` function for
each of the list's elements, and returns the list. 

The key function's result is cached, so it's only called once per element.

### Example

```koto
x = [1, -1, 99, 42]
print! x.sort()
check! [-1, 1, 42, 99]
print! x
check! [-1, 1, 42, 99]

x = ['bb', 'ccc', 'a']
print! x.sort size
check! ['a', 'bb', 'ccc']
print! x
check! ['a', 'bb', 'ccc']

x = [2, 1, 3]
# Sort in reverse order by using a key function
print! x.sort |n| -n
check! [3, 2, 1]
print! x
check! [3, 2, 1]
```

## swap

```kototype
|first: List, second: List| -> Null
```

Swaps the contents of the two input lists.

### Example

```koto
x = [1, 2, 3]
y = [7, 8, 9]
x.swap y

print! x
check! [7, 8, 9]

print! y
check! [1, 2, 3]
```

## to_tuple

```kototype
|List| -> Tuple
```

Returns a copy of the list data as a tuple.

### Example

```koto
print! [1, 2, 3].to_tuple()
check! (1, 2, 3)
```

## transform

```kototype
|List, transformer: |Any| -> Any| -> List
```

Transforms the list data in place by replacing each value with the result of 
calling the provided `transformer` function, and then returns the list.

### Example

```koto
x = ['aaa', 'bb', 'c']
print! x.transform size
check! [3, 2, 1]
print! x
check! [3, 2, 1]

print! x.transform |n| '{n}!'
check! ['3!', '2!', '1!']
print! x
check! ['3!', '2!', '1!']
```
