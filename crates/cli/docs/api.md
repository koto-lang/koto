A rendered version of this document can be found
[here](https://koto.dev/docs/next/api).

The Rust code examples are included from the
[Koto examples dir](../../koto/examples).

---

# Rust API

This document contains a collection of examples of how to interact with Koto from Rust code.

The complete API documentation can be found [here][koto-docs].

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

## Using Serde for Value Conversions

Types that implement `serde::Deserialize` and `Serialize` can be converted
to and from Koto values via `koto::serde::to_koto_value` and `from_koto_value`.

```rust_include
serde.rs
```

## Adding Values to the Prelude

The runtime's prelude is a `KMap`, which is Koto's standard hashmap type.

Values can be added to the prelude via `KMap::insert`, taking any Rust value
that implements `Into<KValue>`. Basic types like strings and numbers are
automatically converted to corresponding Koto types.

```rust_include
prelude_value_insert.rs
```

## Removing Values from the Prelude

Values can also be removed from the prelude, which can be useful if you want
to restrict the capabilities of a script.

```rust_include
prelude_value_remove.rs
```

## Passing Arguments to Koto

The arguments that are accessible in a script from `os.args` can be set via
`KotoSettings::with_args`.

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

## Running Async Scripts

`Koto::run` blocks until the script has finished, including any suspended async work.
Use `Koto::run_async` or `Koto::compile_and_run_async` when the host application
already has an async executor and should drive suspended Koto work itself.

The example uses Tokio's current-thread runtime to provide the host executor.

```rust_include
async_tasks.rs
```

Koto's async semantics are executor-agnostic, but executor-backed modules still
depend on a host-provided async backend.

The Koto CLI installs one when built with the `tokio` feature, which enables
modules like `task`, `io_async`, and `http`. Embedding applications can install
whatever backend they want to support, or omit one entirely and stick to the
blocking APIs.

## Async Native APIs

`KotoObject` trait hooks like `index`, `index_assign`, `call`, and operator
overloads are synchronous.

If a native API may need to execute Koto code or suspend while async work is
pending, then it should be exposed as a VM-aware function or method instead:

- module functions via `KMap::add_vm_fn`
- object methods via `#[koto_vm_method]`
- lower-level wrappers via `KNativeVmFunction`

This is the path used by the core library for suspendable operations, and keeps
pending state visible to the VM.

## Adding a Module to the Prelude

A module in Koto is simply a `KMap`, conventionally with a defined
[`@type`][type].

```rust_include
module.rs
```

## Adding a Custom Object Type

Any Rust type that implements `KotoObject` can be used in the Koto runtime.
`KotoObject` requires `KotoType`, `KotoCopy`, and `KotoAccess` to be
implemented.

```rust_include
rust_object.rs
```

## Disabling type checks

Runtime type checks are enabled by default, the compiler can be prevented from
emitting type check instructions by disabling the `enable_type_checks` flag.

```rust_include
disabling_type_checks.rs
```

## Using the multi-threaded runtime

By default, Koto's runtime is single-threaded, and many of its core types (e.g. `KValue`) don't
implement `Send` or `Sync`.

For applications that need to support multi-threaded scripting, the `arc` feature switches from an
`Rc<RefCell<T>>`-based memory strategy to one using `Arc<RwLock<T>>`.

Only one memory strategy can be enabled at a time, so default features need to be disabled.

```toml
# Cargo.toml
# ...

[dependencies.koto]
version = "0.15"
default-features = false
features = ["arc"]
```

## Using Koto in a REPL

Some applications (like REPLs) require assigned variables to persist between each script evaluation.
This can be achieved by enabling the `export_top_level_ids` flag,
which will result in all top-level assignments being exported.

```rust_include
using_koto_in_a_repl.rs
```

---

[koto-docs]: https://docs.rs/koto/latest/koto/
[type]: ./language_guide.md#type
