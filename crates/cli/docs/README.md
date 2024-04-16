# Koto Docs

The docs here serve as the source material for the
[Koto website's docs section](https://koto.dev/docs/next),
and are also included in the `help` command of the CLI. 

## Code Examples

### `print!` and `check!`

The code examples in the docs make use of `print!` and `check!` placeholders 
which are used by various preprocessor tools.
- The ['docs examples' tests](../../koto/tests/docs_examples.rs) validate
  that the code examples work correctly by checking the example's output against
  expectations defined by the `check!` commands.
- The [CLI's help command](../src/help.rs) replaces the `check!`
  commands with comments showing the expected output.

### `skip_check` and `skip_run`

Code examples tagged with `skip_run` will be checked to ensure that they can be 
compiled, but won't be executed.

`skip_check` will check that the script can be compiled and executed, 
but the script's output won't be validated.
