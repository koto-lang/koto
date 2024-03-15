# string

## bytes

```kototype
|String| -> Iterator
```

Returns an iterator that yields a series of Numbers representing the bytes
contained in the string data.

### Example

```koto
print! 'Hëy!'.bytes().to_tuple()
check! (72, 195, 171, 121, 33)
```

### See Also

- [`string.from_bytes`](#from-bytes)

## chars

```kototype
|String| -> Iterator
```

Returns an iterator that yields the string's characters as strings.

### Note

A 'character' in Koto is defined as a grapheme, so `.chars()` iterates over the
string's grapheme clusters.

### Note

Note that this is the default iteration behaviour for a string, so calling
`.chars()` on a string is equivalent to calling `iterator.iter()`.

### Example

```koto
print! 'Héllø! 👋'.chars().to_tuple()
check! ('H', 'é', 'l', 'l', 'ø', '!', ' ', '👋')
```

## contains

```kototype
|String, String| -> Bool
```

Returns `true` if the second provided string is a sub-string of the first.

### Example

```koto
print! 'xyz'.contains 'abc'
check! false

print! 'xyz'.contains 'yz'
check! true

print! 'xyz'.contains 'xyz'
check! true

print! 'xyz'.contains ''
check! true
```

## ends_with

```kototype
|String, String| -> Bool
```

Returns `true` if the first string ends with the second string.

### Example

```koto
print! 'abcdef'.ends_with 'def'
check! true

print! 'xyz'.ends_with 'abc'
check! false
```

## escape

```kototype
|String| -> String
```

Returns the string with characters replaced with escape codes.

For example, newlines get replaced with `\n`, tabs get replaced with `\t`.

### Example

```koto
print! '👋'.escape()
check! \u{1f44b}
```

## format

```kototype
|String, Value...| -> String
```

Returns a formatted string, with the arguments being assigned to
`{}` placeholders in the format string.

### Formatting Syntax

The syntax for format strings in Koto is similar to
[Rust's formatting syntax](https://doc.rust-lang.org/std/fmt/).

#### Placeholders

- `{}`
  - Takes the next value from the list of arguments, starting with the first.
  - Subsequent `{}` placeholders will take following values.
- `{0}, {1}, {2}, ...`
  - Takes the value at the specified index.
- `{x}, {name}, {id}`
  - Takes values by name from a Map.
    - The Map is expected to be the first argument after the format string.

`{` characters can be included in the output string by escaping them with
another `{`, e.g. `'{{}}'.format()` will output `'{}'`.

#### Formatting modifiers

Modifiers can be provided after a `:` separator in the format string.

##### Minimum width, fill, and alignment

A minimum width can be specified, ensuring that the formatted value takes up at
least that many characters, e.g. `'x{:4}x'.format 'ab'` will output `xab  x`.

The minimum width can be prefixed with an alignment modifier:

- `<` - left-aligned
- `^` - centered
- `>` - right-aligned

e.g. `'x{:>4}x'.format 'ab'` will output `x  abx`.

Values are left-aligned by default, except for numbers which are right-aligned
by default, e.g. `'x{:4}x'.format 1.2` will output `x 1.2x`.

The alignment modifier can be prefixed with a character which will be used to
fill any empty space in the formatted string (the default character being ` `).
e.g. `'{:x^8}'.format 1234` will output `xx1234xx`.

##### Maximum width / Precision

A maximum width can be specified following a `.` character,
e.g. `'{:.2}'.format abcd'` will output `ab`.

For numbers this will define the number of decimal places that should be
displayed.

Combining a maximum width with a minimum width is allowed, with the minimum
coming before the maximum in the format string,
e.g. `'x{:4.2}x'.format 'abcd'` will output `xab  x`.

### Example

```koto
print! '{}, {}!'.format 'Hello', 'World'
check! Hello, World!

print! '{0}-{1}-{0}'.format 99, 'xxx'
check! 99-xxx-99

print! '{foo} {bar}'.format {foo: 42, bar: true}
check! 42 true

print! '{:.2}'.format 1/3
check! 0.33

print! '{:-^8.2}'.format 2/3
check! --0.67--

print! 'foo = {foo:8.3}'.format {foo: 42}
check! foo =   42.000
```

## is_empty

```kototype
|String| -> Bool
```

Returns `true` if the string contains no characters.

### Example

```koto
print! 'abcdef'.is_empty()
check! false

print! ''.is_empty()
check! true
```

## from_bytes

```kototype
|Iterable| -> String
```

Returns a string containing the bytes that are produced by the input iterable.
The iterable output must contain only Numbers in the `0..=255` range.
The resulting sequence of bytes must contain UTF-8 data.

### Example

```koto
print! string.from_bytes (72, 195, 171, 121, 33)
check! Hëy!
```

### See Also

- [`string.bytes`](#bytes)

## lines

```kototype
|String| -> Iterator
```

Returns an iterator that yields the lines contained in the input string.

### Note

Lines end with either `\r\n` or `\n`.

### Example

```koto
print! 'foo\nbar\nbaz'.lines().to_tuple()
check! ('foo', 'bar', 'baz')

print! '\n\n\n'.lines().to_tuple()
check! ('', '', '')
```

## replace

```kototype
|String, String, String| -> String
```

Returns a copy of the input string with all occurrences of the match string
replaced with an alternative string.

### Example

```koto
print! '10101'.replace '0', 'x'
check! 1x1x1
```

## split

```kototype
|String, String| -> Iterator
```

Returns an iterator that yields strings resulting from splitting the first
string wherever the second string is encountered.

```kototype
|String, |String| -> Bool| -> Iterator
```

Returns an iterator that yields strings resulting from splitting the input
string based on the result of calling a function. The function will be called
for each grapheme in the input string, and splits will occur when the function
returns true.

### Example

```koto
print! 'a,b,c'.split(',').to_tuple()
check! ('a', 'b', 'c')

print! 'O_O'.split('O').to_tuple()
check! ('', '_', '')

print! 'x!y?z'.split(|c| c == '!' or c == '?').to_tuple()
check! ('x', 'y', 'z')
```

## starts_with

```kototype
|String, String| -> Bool
```

Returns `true` if the first string starts with the second string.

### Example

```koto
print! 'abcdef'.starts_with 'abc'
check! true

print! 'xyz'.starts_with 'abc'
check! false
```

## to_lowercase

```kototype
|String| -> String
```

Returns a lowercase version of the input string.

### Example

```koto
print! 'HÉLLÖ'.to_lowercase()
check! héllö

print! 'O_o'.to_lowercase()
check! o_o
```

## to_number

```kototype
|String| -> Number
```

Returns the string converted into a number.
- `0x`, `0o`, and `0b` prefixes will cause the parsing to treat the input as
  containing a hexadecimal, octal, or binary number respectively.
- Otherwise the number is assumed to be base 10, and the presence of a decimal
  point will produce a float instead of an integer.

If a number can't be produced then `Null` is returned.

```kototype
|String, Integer| -> Integer
```

Returns the string converted into an integer given the specified base.

The base must be in the range `2..=36`, otherwise an error will be thrown.

If the string contains non-numerical digits then `Null` is returned.

### Example

```koto
print! '123'.to_number()
check! 123

print! '-8.9'.to_number()
check! -8.9

print! '0x7f'.to_number()
check! 127

print! '0b10101'.to_number()
check! 21

print! '2N9C'.to_number(36)
check! 123456
```

## to_uppercase

```kototype
|String| -> String
```

Returns an uppercase version of the input string.

### Example

```koto
print! 'héllö'.to_uppercase()
check! HÉLLÖ

print! 'O_o'.to_uppercase()
check! O_O
```

## trim

```kototype
|String| -> String
```

Returns the string with whitespace at the start and end of the string trimmed.

### Example

```koto
print! '   x    '.trim()
check! x

print! '     >'.trim()
check! >
```
