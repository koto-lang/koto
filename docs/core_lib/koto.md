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
import koto.hash

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
