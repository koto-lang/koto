A rendered version of this document can be found
[here](https://koto.dev/docs/next/api).

The Rust code examples are included from the [Koto examples
dir](/crates/koto/examples).

---

# Rust API Cookbook

## Hello World

To run a Koto script, instantiate `koto::Koto` and call `compile_and_run`:

```rust_include
hello_world.rs
```

## Getting a Return Value

The result of calling `compile_and_run` is a _Koto value_, aka `KValue`.

`KValue` is an enum that contains variants for each of the core Koto value
types, like `Number`, `String`, etc.

The type of a `KValue` as a string can be retrieved via `KValue::type_as_string`,
and to render a `KValue`, call `Koto::value_to_string`.

```rust_include
return_value.rs
```

## Adding Values to the Prelude

The runtime's prelude is a `KMap`, which is Koto's standard hashmap type. 

Values can be added via `KMap::insert`, taking any value that implements
`Into<KValue>`. Basic types like strings and numbers are automatically converted
to corresponding Koto types. 

```rust_include
prelude_value.rs
```

## Passing Arguments to Koto

```rust_include
args.rs
```


## Rust Functions in Koto

Any Rust function that implements `KotoFunction` can be made available to the
Koto runtime. 

```rust_include
rust_function.rs
```

## Adding a Module to the Prelude


A module in Koto is simply a `KMap`, conventionally with a defined `@type`,
which contains a collection of useful functionality.

```rust_include
module.rs
```

## Adding a Custom Object Type

Rust types that implement `KotoObject` can be used in the Koto runtime.

```rust_include
rust_object.rs
```
