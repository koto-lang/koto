# io

A collection of utilities for working with the local filesystem.

# Reference

- [create](#create)
- [exists](#exists)
- [open](#open)
- [read_to_string](#read_to_string)
- [remove_file](#remove_file)
- [temp_dir](#temp_dir)
- [File](#file)
- [File.path](#filepath)
- [File.read_to_string](#fileread_to_string)
- [File.seek](#fileseek)
- [File.write](#filewrite)
- [File.write_line](#filewrite_line)

## create

`|String| -> Map`

Returns an empty `File` map at the provided path.
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

`|String| -> Map`

Opens the file at the given path, and returns a corresponding `File` map.

### Errors

An error is thrown if a file can't be opened at the given path.

### Example

```koto
f = io.open "path/to/existing.file"
f.exists()
# true
```

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

## temp_dir

`|| -> String`

Returns the path to a temporary directory.

### Note

This defers to Rust's `std::env::temp_dir`, for details see
[its documentation](https://doc.rust-lang.org/std/env/fn.temp_dir.html).

## File

A map that wraps a file handle, returned from functions in `io`.

## File.path

`|File| -> String`

Returns the file's path.

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
