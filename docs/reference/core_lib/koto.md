# koto

A collection of utilities for working with the Koto runtime.

# Reference

- [args](#args)
- [exports](#exports)
- [script_dir](#script_dir)
- [script_path](#script_path)
- [type](#type)

## args

`Tuple`

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

`|| -> Map`

Returns the current module's `export` map.

Although typically module items are exported with `export` expressions,
it can be useful to export items programatically.


## script_dir

`String or Empty`

If a script is being executed then `script_dir` provides the directory that the
current script is contained in as a String, otherwise `script_dir` is Empty.

## script_path

`String or Empty`

If a script is being executed then `script_path` provides the path of the
current script as a String, otherwise `script_path` is Empty.

## type

`|Value| -> String`

Returns the type of the input Value as a String.

Note that a map value can override the value returned from `type` by defining
the `@type` meta value, for more information see
[the reference for map](map.md#meta-maps-and-overloaded-operations).

### Example

```koto
koto.type true
# Bool

x = 42
koto.type x
# Int

foo =
  @type: "Foo"
koto.type foo
# Foo
```
