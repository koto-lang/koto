# map

Maps in Koto are associative containers of keys mapped to values.

The order in which items are added to the map will be preserved.

## Creating a map

There are two ways to directly create a map in Koto:
map blocks, and inline maps.

### Block map syntax

Maps can be created with indented blocks, where each line contains an entry of
the form `Key: Value`.

```koto
x =
  hello: -1
  goodbye: 99

x.hello
# -1
x.goodbye
# 99
```

Nested Maps can be defined with additional indentation:

```koto
x =
  hello:
    world: 99
    everybody: 123
    to:
      you: -1
x.hello.world
# 99
x.hello.to.you
# 123
```

### Inline map syntax

Maps can also be created with curly braces, with comma-separated entries.

```koto
x = {hello: -1, goodbye: "abc"}
x.hello
# -1
x.goodbye
# abc
```

If only the key is provided for an entry, then a value matching the name of the
key is looked for and is then copied into the entry.

```koto
hello = "abc"
goodbye = 99
x = {hello, goodbye, tschüss: 123}
x.goodbye
# 99
```

## Keys

When creating a Map directly, the keys are defined as strings.
To use non-string values as keys, [`map.insert`](#insert) can be used.

```koto
x = {}
x.insert 0, "Hello"
x.insert true, "World"
"{}, {}!".format x.get(0), x.get(true)
# Hello, World!
```

## Instance functions

When a Function is used as a value in a Map, and if it uses the keyword `self`
as its first argument, then the runtime will pass the instance of the map that
contains the function as the `self` argument.

```koto
x =
  # Initialize an empty list
  data: []
  # Takes a value and adds it to the list
  add_to_data: |self, n| self.data.push n
  # Returns the sum of the list
  sum: |self| self.data.sum()

x.add_to_data 2
x.add_to_data 20
x.sum()
# 22
```

## Operators

The `+` operator can be used to merge two maps together.

```koto
x = {hello: 123}
y = {goodbye: 99}
x + y
# {hello: 123, goodbye: 99}
```

### Meta Maps and overloaded operations

Maps can be used to create value types with custom behaviour.

Keys with `@` prefixes go into the map's 'meta map',
which is checked when the map is encountered in operations.

```koto
make_x = |n|
  data: n
  # Overloading the addition operator
  @+: |self, other|
    # a new instance is made with the result of adding the two values together
    make_x self.data + other.data
  # Overloading the subtraction operator
  @-: |self, other|
    make_x self.data - other.data

x1 = make_x 10
x2 = make_x 20

(x1 + x2).data
# 30
(x1 - x2).data
# -10
```

All binary operators can be overloaded following this pattern.

Additionally, the following meta functions can customize object behaviour:

- `@negate`
  - Overloads the unary negation operator:
    - `@negate: |self| make_x -self.data`
- `@not`
  - Overloads the unary `not` operator:
    - `@not: |self| self.data == 0`
- `@index`
  - Overloads `[]` indexing:
    - `@index: |self, index| self.data + index`
- `@iterator`
  - Customizes how iterators will be made from the map. The function returns an
    iterator that will be used in place of the default iteration behaviour.
    - `@iterator: |self| 0..self.data`
- `@display`
  - Customizes how the map will be displayed when formatted as a string:
    - `@display: |self| "X: {}".format self.data`
- `@type`
  - Provides a String that's used when checking the map's type:
    - `@type: "X"`

#### Meta entries

`@meta` can be used as a prefix on a map entry to add it to the meta map.
The entry will be accessible on value lookups but won't show up in the regular
map data:

```koto
make_x = |n|
  data: n
  # Overloading the addition operator
  @meta get_data: |self| self.data

x = make_x 42
x.keys().to_list()
# ["data"]
x.get_data()
# 42
```

#### Tests

Tests are also stored in the meta map, see [test.md](test.md) for info.

# Reference

- [clear](#clear)
- [contains_key](#contains_key)
- [copy](#copy)
- [deep_copy](#deep_copy)
- [get](#get)
- [get_index](#get_index)
- [insert](#insert)
- [is_empty](#is_empty)
- [keys](#keys)
- [remove](#remove)
- [size](#size)
- [sort](#sort)
- [update](#update)
- [values](#values)

## clear

`|Map| -> Null`

Clears the map by removing all of its elements.

### Example

```koto
x = {x: -1, y: 42}
x.clear()
x
# {}
```

## contains_key

`|Map, Key| -> Bool`

Returns `true` if the map contains a value with the given key,
and `false` otherwise.

## copy

`|Map| -> Map`

Makes a unique copy of the map data.

Note that this only copies the first level of data, so nested containers
will share their data with their counterparts in the copy. To make a copy where
any nested containers are also unique, use [`map.deep_copy`](#deep_copy).

### Example

```koto
x = {foo: -1, bar: 99}
y = x
y.foo = 42
x.foo
# 42

z = x.copy()
z.bar = -1
x.bar # x.bar remains unmodified due to the
# 99
```

### See also

- [`map.deep_copy`](#deep_copy)

## deep_copy

`|Map| -> Map`

Makes a unique _deep_ copy of the map data.

This makes a unique copy of the map data, and then recursively makes deep copies
of any nested containers in the map.

If only the first level of data needs to be made unique, then use
[`map.copy`](#copy).

### Example

```koto
x = {foo: 42, bar: {baz: 99}}
y = m.deep_copy()
y.bar.baz = 123
x.bar.baz # a deep copy has been made, so x is unaffected by the change to y
# 99
```

### See also

- [`map.copy`](#copy)

## get

`|Map, Key| -> Value`
`|Map, Key, Value| -> Value`

Returns the value corresponding to the given key, or the provided default value
if the map doesn't contain the key.

If no default value is provided then Null is returned.

### Example

```koto
x = hello: -1
x.get "hello"
# -1

x.get "goodbye"
# Null

x.get "goodbye", "byeeee"
# "byeeee"

x.insert 99, "xyz"
x.get 99
# xyz
```

### See also

- [`map.get_index`](#get_index)

## get_index

`|Map, Number| -> Tuple`
`|Map, Number, Value| -> Tuple`

Returns the entry at the given index as a key/value tuple, or the provided
default value if the map doesn't contain an entry at that index.

If no default value is provided then Null is returned.

### Example

```koto
x = foo: -1, bar: -2
x.get_index 1
# (bar, -2)

x.get_index -99
# Null

x.get_index 99, "xyz"
# "xyz"
```

### See also

- [`map.get`](#get)

## insert

`|Map, Key| -> Value`

Inserts Null into the map with the given key.

`|Map, Key, Value| -> Value`

Inserts a value into the map with the given key.

If the key already existed in the map, then the old value is returned.
If the key didn't already exist, then Null is returned.

### Example

```koto
x = hello: -1
x.insert "hello", 99 # -1 already exists at `hello`, so it's returned here
# -1

x.hello # hello is now 99
# 99

x.insert "goodbye", 123 # No existing value at `goodbye`, so () is returned
# Null

x.goodbye
# 123
```

### See also

- [`map.remove`](#remove)
- [`map.update`](#update)

## is_empty

`|Map| -> Bool`

Returns `true` if the map contains no entries, otherwise `false`.

### Example

```koto
{}.is_empty()
# true

{hello: -1}.is_empty()
# false
```

### See also

- [`map.size`](#size)

## keys

`|Map| -> Iterator`

Returns an iterator that iterates in order over the map's keys.

### Example

```koto
m =
  hello: -1
  goodbye: 99

x = m.keys()

x.next()
# "hello"

x.next()
# "goodbye"

x.next()
# Null
```

### See also

- [`map.values`](#values)

## remove

`|Map, Key| -> Value`

Removes the entry that matches the given key.

If the entry existed then its value is returned, otherwise Null is returned.

### Example

```koto
x =
  hello: -1
  goodbye: 99

x.remove "hello"
# -1

x.remove "xyz"
# Null

x.remove "goodbye"
# 99

x.is_empty()
# true
```

### See also

- [`map.insert`](#insert)

## size

`|Map| -> Number`

Returns the number of entries contained in the map.

### Example

```koto
{}.size()
# 0

{"a": 0, "b": 1}.size()
# 2
```

### See also

- [`map.is_empty`](#is_empty)

## sort

`|Map| -> Null`

Sorts the map's entries by key.

`|Map, |Value, Value| -> Value| -> Null`

Sorts the map's entries, based on the output of calling a 'key' function for
each entry. The entry's key and value are passed into the function as separate
arguments.

The function result is cached, so it's only called once per entry.

### Example

```koto
x =
  hello: 123
  bye: -1
  tschüss: 99
x.sort() # Sorts the map by key
x
# {bye: -1, hello: 123, tschüss: 99}

x.sort |_, value| value # Sort the map by value
x
# {bye: -1, tschüss: 99, hello: 123}

x.sort |key, _| -key.size() # Sort the map by reversed key length
x
# {tschüss: 99, hello: 123, bye: -1}
```

## update

`|Map, Key, |Value| -> Value| -> Value`

Updates the value associated with a given key by calling a function with either
the existing value, or Null if there isn't a matching entry.

The result of the function will either replace an existing value, or if no value
existed then an entry will be inserted into the map with the given key and the
function's result.

The function result is then returned from `update`.

`|Map, Key, Value, |Value| -> Value| -> Value`

This variant of `update` takes a default value that is provided to the
function if a matching entry doesn't exist.

### Example

```koto
x =
  hello: -1
  goodbye: 99

x.update "hello", |n| n * 2
# -2
x.hello
# -2

x.update "tschüss", 10, |n| n * 10
# 100
x.tschüss
# 100
```

### See also

- [`map.insert`](#insert)

## values

`|Map| -> Iterator`

Returns an iterator that iterates in order over the map's values.

### Example

```koto
m =
  hello: -1
  goodbye: 99

x = m.values()

x.next()
# -1

x.next()
# 99

x.next()
# Null
```

### See also

- [`map.keys`](#keys)
