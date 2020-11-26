# Koto

Koto is an embeddable scripting language written in Rust, designed for ease of
use, efficiency, and being a lightweight addition to larger applications.

Koto should be versatile enough to be usable in a variety of applications, but
there has been an emphasis during development on interactive systems, such as
rapid iteration during game development, or experimentation in creative coding.


## Example

```koto
# Items from other modules can be brought into the current module with 'import'
import test.assert_eq

# Items from the current module can be made available externally with 'export'
export main = ||
  # Functions are created with || and can be assigned to values
  fizz_buzz = |n|
    # Match expressions allow for ergonomic conditional logic
    match n % 3, n % 5
      0, 0 then "Fizz Buzz"
      0, _ then "Fizz"
      _, 0 then "Buzz"
      _ then n

  # A powerful iterator library allows for expressive data pipelines
  result = (1..100)
    .keep |n| n >= 10 and n <= 15
    .each |n| fizz_buzz n
    .to_tuple()
  assert_eq
    result
    ("Buzz", 11, "Fizz", 13, 14, "Fizz Buzz")
```


## Current state

The language itself is far enough along that I'm happy to share it with the
wider world, although you should be warned that it's very early days,
and you can expect to find missing features, usability quirks, and bugs galore!
Parts of the language are likely to change in response to it being used
more in real-world contexts. We're some distance away from a stable 1.0 release.

That said, if you're curious and feeling adventurous then please give Koto
a try, your early feedback will be invaluable!


## Getting started

### Learning the language

While there's currently no complete guide to Koto, there are some examples
that can be used as a starting point for getting to know the language.

* [Example application with Koto bindings](./examples/poetry/)
* [Example tests, organized by feature](./koto/tests/)
* [Benchmark scripts](./src/koto/benches/)

### REPL

A [REPL][1] is provided to allow for quick experimentation.
Launching `koto` without a script enters the REPL by default.

[1]: https://en.wikipedia.org/wiki/Read–eval–print_loop


## Design Goals

* A clean, minimal syntax designed for coding in creative contexts.
  * Minimizing visual noise was an early goal.
    Along the way I've let go of ideas like 'no commas!', but the visual
    appearance of the language is still more on the minimal side.
* Fast compilation.
  * The lexer, parser, and compiler are all written with speed in mind,
    enabling as-fast-as-possible iteration when working on an idea.
* Predictable runtime performance.
  * Memory is reference counted. Currently there's no garbage collector so
    memory leaks are possible if cyclic references are created.
* Lightweight integration into host applications.
  * One of the primary use cases for Koto is for it to be embedded as a library
    in other applications, so it should be a good citizen and not introduce too
    much overhead.
