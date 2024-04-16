# The Koto CLI 

Koto was originally designed as an extension language for Rust applications, 
but it is also usable as a standalone scripting language via the Koto [CLI][cli]. 
The CLI can run `.koto` scripts, and provides an interactive [REPL][repl]. 

## Installation

Installing the Koto CLI currently requires the [Rust][rust] toolchain 
(see [rustup.sh][rustup] for installation instructions). 

With Rust available on your system, run `cargo install koto_cli`, 
which provides you with the `koto` command.

## Using the REPL

Running `koto` without any arguments will start the Koto REPL, 
where Koto expressions can be entered and evaluated interactively. 

```
> koto
Welcome to Koto 

» 1 + 1
➝ 2

» 'hello!'
➝ hello!
```
## Help

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
[core]: ./core_lib/
[guide]: ./language_guide.md
[repl]: https://en.wikipedia.org/wiki/Read–eval–print_loop
[rust]: https://rust-lang.org 
[rustup]: https://rustup.sh
