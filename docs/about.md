# About Koto

Koto is a simple and expressive programming language, usable as an extension
language for [Rust][rust] applications, or as a standalone scripting language.

Koto was started in 2020 with the goal to create an ideal language for adding 
scripting to applications developed in Rust. Of particular interest were 
interactive systems like animation or game engines, where rapid iteration 
demands a lightweight programming interface that compiles and runs quickly.

The guiding design principle is that Koto should be _simple_, both 
conceptually and visually. To that end, a focus throughout the language's development has 
been on reducing visual noise and minimizing core concepts wherever possible.

## Koto CLI 

Koto was designed as an extension language, but it is also usable as a
standalone scripting language via the Koto [CLI][cli]. 
The CLI can run `.koto` scripts, and provides an interactive [REPL][repl]. 

Installing the Koto CLI currently requires the [Rust][rust] toolchain 
(see [rustup.sh][rustup] for installation instructions). 

With Rust available on your system, run `cargo install koto_cli`, 
which provides you with the `koto` command.

### Using the REPL

Running `koto` without any arguments will start the Koto REPL, 
where Koto expressions can be entered and evaluated interactively. 

```
> koto
Welcome to Koto v0.11.0
» 1 + 1
➝ 2

» 'hello!'
➝ hello!
```

The [language guide][guide] and the [core library reference][core], 
can be accessed in the REPL using the `help` command. 

```
> koto
Welcome to Koto
» help bool

  Booleans
  ========

  Booleans are declared with the `true` and `false` keywords, 
  and combined using the `and` and `or` operators.

  |
  |  true and false
  |  # ➝ false
  ...
```

[cli]: https://en.wikipedia.org/wiki/Command-line_interface
[core]: ../core_lib
[guide]: language_guide.md
[repl]: https://en.wikipedia.org/wiki/Read–eval–print_loop
[rust]: https://rust-lang.org 
[rustup]: https://rustup.sh
