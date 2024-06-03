# string

## bytes

```kototype
|String| -> Iterator
```

Returns an iterator that yields a series of integers representing the bytes
contained in the string data.

### Example

```koto
print! 'Hëy!'.bytes().to_tuple()
check! (72, 195, 171, 121, 33)
```

### See Also

- [`string.from_bytes`](#from_bytes)

## chars

```kototype
|String| -> Iterator
```

Returns an iterator that yields the string's characters as strings.

A 'character' in Koto is defined as being a 
[unicode grapheme cluster][grapheme-cluster].

### Note

Note that this is the default iteration behaviour for a string, so calling
`'hello'.chars()` is equivalent to calling `iterator.iter('hello')`.

### Example

```koto
print! 'Héllø! 👋'.chars().to_tuple()
check! ('H', 'é', 'l', 'l', 'ø', '!', ' ', '👋')
```

### See Also

- [`string.char_indices`](#char_indices)

## char_indices

```kototype
|String| -> Iterator
```

Returns an iterator that yields the indices of each 
[grapheme cluster][grapheme-cluster] in the string.

Each cluster is represented as a range, which can then be used to extract the
cluster from the string via indexing.

### Example

```koto
s = 'Hi 👋'

print! indices = s.char_indices().to_tuple()
check! (0..1, 1..2, 2..3, 3..7)

print! s[indices[3]]
check! 👋
```

### See Also

- [`string.chars`](#chars)

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
|String, match: String, replacement: String| -> String
```

Returns a copy of the input string with all occurrences of the `match` string
replaced with a `replacement` string.

### Example

```koto
print! '10101'.replace '0', 'x'
check! 1x1x1
```

## split

```kototype
|String, match: String| -> Iterator
```

Returns an iterator that yields strings resulting from splitting the first
string wherever the `match` string is encountered.

```kototype
|String, match: |String| -> Bool| -> Iterator
```

Returns an iterator that yields strings resulting from splitting the input
string based on the result of calling a `match` function. 

The `match` function will be called for each grapheme in the input string, and
splits will occur when the function returns true.

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
|String, match: String| -> Bool
```

Returns `true` if the first string starts with the `match` string.

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
|String, base: Number| -> Number
```

Returns the string converted into a number given the specified `base`.

The base must be an integer in the range `2..=36`, 
otherwise an error will be thrown.

If the string contains non-numerical digits then `null` is returned.

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

print! '     !'.trim()
check! !
```

[grapheme-cluster]: https://www.unicode.org/glossary/#grapheme_cluster
