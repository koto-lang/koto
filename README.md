[![Koto](assets/koto.svg)][koto]

---

[![Docs](https://img.shields.io/docsrs/koto)][docs]
[![Crates.io](https://img.shields.io/crates/v/koto.svg)][crates]
[![CI](https://github.com/koto-lang/koto/workflows/CI/badge.svg)][ci]
[![Discord](https://img.shields.io/discord/894599423970136167?logo=discord)][discord]

Koto is an embeddable scripting language developed in Rust. 
Prioritizing speed and ease of use, its goal is to be an ideal option for 
adding scripting support to Rust applications.

During its early development there was a focus on interactive systems, 
such as rapid iteration during game development,
but Koto is versatile enough to be useful in a wide range of applications.

- [Current State](#current-state)
- [Getting Started](#getting-started)
  - [A Quick Tour](#a-quick-tour)
  - [Learning the Language](#learning-the-language)
  - [Installation](#installation)
- [Language Goals](#language-goals)
- [Editor Support](#editor-support)

## Current State

...I think we're getting somewhere?

The language is close to feature complete from my perspective, 
but it hasn't been used in enough real-world projects for me to suggest that 
anyone else should use it for anything serious. 
In particular, you can expect breaking changes to the language, 
although these are now becoming less frequent. 

That said, your feedback is invaluable to Koto's development. 
If you decide to try it out, please let me know how you get on.

## Getting Started

See the [Getting Started][getting-started] section of the language guide.

Reference documentation for Koto's core library can be found [here][core-lib].

You're also welcome to reach out in [Discussions][discussions],
or on [Discord][discord].

### A Quick Tour

```coffee,skip_check
## Strings
name = 'Koto'
print 'Hello, $name!'
# -> Hello, Koto!

## Functions
square = |n| n * n
print '8 squared is ${square 8}'
# -> 8 squared is 64

add_squares = |a, b| (square a) + (square b)
assert_eq (add_squares 2, 4), 20

## Iterators, Ranges, and Lists
fizz_buzz = 
  (1..100)
    .keep |n| (10..=15).contains n
    .each |n|
      match n % 3, n % 5
        0, 0 then 'Fizz Buzz'
        0, _ then 'Fizz'
        _, 0 then 'Buzz'
        else n
    .to_list()
print fizz_buzz
# -> ['Buzz', 11, 'Fizz', 13, 14, 'Fizz Buzz']

## Maps and tuples

### Maps can be defined with curly braces
fruits = {peaches: 42, pears: 99}

### Maps can also be defined using indented `key: value` pairs
more_fruits = 
  apples: 123
  plums: 99

fruits.extend more_fruits
print fruits.keys().to_tuple(),
# -> ('peaches', 'pears', 'apples', 'plums')

fruit, amount = fruits.max |(_, amount)| amount
print 'The highest amount of fruit is: $amount $fruit'
# -> The highest amount of fruit is: 123 apples
```

### Learning the Language

The [language guide](docs/language/_index.md) and 
[core library reference](docs/core_lib) give an overview of Koto's features. 

Rendered versions of the docs are available on the [Koto website](koto-docs).

There are also some code examples that are a good starting point for getting to 
know the language.

- [Koto test scripts, organized by feature](./koto/tests/)
- [Koto benchmark scripts](./koto/benches/)
- [Example Rust application with Koto bindings](./examples/poetry/)

### Installation

The most recent [release](cli-crate) of the Koto CLI can be installed with
[Cargo](https://rustup.rs):

```
cargo install koto_cli
```

To build and install the latest version of the CLI from source:

```
cargo install --path core/cli
```

## Language Goals

- A clean, minimal syntax designed for coding in creative contexts.
- Fast compilation.
  - The lexer, parser, and compiler are all written with speed in mind,
    enabling as-fast-as-possible iteration when working on an idea.
- Fast and predictable runtime performance.
  - Memory allocations are reference counted.
  - Currently there's no tracing garbage collector (and no plan to add one)
    so memory leaks are possible if cyclic references are created.
- Lightweight integration into host applications.
  - One of the primary use cases for Koto is for it to be embedded as a library
    in other applications, so it should be a good citizen and not introduce too
    much overhead.

## Editor Support

- [Visual Studio Code](https://github.com/koto-lang/koto-vscode)
- [Vim / Neovim](https://github.com/koto-lang/koto.vim)
- [Sublime Text](https://github.com/koto-lang/koto-sublime)
- [Tree-sitter](https://github.com/koto-lang/tree-sitter-koto)

## MSRV

Koto is still under active development, and is tested against the latest stable
release of Rust.

[ci]: https://github.com/koto-lang/koto/actions
[cli-crate]: https://crates.io/crates/koto_cli
[core-lib]: https://koto.dev/docs/next/core
[crates]: https://crates.io/crates/koto
[discord]: https://discord.gg/JeV8RuK4CT
[discussions]: https://github.com/koto-lang/koto/discussions
[docs]: https://docs.rs/koto
[getting-started]: https://koto.dev/docs/next/language/#getting-started
[koto]: https://koto.dev
[koto-docs]: https://koto.dev/docs/next
[repl]: https://en.wikipedia.org/wiki/Read–eval–print_loop
[tags]: https://github.com/koto-lang/koto/tags
