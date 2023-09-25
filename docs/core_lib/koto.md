# koto

A collection of utilities for working with the Koto runtime.

## args

```kototype
Tuple
```

Provides access to the arguments that were passed into the script when running
the `koto` CLI application.

If no arguments were provided then the list is empty.

### Example

```koto
# Assuming that the script was run with `koto script.koto -- 1 2 "hello"`
koto.args.size()
# 3
koto.args.first()
# 1
koto.args.last()
# hello
```

## copy

```kototype
|Value| -> Value
```

Makes a copy of the value. 

### Shared mutable data

For values that have shared mutable data (i.e., `List`, `Map`), unique copies of
the data will be made. Note that this only applies to the first level of data,
so nested containers will still share their data with their counterparts in the
original data. To make a copy where any nested containers are also unique, 
use [`koto.deep_copy`](#deep-copy).

### Iterator copies

Copied iterators share the same underlying data as the original, but have a
unique iteration position, which is part of an iterator's shared state by
default.

If the iterator is a generator, some effort will be made to make the generator's
copy produce the same output as the original. However, this isn't guaranteed to
be successful. Specifically, the value stack of the copied virtual machine will
be scanned for iterators, and each iterator will have a copy made. Iterators
that may be used in other ways by the generator (such as being stored in
containers or function captures) won't be copied and will still have shared
state.


### Examples

```koto
# Copying a map
x = {foo: -1, bar: 99}
y = x
y.foo = 42
print! x.foo
check! 42

z = koto.copy x
z.bar = -1
print! x.bar # x.bar remains unmodified due to the copy
check! 99
```

```koto
# Copying a list

x = (1..=10).iter()
y = x # y shares the same iteration position as x.
z = koto.copy x # z shares the same iteration data (the range 1..=10),
                # but has a unique iteration position.

print! x.next()
check! 1
print! x.next()
check! 2
print! y.next() # y shares x's iteration position.
check! 3
print! z.next() # z's iteration hasn't been impacted by the advancing of x and y.
check! 1
```

### See also

- [`koto.deep_copy`](#deep-copy)


## deep_copy

```kototype
|Value| -> Value
```

Makes a unique _deep_ copy of the value's data.

### Shared mutable data

This makes a unique copy of the value's data, and then recursively makes deep
copies of any nested containers in the value.

If only the first level of data needs to be made unique, then use
[`koto.copy`](#copy).

### Example

```koto
x = [[1, 2], [3, [4, 5]]]
y = koto.deep_copy x
y[1][1] = 99
print! x # a deep copy has been made, so x is unaffected by the assignment to y
check! [[1, 2], [3, [4, 5]]]
```

### See also

- [`koto.copy`](#copy)


## exports

```kototype
|| -> Map
```

Returns the current module's `export` map.

Although typically module items are exported with `export` expressions,
it can be useful to export items programatically.


## hash

```kototype
|Value| -> Value
```

Returns the value's hash as an integer, or Null if the value is not hashable.

```koto
from koto import hash

print! (hash 'hi') == (hash 'bye')
check! false

# Lists aren't hashable
print! hash [1, 2] 
check! null

# Tuples are hashable if they only contain hashable values 
print! (hash (1, 2)) == null
check! false
```

## script_dir

```kototype
String or Null
```

If a script is being executed then `script_dir` provides the directory that the
current script is contained in as a String, otherwise `script_dir` is Null.

## script_path

```kototype
String or Null
```

If a script is being executed then `script_path` provides the path of the
current script as a String, otherwise `script_path` is Null.

## type

```kototype
|Value| -> String
```

Returns the type of the input Value as a String.

Note that a map value can override the value returned from `type` by defining
the `@type` meta value, for more information see
[the reference for map](map.md#meta-maps-and-overloaded-operations).

### Example

```koto
print! koto.type true
check! Bool

x = 42
print! koto.type x
check! Int

foo =
  @type: "Foo"
print! koto.type foo
check! Foo
```
