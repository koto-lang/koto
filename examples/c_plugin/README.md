# C Plugin Example

This is a minimal proof-of-concept Koto plugin implemented in C against the
generated FFI header.

It exports:

- `answer`: `42`
- `sum`: a native function that adds two `i64` arguments

Build it with:

```bash
just -f examples/c_plugin/justfile build
```

The `build` recipe generates a local `include/koto.h` header with `cbindgen`
before compiling the plugin.

Run the example with:

```bash
just -f examples/c_plugin/justfile run
```

Or load it from Koto with:

```koto
from 'native:./libkoto_c_plugin.dylib' import answer, sum
```

The example script is in [example.koto](/Users/ian/dev/koto/koto/examples/c_plugin/example.koto).
