# io

A collection of utilities for working with the local filesystem.

## create

```kototype
|path: String| -> File
```

Returns an empty [`File`](#file) at the provided path.
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

```kototype
|| -> String?
```

Returns the current working directory as a String, or `null` if the current
directory can't be retrieved.

## exists

```kototype
|path: String| -> Bool
```

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

## extend_path

```kototype
|path: String, nodes: Any...| -> String
```

Takes an initial path as a string, and extends it with the provided nodes,
inserting a platform-appropriate separator between each node.

### Example

```koto
# On Windows
io.extend_path ".", "foo", "bar", "baz.txt"
# .\foo\bar\baz.txt

# On Linux
io.extend_path ".", "foo", "bar", "baz.txt"
# ./foo/bar/baz.txt
```

## open

```kototype
|path: String| -> File
```

Opens the file at the given path, and returns a corresponding [`File`](#file).

### Errors

An error is thrown if a file can't be opened at the given path.

### Example

```koto
f = io.open "path/to/existing.file"
f.exists()
# true
```

## print

```kototype
|Any| -> Null
```

Prints a single value to the active output.

```kototype
|Any, Any...| -> Null
```

Prints a series of values to the active output as a tuple.

### Note

- To print formatted strings, see [`string.format`](./string.md#format).
- The output for `print` depends on the configuration of the runtime.
  The default output is `stdout`.

## read_to_string

```kototype
|path: String| -> String
```

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

```kototype
|path: String| -> Null
```

Removes the file at the given path.

### Errors

- An error is thrown if a file can't be removed at the given path.

### Example

```koto
path = "foo.temp"
io.create path
io.exists path
# true

io.remove_file path
io.exists path
# false
```

## stderr

```kototype
File
```

The standard error output of the current process as a [`File`](#file).

### Example

```koto
io.stderr.write_line "An error occurred!"
```

### See Also

- [`io.stdin`](#stdin)
- [`io.stdout`](#stdout)

## stdin

```kototype
File
```

The standard input of the current process as a [`File`](#file).

### Example

```koto
io.stdin.read_to_string()
# "..."
```

### See Also

- [`io.stderr`](#stderr)
- [`io.stdout`](#stdout)

## stdout

```kototype
File
```

The standard output of the current process as a [`File`](#file).

### Example

```koto
io.stdout.write_line "Hello, World!"
```

### See Also

- [`io.stderr`](#stderr)
- [`io.stdin`](#stdin)

## temp_dir

```kototype
|| -> String
```

Returns the path to a temporary directory.

### Note

This defers to Rust's `std::env::temp_dir`, for details see
[its documentation](https://doc.rust-lang.org/std/env/fn.temp_dir.html).

## File

An object that represents a file handle.

## File.flush

```kototype
|File| -> Null
```

Ensures that any buffered changes to the file have been written.

### See Also

- [`file.write`](#file-write)
- [`file.write_line`](#file-write-line)

## File.is_terminal

```kototype
|File| -> Bool
```

Returns `true` if the file refers to a terminal/tty.

### Example

```koto
next_line = if io.stdin.is_terminal()
  print 'Please provide some input'
  io.stdin.read_line()
else
  io.stdin.read_line()
```

## File.path

```kototype
|File| -> String
```

Returns the file's path.

## File.read_line

```kototype
|File| -> String?
```

Reads a line of output from the file as a string, not including the newline.

When the end of the file is reached, `null` will be returned.

### Errors

An error is thrown if the line doesn't contain valid UTF-8 data.

## File.read_to_string

```kototype
|File| -> String
```

Reads the file's contents to a string.

### Errors

An error is thrown if the file doesn't contain valid UTF-8 data.

## File.seek

```kototype
|File, position: Number| -> Null
```

Seeks within the file to the specified position in bytes.

## File.write

```kototype
|File, Any| -> Null
```

Writes the formatted value as a string to the file.

## File.write_line

```kototype
|File, Any| -> Null
```

Writes the formatted value as a string, with a newline, to the file.
