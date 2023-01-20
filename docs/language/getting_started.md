# Getting Started

## Installing Koto 

Installing the Koto command-line interface (CLI) currently requires the 
[Rust](https://rust-lang.org) toolchain, 
see [rustup.sh](https://rustup.sh) for installation instructions.

With Rust available on your system, the `koto` command can be installed with
`cargo install koto_cli`.

## REPL

Running `koto` without arguments will start the Koto 
[REPL](https://en.wikipedia.org/wiki/Read–eval–print_loop), where Koto
expressions can be entered and evaluated interactively. 


```lua
> koto
Welcome to Koto v0.11.0
» 1 + 1
➝ 2

» 'hello!'
➝ hello!
```

This guide, along with the [core library reference](../../core), 
can be read in the REPL using the `help` command. 

```lua
> koto
Welcome to Koto v0.11.0
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
