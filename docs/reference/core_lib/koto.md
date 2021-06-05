# koto

A collection of utilities for working with the Koto runtime.

# Reference

- [args](#args)
- [current_dir](#current_dir)
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

## current_dir

`|| -> String`

Returns the current working directory as a String, or Empty if the current
directory can't be retrieved.

## exports

`|| -> Map`

Returns the current module's `export` map.

Although typically module items are exported with `export` expressions,
it can be useful to export items programatically.


## script_dir

`String`

Provides the directory that the current script is contained in as a String.

## script_path

`String`

Provides the path of the current script as a String.

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
