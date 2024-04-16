# tempfile

Utilities for working with temporary files in Koto.

## temp_file

```kototype
|| -> File
```

Creates and returns a temporary file.

This is a wrapper for `NamedTempFile` from the `tempfile` crate, please refer to
the [documentation][crate-docs] for more information.

### Example

```koto,skip_run
f = temp_file.tempfile()
print! f.path()
check! /path/to/a/temporary/file
```

[crate-docs]: https://docs.rs/tempfile/latest/tempfile/struct.NamedTempFile.html
