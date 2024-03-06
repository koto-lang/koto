# Getting Started

## The Koto CLI

Koto has been primarily designed as an embeddable extension language for Rust
projects, however it also offers a command-line interface (CLI). 
The Koto CLI enables you to run `.koto` scripts directly, 
and provides an interactive [REPL][repl]. 

Installing the Koto CLI currently requires the [Rust][rust]
toolchain (see [rustup.sh][rustup] for installation instructions).

With Rust available on your system, the `koto` command can be installed with
`cargo install koto_cli`.

### REPL

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

This guide, along with the [core library reference](../core), 
can be read in the REPL using the `help` command. 

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

[repl]: https://en.wikipedia.org/wiki/Read–eval–print_loop
[rust]: https://rust-lang.org 
[rustup]: https://rustup.sh
