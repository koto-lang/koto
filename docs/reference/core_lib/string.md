# string

Koto's strings are immutable sequences of characters with UTF-8 encoding.

String literals can be created with either double or single quotation marks.
Both styles are offered as a convenience to reduce the need for escaping,
e.g. `'a "b" c'` is equivalent to `"a \"b\" c"`,
and `"a 'b' c"` is equivalent to `'a \'b\' c'`

Functions that produce sub-strings (e.g. `string.trim`) share the string
data between the original string and the sub-string.

Strings support indexing operations, with string indices referring to
grapheme clusters.

## Escape codes

Strings can contain the following escape codes to define special characters,
all of which start with a `\`. To avoid having an escape sequence acting as an
escape code, then it can be escaped with an additional `\`.

- `\n`: Newline
- `\r`: Carriage Return
- `\t`: Tab
- `\u{NNNNNN}`: Unicode character
  - Up to 6 hexadecimal digits can be included within the `{}` braces.
    The maximum value is `\u{10ffff}`.
- `\xNN`: ASCII character
  - Exactly 2 hexadecimal digits follow the `\x`.
- `\'`: Single quote
- `\"`: Double quote
- `\\`: Backslash

## Example

```koto
a = "Hello"
b = 'World!'
x = "{}, {}!".format a, b
# Hello, World!
'👋🥳😆'[1]
# 🥳
```

# Reference

- [bytes](#bytes)
- [chars](#chars)
- [contains](#contains)
- [ends_with](#ends_with)
- [escape](#escape)
- [format](#format)
- [is_empty](#is_empty)
- [lines](#lines)
- [size](#size)
- [slice](#slice)
- [split](#split)
- [starts_with](#starts_with)
- [to_lowercase](#to_lowercase)
- [to_number](#to_number)
- [to_uppercase](#to_uppercase)
- [trim](#trim)

## bytes

`|String| -> Iterator`

Returns an iterator that yields a series of Numbers representing the bytes
contained in the string data.

### Example

```koto
"Hëy".bytes().to_tuple()
# (72, 195, 171, 121)
```

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
another `{`, e.g. `"{{}}".format()` will output `"{}"`.

#### Formatting modifiers

Modifiers can be provided after a `:` separator in the format string.

##### Minimum width, fill, and alignment

A minimum width can be specified, ensuring that the formatted value takes up at
least that many characters, e.g. `"x{:4}x".format "ab"` will output `xab  x`.

The minimum width can be prefixed with an alignment modifier:

- `<` - left-aligned
- `^` - centered
- `>` - right-aligned

e.g. `"x{:>4}x".format "ab"` will output `x  abx`.

Values are left-aligned by default, except for numbers which are right-aligned
by default, e.g. `"x{:4}x".format 1.2` will output `x 1.2x`.

The alignment modifier can be prefixed with a character which will be used to
fill any empty space in the formatted string (the default character being ` `).
e.g. `"{:x^8}".format 1234` will output `xx1234xx`.

##### Maximum width / Precision

A maximum width can be specified following a `.` character,
e.g. `"{:.2}".format abcd"` will output `ab`.

For numbers this will define the number of decimal places that should be
displayed.

Combining a maximum width with a minimum width is allowed, with the minimum
coming before the maximum in the format string,
e.g. `"x{:4.2}x".format "abcd"` will output `xab  x`.

### Example

```koto
"{}, {}!".format "Hello", "World"
# "Hello, World!"

"{0}-{1}-{0}".format 99, "xxx"
# "99-xxx-99

"{foo} {bar}".format {foo: 42, bar: true}
# "42 true"

"{:.2}".format 1/3
# 0.33

"{:-^8.2}".format 2/3
# --0.67--

"foo = {foo:8.3}".format {foo: 42}
# foo =   42.000
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

`|String, |String| -> Bool| -> Iterator`

Returns an iterator that yields strings resulting from splitting the input
string based on the result of calling a function. The function will be called
for each grapheme in the input string, and splits will occur when the function
returns true.

### Example

```koto
"a,b,c".split(",").to_tuple()
# ("a", "b", "c")

"O_O".split("O").to_tuple()
# ("", "_", "")

"x!y?z".split(|c| c == "!" or c == "?").to_tuple()
# ("x", "y", "z")
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
