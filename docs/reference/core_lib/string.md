# string

Koto's strings are immutable sequences of characters with UTF-8 encoding.

Functions that produce sub-strings (e.g. `string.trim`) share the string
data between the original string and the sub-string.

Strings support indexing operations, with string indices referring to
grapheme clusters.

## Example

```koto
a = "Hello"
b = "World!"
x = "{}, {}!".print a, b
# Hello, World!
"👋🥳😆"[1]
# 🥳
```

# Reference

- [chars](#chars)
- [contains](#contains)
- [ends_with](#ends_with)
- [escape](#escape)
- [format](#format)
- [is_empty](#is_empty)
- [lines](#lines)
- [print](#print)
- [size](#size)
- [slice](#slice)
- [split](#split)
- [starts_with](#starts_with)
- [to_lowercase](#to_lowercase)
- [to_number](#to_number)
- [to_uppercase](#to_uppercase)
- [trim](#trim)

## chars

`|String| -> Iterator`

Returns an iterator that yields the string's characters as strings.

### Note

A 'character' in Koto is defined as a grapheme, so `.chars()` iterates over the
string's grapheme clusters.

### Example

```koto
"Héllø! 👋".chars().to_tuple()
# ("H", "é", "l", "l", "ø", "!", " ", "👋")
```

## contains

`|String, String| -> Bool`

Returns `true` if the second provided string is a sub-string of the first.

### Example

```koto
"xyz".contains "abc"
# false

"xyz".contains "yz"
# true

"xyz".contains "xyz"
# true

"xyz".contains ""
# true
```

## ends_with

`|String, String| -> Bool`

Returns `true` if the first string ends with the second string.

### Example

```koto
"abcdef".ends_with "def"
# true

"xyz".ends_with "abc"
# false
```

## escape

`|String| -> String`

Returns the string with characters replaced with escape codes.

For example, newlines get replaced with `\n`, tabs get replaced with `\t`.

### Example

```koto
"
".escape()
# "\n"
```

## format

`|String, Value...| -> String`

Returns a formatted string, with the arguments being assigned to
`{}` placeholders in the format string.

### Formatting Syntax

- `{}`
  - Takes the next value.
  - Subsequent `{}` placeholders will take following values.
- `{0}, {1}, {2}, ...`
  - Takes the value at the specified index.
- `{x}, {name}, {id}`
  - Takes values by name from a map.
    - The map is expected to be the first argument after the format string.

### Example

```koto
"{}, {}!".format "Hello", "World"
# "Hello, World!"

"{0}-{1}-{0}".format 99, "xxx"
# "99-xxx-99

"{foo} {bar}".format {foo: 42, bar: true}
# "42 true"
```

## is_empty

`|String| -> Bool`

Returns `true` if the string contains no characters.

### Example

```koto
"abcdef".is_empty()
# false

"".is_empty()
# true
```

## lines

`|String| -> Iterator`

Returns an iterator that yields the lines contained in the input string.

### Note

Lines end with either `\r\n` or `\n`.

### Example

```koto
"foo\nbar\nbaz".lines().to_tuple()
# ("foo", "bar", "baz")

"\n\n\n".lines().to_tuple()
# ("", "", "")
```

## print

`|String, Value...| -> ()`

Prints a formatted string to the active logger,
by default this is the standard output.

### Note

See `string.format` for the formatting syntax.

## size

`|String| -> Number`

Returns the number of graphemes in the string.

### Note

Equivalent to calling `.chars().count()`.

### Example

```koto
"".size()
# 0

"abcdef".size()
# 6

"🥳👋😁".size()
# 3
```

## slice

`|String, Number| -> String`

Returns a string with the contents of the input string starting from the
provided character index.

`|String, Number, Number| -> String`

Returns the sub-string of the input string,
starting at the first index and ending at the second number.

### Note

Invalid start indices return Empty.

### Example

```koto
"abcdef".slice 3
# "def"

"abcdef".slice 2, 4
# "cd"

"abcdef".slice 100, 110
# ()
```

## split

`|String, String| -> Iterator`

Returns an iterator that yields strings resulting from splitting the first
string wherever the second string is encountered.

### Example

```koto
"a,b,c".split(",").to_tuple()
# ("a", "b", "c")

"O_O".split("O").to_tuple()
# ("", "_", "")
```

## starts_with

`|String, String| -> Bool`

Returns `true` if the first string starts with the second string.

### Example

```koto
"abcdef".starts_with "abc"
# true

"xyz".starts_with "abc"
# false
```

## to_lowercase

`|String| -> String`

Returns a lowercase version of the input string.

### Example

```koto
"HÉLLÖ".to_lowercase()
# "héllö"

"O_o".to_lowercase()
# o_o
```

## to_number

`|String| -> Number`

Returns the string parsed as a number.

### Example

```koto
"123".to_number()
# 123

"-8.9".to_number()
# -8.9
```

## to_uppercase

`|String| -> String`

Returns an uppercase version of the input string.

### Example

```koto
"héllö".to_uppercase()
# "HÉLLÖ"

"O_o".to_uppercase()
# O_O
```

## trim

`|String| -> String`

Returns the string with whitespace at the start and end of the string trimmed.

### Example

```koto
"   x    ".trim()
# "x"

">    ".trim()
# >
```
