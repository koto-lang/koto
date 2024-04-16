A rendered version of this document can be found
[here](https://koto.dev/docs/next/api).

The Rust code examples are included from the 
[Koto examples dir](../../koto/examples).

---

# Rust API Cookbook

## Hello World

To run a Koto script, instantiate `koto::Koto` and call `compile_and_run`:

```rust_include
hello_world.rs
```

## Getting a Return Value

The result of calling `compile_and_run` is a `KValue`, which is Koto's main
value type.

`KValue` is an enum that contains variants for each of the core Koto types, 
like `Number`, `String`, etc.

The type of a `KValue` as a string can be retrieved via `KValue::type_as_string`,
and to render a `KValue`, call `Koto::value_to_string`.

```rust_include
return_value.rs
```

## Getting an Exported Value

Values that are exported from the script are inserted in to the _exports_ map,
which can be accessed by calling `Koto::exports()`.

```rust_include
exported_values.rs
```

## Adding Values to the Prelude

The runtime's prelude is a `KMap`, which is Koto's standard hashmap type. 

Values can be added to the prelude via `KMap::insert`, taking any Rust value
that implements `Into<KValue>`. Basic types like strings and numbers are
automatically converted to corresponding Koto types. 

```rust_include
prelude_value.rs
```

## Passing Arguments to Koto

The arguments that are accessible in a script from `koto.args` can be set via
`Koto::set_args`.

```rust_include
args.rs
```

## Calling Rust Functions in Koto

Any Rust function that implements `KotoFunction` can be made available to the
Koto runtime. 

```rust_include
rust_function.rs
```

## Calling Koto Functions in Rust

`Koto::call_function` can be used to call Koto functions, or any other callable
Koto values.



```rust_include
koto_function.rs
```

## Adding a Module to the Prelude


A module in Koto is simply a `KMap`, conventionally with a defined
[`@type`][type].

```rust_include
module.rs
```

## Adding a Custom Object Type

Any Rust type that implements `KotoObject` can be used in the Koto runtime.
`KotoObject` requires `KotoType`, `KotoCopy`, and `KotoEntries` to be
implemented. 

```rust_include
rust_object.rs
```

[type]: ./language_guide.md#type
