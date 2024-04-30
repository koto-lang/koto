# Koto Poetry

An example of integrating Koto into a Rust application, in this case a simple
Markov chain generator that produces generated 'poetry'.

`cargo run --example poetry -- -s scripts/readme.koto`

## `poetry.rs`

The poetry generator, implemented in Rust.

## `koto_bindings.rs`

Bindings that expose the poetry generator to the Koto runtime.

## `main.rs`

A small CLI application that runs sets up a Koto runtime with the bindings
to the poetry generator, and then runs a user-provided script.

## `scripts/`

Example scripts to run with the poetry generator app.
