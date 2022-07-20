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

Individual elements of a String can be accessed via indexing with `[]` braces.

```koto
print! 'abcdef'[3]
check! d
print! '👋🥳😆'[1]
check! 🥳
```

## String Interpolation

Assigned values can be included in a String by prefixing them with `$`.

```koto
xyz = 123
print! 'The value of xyz is $xyz'
check! The value of xyz is 123
```

The `$` prefix can also be used to include the results of expressions surrounded with `{}` curly braces.

```koto
print! '2 plus 3 is ${2 + 3}.'
check! 2 plus 3 is 5.
```

## String Escape codes

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

## Single or double quotes

Whether you use `'` or `"` for your strings doesn't make a difference, except that you can use the other quote character freely in the string without having to escape it with `\`.

```koto
print "This string contains 'single quotes'."
check! This string contains 'single quotes'.

print 'This string has to escape its \'single quotes\'.'
check! This string has to escape its 'single quotes'.
```

