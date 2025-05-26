# The Koto CLI

Koto was originally designed as an extension language for Rust applications,
but it is also usable as a standalone scripting language via the Koto [CLI][cli].
The CLI can run `.koto` scripts, and provides an interactive [REPL][repl].

## Installation

Installing the Koto CLI currently requires the [Rust][rust] toolchain
(see [rustup.sh][rustup] for installation instructions).

With Rust available on your system, run `cargo install koto_cli`,
which provides you with the `koto` command.

## Running Scripts

Koto scripts can be run by the CLI by passing the script's name as the first argument.

```
» cat say_hello.koto
print 'Hello!'

» koto say_hello.koto
Hello!
```

### Command Arguments

Arguments following the script are made available via [`os.args`][os-args].

```
» cat print_args.koto
for i, arg in os.args.enumerate()
  print '{i + 1}: {arg}'

» koto print_args.koto a b c
1: a
2: b
3: c
```

### Running Tests

Tests are disabled by default in the CLI, but can be enabled by using the `--tests` flag.

```
» cat testing.koto
@main = ||
  print 'Hello!'

@test always_fails = ||
  assert false

» koto testing.koto
Hello!

» koto --tests testing.koto
Error: assertion failed (while running test 'always_fails')
--- testing.koto - 5:3
   |
 5 |   assert false
   |   ^^^^^^^^^^^^
```

`--tests` only enables tests in the script that's being run,
use the `--import_tests` flag to also enable tests in any imported modules.

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

## Formatting

The `koto` CLI can format scripts with the `--format` flag.
If a script path is provided then the file will be formatted in place,
otherwise the script will be read from `stdin` and written to `stdout`.

## Configuration

Options for formatting and the REPL can be chosen by exporting them from a `config.koto` file, which is expected to be placed in `~/.koto/config.koto`.

The default configuration can be displayed with the `--print_config` flag.

### Format Options

- `always_indent_arms`: Whether or not `match` and `switch` arms should always be indented. (default: `false`)
- `chain_break_threshold`: The threshold that causes chain expressions to be broken onto multiple lines. (default: `4`)
  - The threshold counts against the number of `.` accesses that are followed by a call or index.
    - `a.b.c().d()` - 2 `.` accesses that count against the threshold.
    - `a[0].b[1].c().d().e()` - 4 `.` accesses that count against the threshold.
  - A value of `0` disables the threshold.
- `indent_width`: The width in characters to use when inserting indents. (default: `2`)
- `line_length`: The maximum line length. (default: `100`)

### REPL Options

- `edit_mode`: The editing keybindings that should be enabled in the REPL. (`emacs` or `vim`, default: `emacs`)
- `colored_output`: Whether or not the REPL should use colored output. (default: `true`)
- `max_history`: The maximum number of entries to keep in its persistent history (default: `100`)
  - The history is stored in `~/.koto/repl_history.txt`


[cli]: https://en.wikipedia.org/wiki/Command-line_interface
[core]: ./core_lib/
[os-args]: ./core_lib/os.md#args
[guide]: ./language_guide.md
[repl]: https://en.wikipedia.org/wiki/Read–eval–print_loop
[rust]: https://rust-lang.org
[rustup]: https://rustup.sh
