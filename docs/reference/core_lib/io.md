# io

A collection of utilities for working with the local filesystem.

# Reference

- [create](#create)
- [current_dir](#current_dir)
- [exists](#exists)
- [open](#open)
- [print](#print)
- [read_to_string](#read_to_string)
- [remove_file](#remove_file)
- [stderr](#stderr)
- [stdin](#stdin)
- [stdout](#stdout)
- [temp_dir](#temp_dir)
- [File](#file)
- [File.flush](#fileflush)
- [File.path](#filepath)
- [File.read_line](#fileread_line)
- [File.read_to_string](#fileread_to_string)
- [File.seek](#fileseek)
- [File.write](#filewrite)
- [File.write_line](#filewrite_line)

## create

`|String| -> File`

Returns an empty `File` at the provided path.
If the file already exists it will be truncated.

### Errors

A runtime error will be thrown if the file can't be created.

### Example

```koto
f = io.create "foo.temp"
f.write_line "Hello"
f.read_to_string()
# Hello
```

## current_dir

`|| -> String`

Returns the current working directory as a String, or Empty if the current
directory can't be retrieved.

## exists

`|String| -> Bool`

Returns true if a file exists at the provided path.

### Example

```koto
path = "foo.temp"
io.exists path
# false

io.create path
io.exists path
# true
```

## open

`|String| -> File`

Opens the file at the given path, and returns a corresponding `File`.

### Errors

An error is thrown if a file can't be opened at the given path.

### Example

```koto
f = io.open "path/to/existing.file"
f.exists()
# true
```

## print

`|Value| -> ()`
`|String, Value...| -> ()`

Prints a formatted string to the active logger,
by default this is the standard output.

### Note

See `string.format` for the formatting syntax.

## read_to_string

`|String| -> String`

Returns a string containing the contents of the file at the given path.

### Errors

Errors are thrown:

- if the file doesn't contain valid UTF-8 data.
- if a file can't be opened at the given path.

### Example

```koto
f = io.create "foo.temp"
f.write_line "Hello!"
io.read_to_string "foo.temp"
# Hello!
```

## remove_file

`|String| -> ()`

Removes the file at the given path.

### Errors

- An error is thrown if a file can't be removed at the given path.

### Example

```koto
path "foo.temp"
io.create path
io.exists path
# true

io.remove_file path
io.exists path
# false
```

## stderr

`|| -> File`

Returns the standard error output of the current process as a file.

### Example

```koto
io.stderr().write_line "An error occurred!"
```

### See Also

- [`io.stdin`](#stdin)
- [`io.stdout`](#stdout)

## stdin

`|| -> File`

Returns the standard input of the current process as a file.

### Example

```koto
io.stdin().read_to_string()
# "..."
```

### See Also

- [`io.stderr`](#stderr)
- [`io.stdout`](#stdout)

## stdout

`|| -> File`

Returns the standard output of the current process as a file.

### Example

```koto
io.stdout().write_line "Hello, World!"
```

### See Also

- [`io.stderr`](#stderr)
- [`io.stdin`](#stdin)

## temp_dir

`|| -> String`

Returns the path to a temporary directory.

### Note

This defers to Rust's `std::env::temp_dir`, for details see
[its documentation](https://doc.rust-lang.org/std/env/fn.temp_dir.html).

## File

A map that wraps a file handle, returned from functions in `io`.

## File.flush

`|File| -> ()`

Ensures that any buffered changes to the file have been written.

### See Also

- [`file.write`](#filewrite)
- [`file.write_line`](#filewrite_line)

## File.path

`|File| -> String`

Returns the file's path.

## File.read_line

`|File| -> String or Empty`

Reads a line of output from the file as a string, not including the newline.

When the end of the file is reached, Empty will be returned.

### Errors

An error is thrown if the line doesn't contain valid UTF-8 data.

## File.read_to_string

`|File| -> String`

Reads the file's contents to a string.

### Errors

An error is thrown if the file doesn't contain valid UTF-8 data.

## File.seek

`|File, Number| -> ()`

Seeks within the file to the specified position in bytes.

## File.write

`|File, Value| -> ()`

Writes the formatted value as a string to the file.

## File.write_line

`|File, Value| -> ()`

Writes the formatted value as a string, with a newline, to the file.
