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
size koto.args
# 3
koto.args.first()
# 1
koto.args.last()
# hello
```

## copy

```kototype
|value: Any| -> Any
```

Makes a copy of the provided value. 

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

print! x.next().get()
check! 1
print! x.next().get()
check! 2
print! y.next().get() # y shares x's iteration position.
check! 3
print! z.next().get() # z isn't impacted by the advancing of x and y.
check! 1
```

### See also

- [`koto.deep_copy`](#deep-copy)


## deep_copy

```kototype
|value: Any| -> Any
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
|value: Any| -> Number or Null
```

Returns the value's hash as an integer, or Null if the value is not hashable.

### Example

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

## load

```kototype
|script: String| -> Chunk
```

Compiles the provided Koto `script` and returns a compiled `Chunk`.

Any compilation errors get thrown.

### Example

```koto
chunk = koto.load '1 + 2'
print! koto.run chunk
check! 3
```

### See also

- [`koto.run`](#run)

## run

```kototype
|script: String| -> Any
```

Compiles and runs the provided Koto `script`, and returns the resulting value.

Any compilation or runtime errors get thrown.

```kototype
|Chunk| -> Any
```

Runs the compiled `Chunk`, and returns the resulting value.

Any runtime errors encountered during execution get thrown.

### Example

```koto
print! koto.run '[1, 2, 3, 4].sum()'
check! 10
```

### See also

- [`koto.load`](#load)

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

## size

```kototype
|value: Any| -> Number
```

Returns the _size_ of a value.

The size of a value is typically defined as the number of elements in a
container, with some notable exceptions:

- For strings, the size is the number of bytes in the string data.
- For ranges, the size is the number of integers in the range. 
  - For non-inclusive ranges, this is equivalent to 
    `range.end() - range.start()`.
  - For inclusive ranges, this is equivalent to 
    `range.end() + 1 - range.start()`.
  - If the range is unbounded then an error will be thrown.
- An error will be thrown if the value doesn't have a defined size.

### Example

```koto
from koto import size

print! (size [1, 2, 3]), (size (,))
check! (3, 0)

print! (size 'hello'), (size 'héllø'), (size '')
check! (5, 7, 0)

print! (size 10..20), (size 10..=20), (size 20..0)
check! (10, 11, 20)
```


## type

```kototype
|value: Any| -> String
```

Returns the type of the input value as a String.

### Example

```koto
print! koto.type true
check! Bool

x = 42
print! koto.type x
check! Number

foo =
  @type: "Foo"
print! koto.type foo
check! Foo
```
