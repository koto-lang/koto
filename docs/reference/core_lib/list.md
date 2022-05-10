# list

Lists in Koto are dynamically sized contiguous arrays of values.

Like other containers in Koto, a list's data is shared between all instances of
the list.

## Example

```koto
x = [1, 2, "hello"]
x[1] = 99
print! x
check! [1, 99, "hello"]

y = x
y[0] = "abc" # x and y share the same internal list data
print! x
check! ["abc", 99, "hello"]

z = x.copy()
z[1] = -1 # z is a copy of x, so has unique internal data
print! x # x remains unchanged after the modificaton of z
check! ["abc", 99, "hello"]
```

# Reference

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
|List, Value| -> Bool
```

Returns `true` if the list contains a value that matches the input value.

Matching is performed with the `==` equality operator.

### Example

```koto
print! [1, "hello", (99, -1)].contains "hello"
check! true
```

## copy

```kototype
|List| -> List
```

Makes a unique copy of the list data.

Note that this only copies the first level of data, so nested containers
will share their data with their counterparts in the copy. To make a copy where
any nested containers are also unique, use [`list.deep_copy`](#deep-copy).

### Example

```koto
x = [1, 2, "hello"]
y = x
y[0] = "abc" # x and y share the same internal list data
print! x
check! ["abc", 2, "hello"]

z = x.copy()
z[1] = -1 # z is a copy of x, so has unique internal data
print! x # x remains unchanged after the modificaton of z
check! ["abc", 2, "hello"]
```

### See also

- [`list.deep_copy`](#deep-copy)

## deep_copy

```kototype
|List| -> List
```

Makes a unique _deep_ copy of the list data.

This makes a unique copy of the list data, and then recursively makes deep
copies of any nested containers in the list.

If only the first level of data needs to be made unique, then use
[`list.copy`](#copy).

### Example

```koto
x = [[1, 2], [3, [4, 5]]]
y = x.deep_copy()
y[1][1] = 99
print! x # a deep copy has been made, so x is unaffected by the assignment to y
check! [[1, 2], [3, [4, 5]]]
```

### See also

- [`list.copy`](#copy)

## fill

```kototype
|List, Value| -> List
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
|List| -> Value
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
|List, Number| -> Value
```
```kototype
|List, Number, Value| -> Value
```

Gets the Nth value in the list.
If the list doesn't contain a value at that position then the provided default
value is returned. If no default value is provided then Null is returned.

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
|List, Number, Value| -> List
```

Inserts the value into the Nth position in the list, and returns the list.

An error is thrown if the position is negative or greater than the size of the
list.

### Example

```koto
x = [99, -1, 42]
print! x.insert 2, "hello"
check! [99, -1, "hello", 42]
print! x
check! [99, -1, "hello", 42]
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
|List| -> Value
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
|List| -> Value
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
|List, Value| -> Value
```

Adds the value to the end of the list, and returns the list.

### Example

```koto
x = [99, -1]
print! x.push "hello"
check! [99, -1, "hello"]
print! x
check! [99, -1, "hello"]
```

### See also

- [`list.pop`](#pop)

## remove

```kototype
|List, Number| -> Value
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
|List, Number| -> List
```
```kototype
|List, Number, Value| -> List
```

Grows or shrinks the list to the specified size, and returns the list.
If the new size is larger, then copies of the provided value (or Null if no
value is provided) are used to fill the new space.

### Example

```koto
x = [1, 2]
print! x.resize 4, "x"
check! [1, 2, "x", "x"]

print! x.resize 3
check! [1, 2, "x"]

print! x.resize 4
check! [1, 2, "x", null]
```

## resize_with

```kototype
|List, Number, || -> Value| -> List
```

Grows or shrinks the list to the specified size, and returns the list.
If the new size is larger, then the provided function will be called repeatedly
to fill the remaining space, with the result of the function being added to the
end of the list.

### Example

```koto
new_entries = (5, 6, 7, 8).iter()
x = [1, 2]
print! x.resize_with 4, || new_entries.next()
check! [1, 2, 5, 6]

print! x.resize_with 2, || new_entries.next()
check! [1, 2]
```

## retain

```kototype
|List, Value| -> List
```

Retains matching values in the list (discarding values that don't match), and
returns the list.

If the test value is a function, then the function will be called with each of
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
x = ["hello", -1, 99, "world"]
print! x.reverse()
check! ["world", 99, -1, "hello"]
print! x
check! ["world", 99, -1, "hello"]
```

## size

```kototype
|List| -> Number
```

Returns the number of values contained in the list.

### Example

```koto
x = (1..=100).to_list()
print! x.size()
check! 100

print! [].size()
check! 0
```

## sort

```kototype
|List| -> List
```

Sorts the list in place, and returns the list.

```kototype
|List, |Value| -> Value| -> List
```

Sorts the list in place, based on the output of calling a 'key' function for
each value, and returns the list. The function result is cached, so it's only
called once per value.

### Example

```koto
x = [1, -1, 99, 42]
print! x.sort()
check! [-1, 1, 42, 99]
print! x
check! [-1, 1, 42, 99]

x = ["bb", "ccc", "a"]
print! x.sort string.size
check! ["a", "bb", "ccc"]
print! x
check! ["a", "bb", "ccc"]

x = [2, 1, 3]
print! x.sort |n| -n
check! [3, 2, 1]
print! x
check! [3, 2, 1]
```

## swap

```kototype
|List, List| -> Null
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
|List, |Value| -> Value| -> List
```

Transforms the list data by replacing each value with the result of calling the
provided function, and then returns the list.

### Example

```koto
x = ["aaa", "bb", "c"]
print! x.transform string.size
check! [3, 2, 1]
print! x
check! [3, 2, 1]

print! x.transform |n| "{}".format n
check! ["3", "2", "1"]
print! x
check! ["3", "2", "1"]
```

## with_size

```kototype
|Number, Value| -> List
```

Returns a list containing `N` copies of a value.

### Example

```koto
print! list.with_size 5, "!"
check! ["!", "!", "!", "!", "!"]
```
