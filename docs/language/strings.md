# Strings

Strings can be declared using `'` or `"` quotes. 

```koto
print! 'Hello, World!'
check! Hello, World!

print! "Welcome to Koto 👋"
check! Welcome to Koto 👋
```

Strings can start on one line and finish on another.

```koto
print! 'This is a string
that spans
several lines.'
check! This is a string
check! that spans
check! several lines.
```

Strings can be joined together with the `+` operator.

```koto
print! 'a' + 'Bc' + 'Def'
check! aBcDef
```

Individual elements of a string can be accessed via indexing with `[]` braces.

```koto
print! 'abcdef'[3]
check! d
print! '👋🥳😆'[0]
check! 👋
print! 'xyz'[1..]
check! yz
```

## String Interpolation

Variables can be easily included in a string by prefixing them with `$`.

```koto
xyz = 123
print! 'The value of xyz is $xyz'
check! The value of xyz is 123
```

Including variables in a string this way is known as _string interpolation_.

Expressions can be evaluated directly in an interpolated string by surrounding
the expression with `${}`.

```koto
print! '2 plus 3 is ${2 + 3}.'
check! 2 plus 3 is 5.
```

## String Escape Codes

Strings can contain the following escape codes to define special characters,
all of which start with a `\`. 

- `\n`: Newline
- `\r`: Carriage Return
- `\t`: Tab
- `\'`: Single quote
- `\"`: Double quote
- `\\`: Backslash
- `\$`: Dollar
- `\u{NNNNNN}`: Unicode character
  - Up to 6 hexadecimal digits can be included within the `{}` braces.
    The maximum value is `\u{10ffff}`.
- `\xNN`: ASCII character
  - Exactly 2 hexadecimal digits follow the `\x`.

```koto
print! '\$\'\"'
check! $'"
print! 'Hi \u{1F44B}'
check! Hi 👋
```

## Single or Double Quotes

Both single `'` and double `"` quotes are valid for defining strings in Koto
and can be used interchangeably.

A practical reason to choose one over the other is that the alternate
quote type can be used in a string without needing to use escape characters.

```koto
print 'This string has to escape its \'single quotes\'.'
check! This string has to escape its 'single quotes'.

print "This string contains unescaped 'single quotes'."
check! This string contains unescaped 'single quotes'.
```

## Raw Strings

When a string contains a lot of special characters, it can be preferable to use
a _raw string_. 

Raw strings ignore escape characters and interpolated expressions, 
providing the raw contents of the string between its _delimiters_.

Raw strings use single or double quotes as the delimiter, prefixed with an `r`.

```koto
print r'This string contains special characters: $foo\n\t.'
check! This string contains special characters: $foo\n\t.
```

For more complex string contents, the delimiter can be extended using up to 255 
`#` characters after the `r` prefix,

```koto
print r#'This string contains "both" 'quote' types.'#
check! This string contains "both" 'quote' types.

print r##'This string also includes a '#' symbol.'##
check! This string also includes a '#' symbol.
```
