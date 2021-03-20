# Koto

[![Docs](https://docs.rs/koto/badge.svg)][docs]
[![Crates.io](https://img.shields.io/crates/v/koto.svg)][crates]
[![CI](https://github.com/koto-lang/koto/workflows/CI/badge.svg)][ci]

Koto is an embeddable scripting language, written in Rust. It has been designed
for ease of use and built for speed, with the goal of it being an ideal choice
for adding scripting to Rust applications.

Koto is versatile enough to be useful in a variety of applications, although
there has been a focus during development on interactive systems, such as rapid
iteration during game development, or experimentation in creative coding.


* [Current State](#current-state)
* [Getting Started](#getting-started)
  * [A Quick Tour](#a-quick-tour)
  * [Learning the Language](#learning-the-language)
  * [Installation](#installation)
  * [REPL](#repl)
* [Language Goals](#language-goals)
* [Editor Support](#editor-support)
  * [Vim / Neovim](#vim--neovim)


## Current State

The language itself is far enough along that I'm happy to share it with the
wider world, although you should be warned that it's at a very early stage of
development, and you can expect to find missing features, usability quirks, and
bugs. Parts of the language are likely to change in response to it being used in
more real-world contexts. We're some distance away from a stable 1.0 release.

That said, if you're curious and feeling adventurous then please give Koto
a try, your early feedback will be invaluable.


## Getting Started

### A Quick Tour

```coffee
import test.assert_eq

# Numbers
assert_eq (1 + 2.5), 3.5

# Strings
hello = "{}, {}!".format "Hello", "World"
hello.print()

# Functions
square = |n| n * n
assert_eq (square 8), 64

add_squares = |a, b| (square a) + (square b)
assert_eq (add_squares 2, 4), 20

# Iterators, ranges, and lists
fizz_buzz = (1..100)
  .keep |n| (10..=15).contains n
  .each |n|
    match n % 3, n % 5
      0, 0 then "Fizz Buzz"
      0, _ then "Fizz"
      _, 0 then "Buzz"
      else n
  .to_list()
assert_eq
  fizz_buzz,
  ["Buzz", 11, "Fizz", 13, 14, "Fizz Buzz"]

# Maps and tuples
x = {foo: 42, bar: "bar"}
assert_eq x.keys().to_tuple(), ("foo", "bar")
```


### Learning the Language

While there's not yet a complete guide to Koto, there are some code examples
that are a good starting point for getting to know the language.

* [Koto test scripts, organized by feature](./koto/tests/)
* [Koto benchmark scripts](./koto/benches/)
* [Example Rust application with Koto bindings](./examples/poetry/)



### Installation

The most recent release of the Koto CLI can be installed with
[Cargo](https://rustup.rs):

```
cargo install koto_cli
```


### REPL

A [REPL][repl] is provided to allow for quick experimentation.
Launching the `koto` CLI without providing a script enters the REPL.

```
» koto
Welcome to Koto v0.1.0
» 1 + 1
2
» "{}, {}!".print "Hello", "World"
Hello, World!
()
```


## Language Goals

* A clean, minimal syntax designed for coding in creative contexts.
* Fast compilation.
  * The lexer, parser, and compiler are all written with speed in mind,
    enabling as-fast-as-possible iteration when working on an idea.
* Fast and predictable runtime performance.
  * Memory allocations are reference counted.
  * Currently there's no tracing garbage collector (and no plan to add one)
    so memory leaks are possible if cyclic references are created.
* Lightweight integration into host applications.
  * One of the primary use cases for Koto is for it to be embedded as a library
    in other applications, so it should be a good citizen and not introduce too
    much overhead.


[crates]: https://crates.io/crates/koto
[ci]: https://github.com/koto-lang/koto/actions
[docs]: https://docs.rs/koto
[repl]: https://en.wikipedia.org/wiki/Read–eval–print_loop


## Editor Support

* [Visual Studio Code](https://github.com/koto-lang/koto-vscode)
* [Vim / Neovim](https://github.com/koto-lang/koto.vim)
* [Sublime Text](https://github.com/koto-lang/koto-sublime)
