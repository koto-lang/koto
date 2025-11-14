A rendered version of this document can be found
[here](https://koto.dev/docs/next/language).

See the neighboring [readme](./README.md) for an explanation of the
`print!` and `check!` commands used in the following example.

---

# The Koto Language Guide

## Language Basics

Koto programs contain a series of expressions that are evaluated in
top-to-bottom order by Koto's runtime.

As an example, this simple script prints a friendly greeting.

```koto,skip_run
name = 'World'
print 'Hello, {name}!'
```

### Comments

Single-line comments start with a `#`.

```koto
# This is a comment, everything until the end of the line is ignored.
```

Multi-line comments start with `#-` and end with `-#`.

```koto
#-
This is a
multi-line
comment.
-#
```

### Numbers and Arithmetic

Numbers and arithmetic are expressed in a familiar way.

```koto
print! 1
check! 1

# Addition
print! 1 + 1
check! 2

# Negation and Subtraction
print! -1 - 10
check! -11

# Multiplication
print! 3 * 4
check! 12

# Division
print! 9 / 2
check! 4.5

# Remainder
print! 12.5 % 5
check! 2.5

# Power / Exponentiation
print! 2 ^ 3
check! 8
```

Underscores can be used as separators to aid readability in long numbers.

```koto
print! 1_000_000
check! 1000000
```

#### Parentheses

Arithmetic operations follow the
[conventional order of precedence][operation-order].
Parentheses can be used to group expressions as needed.

```koto
# Without parentheses, multiplication is performed before addition
print! 1 + 2 * 3 + 4
check! 11
# With parentheses, the additions are performed first
print! (1 + 2) * (3 + 4)
check! 21
```

#### Non-decimal Numbers

Numbers can be expressed with non-decimal bases.

```koto
# Hexadecimal numbers begin with 0x
print! 0xcafe
check! 51966

# Octal numbers begin with 0o
print! 0o7060
check! 3632

# Binary numbers begin with 0b
print! 0b1001
check! 9
```

### Booleans

Booleans are declared with the `true` and `false` keywords, and combined using
the `and` and `or` operators.

```koto
print! true and false
check! false

print! true or false
check! true
```

Booleans can be negated with the `not` operator.

```koto
print! not true
check! false

print! not false
check! true
```

Values can be compared for equality with the `==` and `!=` operators.

```koto
print! 1 + 1 == 2
check! true

print! 99 != 100
check! true
```

### Null

The `null` keyword is used to declare a value of type `Null`,
which indicates the absence of a value.

```koto
print! null
check! null
```

#### Truthiness

In boolean contexts (such as logical operations), `null` is treated as being
equivalent to `false`. Every other value in Koto evaluates as `true`.

```koto
print! not null
check! true

print! null or 42
check! 42
```

### Assigning Variables

Values are assigned to named identifiers with `=`, and can be freely reassigned.
Named values like this are known as _variables_.

```koto
# Assign the value `42` to `x`
x = 42
print! x
check! 42

# Replace the existing value of `x`
x = true
print! x
check! true
```

The result of an assignment is the value that's being assigned, so chained assignments are possible.

```koto
print! x = 1
check! 1

print! a = b = 100
check! 100
print! a + b
check! 200
```

[Compound assignment][compound-assignment] operators are also available.
For example, `x *= y` is a simpler way of writing `x = x * y`.

```koto
a = 100
print! a += 11
check! 111
print! a
check! 111

print! a *= 10
check! 1110
print! a
check! 1110
```

### Debug

The `debug` keyword allows you to quickly display a value while working on a
program.

It prints the result of an expression, prefixed with its line number and the
original expression as a string.

```koto
x = 10 + 20
debug x / 10
check! [2] x / 10: 3.0
```

When using `debug`, the displayed value is also the result of the expression,
which can be useful if you want to quickly get feedback during development.

```koto
x = debug 2 + 2
check! [1] 2 + 2: 4
print! x
check! 4
```

### Semicolons

Expressions are typically placed on separate lines,
but if necessary they can be separated with semicolons.

```koto
a = 1; b = 2; c = a + b
print! c
check! 3
```

## Lists

Lists in Koto are created with `[]` square brackets and can contain a mix of
different value types.

Access list elements by _index_ using square brackets, starting from `0`.

```koto
x = [99, null, true]
print! x[0]
check! 99
print! x[1]
check! null

x[2] = false
print! x[2]
check! false
```

Once a list has been created, its underlying data is shared between other
instances of the same list.
Changes to one instance of the list are reflected in the other.

```koto
# Assign a list to x
x = [10, 20, 30]

# Assign another instance of the list to y
y = x

# Modify the list through y
y[1] = 99

# The change to y is also reflected in x
print! x
check! [10, 99, 30]
```

If no value is given between commas then `null` is added to the list at that position.

```koto
print! [10, , 30, , 50]
check! [10, null, 30, null, 50]
```

### Joining Lists

The `+` operator allows lists to be joined together, creating a new list that
contains their concatenated elements.

```koto
a = [98, 99, 100]
b = a + [1, 2, 3]
print! b
check! [98, 99, 100, 1, 2, 3]
```

## Tuples

Tuples in Koto are similar to lists,
but are designed for sequences of values that have a fixed structure.

Unlike lists, tuples can't be resized after creation,
and values that are contained in the tuple can't be replaced.

Tuples are declared with a series of expressions separated by commas.

```koto
x = 100, true, -1
print! x
check! (100, true, -1)
```

Parentheses can be used for grouping to avoid ambiguity.

```koto
print! (1, 2, 3), (4, 5, 6)
check! ((1, 2, 3), (4, 5, 6))
```

You can access tuple elements by index using square brackets, starting from `0`.

```koto
print! x = false, 10
check! (false, 10)
print! x[0]
check! false
print! x[1]
check! 10

print! y = true, 20
check! (true, 20)
print! x, y
check! ((false, 10), (true, 20))
```

If no value is given between commas then `null` is added to the tuple at that position.

```koto
x = 10, , 20, , 30
print! x
check! (10, null, 20, null, 30)
```

### Empty and Single Element Tuples

An empty tuple (a tuple that contains zero elements) is created using empty parentheses.

```koto
x = ()
print! x
check! ()
```

A tuple that contains a single element can be created by including a trailing comma.

```koto
# An expression inside parentheses simply resolves to the result of the expression
print! (1 + 2)
check! 3

# To place the result of the expression in a tuple, use a trailing comma
print! (1 + 2,)
check! (3)

# Single element tuples can also be created without parentheses
x = 1 + 2,
print! x
check! (3)
```


### Joining Tuples

The `+` operator allows tuples to be joined together,
creating a new tuple containing their concatenated elements.

```koto
a = 1, 2, 3
b = a + (4, 5, 6)
print! b
check! (1, 2, 3, 4, 5, 6)
```

### Tuple Mutability

While tuples have a fixed structure and its contained values can't be
replaced, [_mutable_][immutable] value types (like [lists](#lists)) can be
modified while they're contained in tuples.

```koto
# A Tuple containing two lists
x = ([1, 2, 3], [4, 5, 6])

# Modify the second list in the tuple
x[1][0] = 99
print! x
check! ([1, 2, 3], [99, 5, 6])
```


## Strings

Strings in Koto contain a sequence of [UTF-8][utf-8] encoded characters,
and can be declared using `'` or `"` quotes.

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

### String Interpolation

Variables can be easily included in a string by surrounding them with `{}` curly
braces.

```koto
xyz = 123
print! 'The value of xyz is {xyz}'
check! The value of xyz is 123
```

Including variables in a string this way is known as _string interpolation_.

Simple expressions can also be interpolated using the same syntax.

```koto
print! '2 plus 3 is {2 + 3}.'
check! 2 plus 3 is 5.
```

### String Escape Codes

Strings can contain the following escape codes to define special characters,
all of which start with a `\`.

- `\n`: Newline
- `\r`: Carriage Return
- `\t`: Tab
- `\'`: Single quote
- `\"`: Double quote
- `\\`: Backslash
- `\{`: Interpolation start
- `\u{NNNNNN}`: Unicode character
  - Up to 6 hexadecimal digits can be included within the `{}` braces.
    The maximum value is `\u{10ffff}`.
- `\xNN`: ASCII character
  - Exactly 2 hexadecimal digits follow the `\x`.

```koto
print! '\{\'\"}'
check! {'"}
print! 'Hi \u{1F44B}'
check! Hi 👋
```

### Continuing a Long Line

The end of a line can be escaped with a `\`, which will skip the
newline and any leading whitespace on the next line.

```koto
foo = "This string \
       doesn't contain \
       newlines."
print! foo
check! This string doesn't contain newlines.
```

### String Indexing

Individual _bytes_ of a string can be accessed via indexing with `[]` braces.

```koto
print! 'abcdef'[3]
check! d
print! 'xyz'[1..]
check! yz
```

Care must be taken when using indexing with strings that could contain
non-[ASCII][ascii] data.
If the indexed bytes would produce invalid UTF-8 data then an
error will be thrown.
To access a string's Unicode characters individually, see [`string.chars`][chars].

### Single or Double Quotes

Both single `'` and double `"` quotes are valid for defining strings in Koto
and have the same meaning.

A practical reason to choose one over the other is that the alternate
quote type can be used in a string without needing to use escape characters.

```koto
print 'This string has to escape its \'single quotes\'.'
check! This string has to escape its 'single quotes'.

print "This string contains unescaped 'single quotes'."
check! This string contains unescaped 'single quotes'.
```

### Raw Strings

When a string contains a lot of special characters, it can be preferable to use
a _raw string_.

Raw strings ignore escape characters and interpolated expressions,
providing the raw contents of the string between its _delimiters_.

Raw strings use single or double quotes as the delimiter, prefixed with an `r`.

```koto
print r'This string contains special characters: {foo}\n\t.'
check! This string contains special characters: {foo}\n\t.
```

For more complex string contents, the delimiter can be extended using up to 255
`#` characters after the `r` prefix,

```koto
print r#'This string contains "both" 'quote' types.'#
check! This string contains "both" 'quote' types.

print r##'This string also includes a '#' symbol.'##
check! This string also includes a '#' symbol.
```

## Functions

Functions in Koto are created using a pair of vertical bars (`||`),
with the function's _arguments_ listed between the bars.
The _body_ of the function follows the vertical bars.

```koto
hi = || 'Hello!'
add = |x, y| x + y
```

Functions are _called_ with arguments contained in `()` parentheses.
The body of the function is evaluated and the result is returned to the caller.

```koto
hi = || 'Hello!'
print! hi()
check! Hello!

add = |x, y| x + y
print! add(50, 5)
check! 55
```

A function's body can be an indented block, where the last
expression in the body is evaluated as the function's result.

```koto
f = |x, y, z|
  x *= 100
  y *= 10
  x + y + z
print! f(2, 3, 4)
check! 234
```

### Optional Call Parentheses

In simple expressions, the parentheses for function call arguments are optional and can be omitted.

```koto
square = |x| x * x
print! square 8
check! 64

add = |x, y| x + y
print! add 2, 3
check! 5

print! square add 2, 3 # Equivalent to square(add(2, 3))
check! 25

first = |x| x[0]
print! first ('a', 'b') # Equivalent to first(('a', 'b'))
check! a
```

### Return

When the function should be exited early, the `return` keyword can be used.

```koto
f = |n|
  return 42
  # This expression won't be reached
  n * n
print! f -1
check! 42
```

If a value isn't provided to `return`, then the returned value is `null`.

```koto
f = |n|
  return
  n * n
print! f 123
check! null
```

### Function Piping

The arrow operator (`->`) can be used to pass the result of one function to
another, working from left to right. This is known as _function piping_,
and can aid readability when working with a long chain of function calls.

```koto
add = |x, y| x + y
multiply = |x, y| x * y
square = |x| x * x

# Chained function calls can be a bit hard to follow for the reader.
print! x = multiply 2, square add 1, 3
check! 32

# Parentheses don't help all that much...
print! x = multiply(2, square(add(1, 3)))
check! 32

# Piping allows for a left-to-right flow of results.
print! x = add(1, 3) -> square -> multiply 2
check! 32

# Call chains can also be broken across lines.
print! x = add 1, 3
  -> square
  -> multiply 2
check! 32
```

Piped arguments are inserted as the first argument for the following call.

```koto
get_name = || 'Ada'
say = |name, greeting| print '{greeting}, {name}!'

get_name() -> say 'Hello'
check! Hello, Ada!
```

## Maps

Maps in Koto are [associative containers][associated] that contain a series of
_entries_ with _keys_ that correspond to associated values.

Maps can be created using _inline syntax_, with `{}` braces containing a series of entries separated by commas.

The `.` operator returns the value associated with a particular key.


```koto
m = {apples: 42, oranges: 99, lemons: 63}

# Get the value associated with the `oranges` key
print! m.oranges
check! 99
```

Maps can also be created using _block syntax_, with each entry on a new indented line:

```koto
m =
  apples: 42
  oranges: 99
  lemons: 63
print! m.apples
check! 42
```

Once a map has been created, its underlying data is shared between other
instances of the same map. Changes to one instance are reflected in the other.

```koto
# Create a map and assign it to `a`.
a = {foo: 99}
print! a.foo
check! 99

# Assign a new instance of the map to `z`.
z = a

# Modifying the data via `z` is reflected in `a`.
z.foo = 'Hi!'
print! a.foo
check! Hi!
```

### Entry Order

A map's entries are maintained in a consistent order,
representing the sequence in which its entries were added.

You can access map entries by index using square brackets, starting from `0`.

The entry is returned as a tuple containing the key and its associated value.

```koto
m = {apples: 42, oranges: 99, lemons: 63}
print! m[1]
check! ('oranges', 99)
```

Entries can also be replaced by assigning a key/value tuple to the entry's index.

```koto
m = {apples: 42, oranges: 99, lemons: 63}
m[1] = ('pears', 123)
print! m
check! {apples: 42, pears: 123, lemons: 63}
```

### Shorthand Values

When creating maps with inline syntax, Koto supports a shorthand notation that
simplifies adding existing values to the map.

If a value isn't provided for a key, then Koto will look for a value
that matches the key's name, and if one is found then it will be used as that
entry's value.

```koto
hi, bye = 'hi!', 'bye!'
print! m = {hi, x: 42, bye}
check! {hi: 'hi!', x: 42, bye: 'bye!'}
```

### Maps and Self

Maps can store any type of value, including functions,
which provides a convenient way to group functions together.

```koto
m =
  hello: |name| 'Hello, {name}!'
  bye: |name| 'Bye, {name}!'

print! m.hello 'World'
check! Hello, World!
print! m.bye 'Friend'
check! Bye, Friend!
```

`self` is a special identifier that refers to the instance of the container in
which the function is contained.

In maps, `self` allows functions to access and modify data from the map,
enabling [_object_][object-wiki]-like behaviour.

```koto
m =
  name: 'World'
  say_hello: || 'Hello, {self.name}!'

print! m.say_hello()
check! Hello, World!

m.name = 'Friend'
print! m.say_hello()
check! Hello, Friend!
```

### Joining Maps

The `+` operator allows maps to be joined together, creating a new map that
combines their entries.

```koto
a = {red: 100, blue: 150}
b = {green: 200, blue: 99}
c = a + b
print! c
check! {red: 100, blue: 99, green: 200}
```

### Quoted Map Keys

Map keys are usually defined and accessed without quotes, but they are stored in
the map as strings. Quotes can be used if a key needs to be defined that would be
otherwise be disallowed by Koto syntax rules
(e.g. a keyword, or using characters that aren't allowed in an identifier).
Quoted keys also allow key names to be generated dynamically by using string
interpolation.

```koto
x = 99
m =
  'true': 42
  'key{x}': x
print! m.'true'
check! 42
print! m.key99
check! 99
```

### Map Key Types

Map keys are typically strings, but any [_immutable_][immutable] value can be
used as a map key by using the [`map.insert`][map-insert] and [`map.get`][map-get]
functions.

The immutable value types in Koto are [strings](#strings), [numbers](#numbers_and_arithmetic),
[booleans](#booleans), [ranges](#ranges), and [`null`](#null).
[Tuples](#tuples) are also considered to be immutable when their contained
elements are all immutable.

```koto
m = {}

m.insert 0, 'zero'
print! m.get 0
check! zero

m.insert (1, 2, 3), 'xxx'
print! m.get (1, 2, 3)
check! xxx
```


## Core Library

The [_Core Library_][core] provides a collection of fundamental functions
and values for working with the Koto language, organized within _modules_.

```koto
# Convert a string to lowercase
print! string.to_lowercase 'HELLO'
check! hello

# Get the first element of a list
print! list.first [99, -1, 3]
check! 99
```

Koto's built-in value types automatically have access to their corresponding
core library modules via `.` access.

```koto
# Convert a string to uppercase
print! 'xyz'.to_uppercase()
check! XYZ

# Get the last element in a list
print! ['abc', 123].last()
check! 123

# Round a floating-point number to the closest integer
print! (7 / 2).round()
check! 4

# Check if a map contains an 'apples' key
print! {apples: 42, pears: 99}.contains_key 'apples'
check! true
```

The [documentation][core] for the Core library (along with this guide) is
also available in the `help` command of the [Koto CLI][cli].

### Prelude

Koto's _prelude_ is a collection of items that are automatically
made available in a Koto script without the need for first calling [`import`](#import).

The core library's modules are all included by default in the prelude,
along with the following functions:

- [`io.print`](./core_lib/io.md#print)
- [`koto.copy`](./core_lib/koto.md#copy)
- [`koto.size`](./core_lib/koto.md#size)
- [`koto.type`](./core_lib/koto.md#type)
- [`test.assert`](./core_lib/test.md#assert)
- [`test.assert_eq`](./core_lib/test.md#assert_eq)
- [`test.assert_ne`](./core_lib/test.md#assert_ne)
- [`test.assert_near`](./core_lib/test.md#assert_near)

```koto
print 'io.print is available without needing to be imported'
check! io.print is available without needing to be imported
```

## Conditional Expressions

Koto includes several ways of producing values that depend on _conditions_.

### `if`

`if` expressions come in two flavors; single-line:

```koto
x = 99
if x % 2 == 0 then print 'even' else print 'odd'
check! odd
```

...And multi-line using indented blocks:

```koto
x = 24
if x < 0
  print 'negative'
else if x > 24
  print 'no way!'
else
  print 'ok'
check! ok
```

The result of an `if` expression is the final expression in the branch that gets
executed.

```koto
x = if 1 + 1 == 2 then 3 else -1
print! x
check! 3

# Assign the result of the if expression to foo
foo = if x > 0
  y = x * 10
  y + 3
else
  y = x * 100
  y * y

print! foo
check! 33
```

### `switch`

`switch` expressions can be used as a cleaner alternative to
`if`/`else if`/`else` cascades.

```koto
fib = |n|
  switch
    n <= 0 then 0
    n == 1 then 1
    else (fib n - 1) + (fib n - 2)

print! fib 7
check! 13
```

### `match`

`match` expressions can be used to match a value against a series of patterns,
with the matched pattern causing a specific branch of code to be executed.

Patterns can be literals or identifiers. An identifier will match any value,
so they're often used with `if` conditions to refine the match.

```koto
print! match 40 + 2
  0 then 'zero'
  1 then 'one'
  x if x < 10 then 'less than 10: {x}'
  x if x < 50 then 'less than 50: {x}'
  x then 'other: {x}'
check! less than 50: 42
```

Ignored values (any identifier starting with `_`) match against any value,
and `else` can be used for fallback branches.

```koto
fizz_buzz = |n|
  match n % 3, n % 5
    0, 0 then "Fizz Buzz"
    0, _ then "Fizz"
    _, 0 then "Buzz"
    else n

print! (10, 11, 12, 13, 14, 15)
  .each |n| fizz_buzz n
  .to_tuple()
check! ('Buzz', 11, 'Fizz', 13, 14, 'Fizz Buzz')
```

List and tuple entries can be matched against by using parentheses,
with `...` available for capturing the rest of the sequence.

```koto
print! match ['a', 'b', 'c'].extend [1, 2, 3]
  ('a', 'b') then
    "A list containing 'a' and 'b'"
  (1, ...) then
    "Starts with '1'"
  (..., 'y', last) then
    "Ends with 'y' followed by '{last}'"
  ('a', x, others...) then
    "Starts with 'a', followed by '{x}', then {size others} others"
  unmatched then "other: {unmatched}"
check! Starts with 'a', followed by 'b', then 4 others
```

### Optional Chaining

Checking optional values for `null` in expression chains can feel a bit
cumbersome, with `if` checks interrupting an expression's natural flow.

The `?` operator can be used to simplify expression chains that contain optional results.
If `?` finds `null` when checking an optional value,
then the chain gets _short-circuited_ with `null` given as the chain's overall result.

```koto
info = {town: 'Hamburg', country: 'Germany'}

# `info` contains a value for 'town', which is then passed to to_uppercase():
print! info.get('town')?.to_uppercase()
check! HAMBURG

# `info` doesn't contain a value for 'state',
# so the `?` operator short-circuits the expression, resulting in `null`:
print! info.get('state')?.to_uppercase()
check! null

# Without the `?` operator, an intermediate step is necessary:
country = info.get('country')
print! if country then country.to_uppercase()
check! GERMANY
```

Multiple `?` checks can be performed in an expression chain:

```koto
get_data = || {nested: {maybe_string: null}}
print! get_data()?
  .get('nested')?
  .get('maybe_string')?
  .to_uppercase()
check! null
```

## Loops

Koto includes several ways of evaluating expressions repeatedly in a loop.

### `for`

`for` loops are repeated for each element in a sequence,
such as a list or tuple.

```koto
for n in [10, 20, 30]
  print n
check! 10
check! 20
check! 30
```

### `while`

`while` loops continue to repeat _while_ a condition is true.

```koto
x = 0
while x < 5
  x += 1
print! x
check! 5
```

### `until`

`until` loops continue to repeat _until_ a condition is true.

```koto
z = [1, 2, 3]
until z.is_empty()
  # Remove the last element of the list
  print z.pop()
check! 3
check! 2
check! 1
```

### `continue`

The `continue` keyword skips the remaining part of a loop's body and proceeds with the next repetition of the loop.

```koto
for n in (-2, -1, 1, 2)
  # Skip over any values less than 0
  if n < 0
    continue
  print n
check! 1
check! 2
```

### `break`

Loops can be terminated with the `break` keyword.

```koto
x = 0
while x < 100000
  if x >= 3
    # Break out of the loop when x is greater or equal to 3
    break
  x += 1
print! x
check! 3
```

A value can be provided to `break`, which is then used as the result of the loop.

```koto
x = 0
y = while x < 100000
  if x >= 3
    # Break out of the loop, providing x + 100 as the loop's result
    break x + 100
  x += 1
print! y
check! 103
```

### `loop`

`loop` creates a loop that will repeat indefinitely.

```koto
x = 0
y = loop
  x += 1
  # Stop looping when x is greater than 4
  if x > 4
    break x * x
print! y
check! 25
```

## Iterators

The elements of a sequence can be accessed sequentially with an _iterator_,
created using the `.iter()` function.

An iterator yields values via [`.next()`][next] until the end of the sequence is
reached, when `null` is returned.

```koto
i = [10, 20].iter()

print! i.next()
check! IteratorOutput(10)
print! i.next()
check! IteratorOutput(20)
print! i.next()
check! null
```

### Iterator Generators

The [`iterator`][iterator] module contains iterator _generators_ like
[`once`][once] and [`repeat`][repeat] that generate output values
[_lazily_][lazy] during iteration.

```koto
# Create an iterator that repeats ! twice
i = iterator.repeat('!', 2)
print! i.next()
check! IteratorOutput(!)
print! i.next()
check! IteratorOutput(!)
print! i.next()
check! null
```

### Iterator Adaptors

The output of an iterator can be modified using _adaptors_ from the
[`iterator`][iterator] module.

The `iterator` module is available to any value which is declared to be _iterable_
(which includes Koto's containers like lists and strings),
so it's not necessary to call `.iter()` before using an adaptor.

```koto
# Create an iterator that outputs any value in the list above 3
x = [1, 2, 3, 4, 5].keep |n| n > 3

print! x.next()
check! IteratorOutput(4)
print! x.next()
check! IteratorOutput(5)
print! x.next()
check! null
```

### Using iterators with `for`

`for` loops accept any iterable value as input, including adapted iterators.

```koto
for x in 'abacad'.keep |c| c != 'a'
  print x
check! b
check! c
check! d
```

### Iterator Chains

Any iterator can be passed into an adaptor, including other adaptors,
creating _iterator chains_ that act as data processing pipelines.

```koto
i = (1, 2, 3, 4, 5)
  .skip 1
  .each |n| n * 10
  .keep |n| n <= 40
  .intersperse '--'

for x in i
  print x
check! 20
check! --
check! 30
check! --
check! 40
```

### Iterator Consumers

Iterators can also be _consumed_ using functions like
[`.to_list()`][to_list] and [`.to_tuple()`][to_tuple],
allowing the output of an iterator to be easily captured in a container.

```koto
print! [1, 2, 3]
  .each |n| n * 2
  .to_tuple()
check! (2, 4, 6)

print! (1, 2, 3, 4)
  .keep |n| n % 2 == 0
  .each |n| n * 11
  .to_list()
check! [22, 44]
```

Iterator consumers are also available for creating [strings][to_string] and [maps][to_map],
as well as operations like [counting the number of values][iterator-count] yielded from an
iterator, or getting the [total sum][iterator-sum] of an iterator's output.

## Value Unpacking

Multiple assignments can be performed in a single expression by separating the
variable names with commas.

```koto
a, b = 10, 20
print! a, b
check! (10, 20)
```

If there's a single value being assigned, and the value is iterable,
then it gets _unpacked_ into the target variables.

```koto
my_tuple = 1, 2
x, y = my_tuple
print! y, x
check! (2, 1)
```

Unpacking works with any iterable value, including adapted iterators.

```koto
a, b, c = [1, 2, 3, 4, 5]
print! a, b, c
check! (1, 2, 3)

x, y, z = 'a-b-c'.split '-'
print! x, y, z
check! ('a', 'b', 'c')
```

If the value being unpacked doesn't contain enough values for the assignment,
then `null` is assigned to any remaining variables.

```koto
a, b, c = [-1, -2]
print! a, b, c
check! (-1, -2, null)

x, y, z = 42
print! x, y, z
check! (42, null, null)
```

Unpacking can also be used in `for` loops, which is particularly useful when
looping over the contents of a map.

```koto
my_map = {foo: 42, bar: 99}
for key, value in my_map
  print key, value
check! ('foo', 42)
check! ('bar', 99)
```

### Ignoring Unpacked Values

`_` can be used as a placeholder for unpacked values that aren't needed elsewhere
in the code and can therefore be ignored.

If you would like to add a name to the ignored value as a reminder, then the name can be appended to `_`.

Ignored values (any variables starting with `_`) can be written to, but can't be accessed.

```koto
a, _, c = 10..20
print! a, c
check! (10, 12)

_first, second = 'xyz'
print! second
check! y
```

## Ranges

Ranges of integers can be created with `..` or `..=`.

`..` creates a _non-inclusive_ range,
which defines a range up to but _not including_ the end of the range.

```koto
# Create a range from 10 to 20, not including 20
print! r = 10..20
check! 10..20
print! r.start()
check! 10
print! r.end()
check! 20
print! r.contains 20
check! false
```

`..=` creates an _inclusive_ range, which includes the end of the range.

```koto
# Create a range from 10 to 20, including 20
print! r = 10..=20
check! 10..=20
print! r.contains 20
check! true
```

If a value is missing from either side of the range operator then an _unbounded_
range is created.

```koto
# Create an unbounded range starting from 10
r = 10..
print! r.start()
check! 10
print! r.end()
check! null

# Create an unbounded range up to and including 100
r = ..=100
print! r.start()
check! null
print! r.end()
check! 100
```

Ranges that have a defined start can be indexed using square brackets.

```koto
r = 100..200
print! r[50]
check! 150

r = 10..
print! r[100]
check! 110
```

_Bounded_ ranges are declared as iterable,
so they can be used in for loops and with the [`iterator`][iterator] module.

```koto
for x in 1..=3
  print x
check! 1
check! 2
check! 3

print! (0..5).to_list()
check! [0, 1, 2, 3, 4]
```

### Slices

Ranges can be used to create a _slice_ of a container's data.

```koto
x = (10, 20, 30, 40, 50)
print! x[1..=3]
check! (20, 30, 40)
```

For immutable containers like tuples and strings,
slices share the original value's data, with no copies being made.

For mutable containers like lists, creating a slice makes a copy of the sliced
portion of the underlying data.

```koto
x = 'abcdef'
# No copies are made when a string is sliced
print! y = x[3..6]
check! def

a = [1, 2, 3]
# When a list is sliced, the sliced elements get copied into a new list
print! b = a[0..2]
check! [1, 2]
print! b[0] = 42
check! 42
print! a[0]
check! 1
```

When creating a slice with an unbounded range,
if the start of the range if omitted then the slice starts from the beginning of the container.
If the end of the range is omitted, then the slice includes all remaining elements in the container.

```koto
z = 'Hëllø'.to_tuple()
print! z[..2]
check! ('H', 'ë')
print! z[2..]
check! ('l', 'l', 'ø')
```

## Type Checks

Koto is a primarily a dynamically typed language, however in more complex programs
you might find it beneficial to add _type checks_.

These checks can help in catching errors earlier, and can also act as
documentation for the reader.

One way to add type checks to your program is to use the
[`type`][koto-type] function, which returns a value's type as a string.

```koto
x = 123
assert_eq (type x), 'Number'
```

Checking types this way is rather verbose, so Koto offers _type hints_ as a more
ergonomic alternative.

### `let`

You can declare variables with type hints using a `let` expression.

If a value is assigned that doesn't match the declared type then an error will
be thrown.

```koto
let x: String = 'hello'
print! x
check! hello

let a: Number, _, c: Bool = 123, x, true
print! a, c
check! (123, true)
```

### `for` arguments

Type hints can also be added to `for` loop arguments.
The type will be checked on each iteration of the loop.

```koto
for i: Number, s: String in 'abc'.enumerate()
  print i, s
check! (0, 'a')
check! (1, 'b')
check! (2, 'c')
```

### Functions

Function arguments can also be given type hints, and the type of the
return value can be checked with the `->` operator.

```koto
f = |s: String| -> Tuple
  s.to_tuple()
print! f 'abc'
check! ('a', 'b', 'c')
```

### `match` patterns

Type hints can be used in `match` patterns to check the type of the a value.
Rather than throwing an error, if a type check fails then the next
match pattern will be attempted.

```koto
print! match 'abc'
  x: Tuple then x
  x: String then x.to_tuple()
check! ('a', 'b', 'c')
```

### Optional Values

Sometimes a value can either be of a particular type, or otherwise it should `null`.

These kinds of values are referred to as [_optional_][optional-type],
and are useful for functions or expressions that return either a valid value, or nothing at all.

Optional value types are expressed by appending `?` to the type hint.

```koto
m = {foo: 'hi!'}

print! let foo: String? = m.get('foo')?.to_uppercase()
check! HI!

print! let bar: String? = m.get('bar')?.to_uppercase()
check! null
```

### Special Types

#### `Any`

The `Any` type hint will result in a successful check with any value.

```koto
print! let x: Any = 'hello'
check! hello
```

#### `Callable`

The `Callable` type hint will accept functions, or any object that can behave
like a function.

```koto
let say_hello: Callable = || 'hello'
print! say_hello()
check! hello
```

#### `Indexable`

The `Indexable` type hint will accept any value that supports `[]` indexing.

```koto
add_first_two = |x: Indexable| x[0] + x[1]
print! add_first_two (100, 99, -1)
check! 199
```

#### `Iterable`

The `Iterable` type hint is useful when any iterable value can be accepted.

```koto
let a: Iterable, b: Iterable = [1, 2], 3..=5
print! a.chain(b).to_tuple()
check! (1, 2, 3, 4, 5)
```

## String Formatting

Interpolated string expressions can be formatted using formatting options
similar to [Rust's][rust-format-options].

Inside an interpolated expression, options are provided after a `:` separator.

```koto
print! '{number.pi:𝜋^8.2}'
check! 𝜋𝜋3.14𝜋𝜋
```

### Minimum Width and Alignment

A minimum width can be specified, ensuring that the formatted value takes up at
least that many characters.

```koto
foo = "abcd"
print! '_{foo:8}_'
check! _abcd    _
```

The minimum width can be prefixed with an alignment modifier:

- `<` - left-aligned
- `^` - centered
- `>` - right-aligned

```koto
foo = "abcd"
print! '_{foo:^8}_'
check! _  abcd  _
```

All values are left-aligned if an alignment modifier isn't specified,
except for numbers which are right-aligned by default.

```koto
x = 1.2
print! '_{x:8}_'
check! _     1.2_
```

The alignment modifier can be prefixed with a character which will be used to
fill any empty space in the formatted string (the default character being ` `).


```koto
x = 1.2
print! '_{x:~<8}_'
check! _1.2~~~~~_
```

For numbers, the minimum width can be prefixed with `0`, which will pad the
number to the specified width with zeroes.

```koto
x = 1.2
print! '{x:06}'
check! 0001.2
```

### Maximum Width / Precision

A maximum width for the interpolated expression can be specified following a
`.` character.

```koto
foo = "abcd"
print! '{foo:_^8.2}'
check! ___ab___
```

For numbers, the maximum width acts as a 'precision' value, or in other words,
the number of decimal places that will be rendered for the number.

```koto
x = 1 / 3
print! '{x:.4}'
check! 0.3333
```

### Representation

Values can be formatted with alternative representations, with representations chosen with a character at the end of the format options.

- `?` - The value will be formatted with additional debug information when available.

The following representations are only supported for numbers:
- `e` - exponential (lower-case)
- `E` - exponential (upper-case)

The following representations are only supported for integers:
- `b` - binary
- `o` - octal
- `x` - hexadecimal (lower-case)
- `X` - hexadecimal (upper-case)

```koto
z = 60
print! '{z:?}'
check! 60
print! '{z:x}'
check! 3c
print! '0x{z:X}'
check! 0x3C
print! '{z:o}'
check! 74
print! '0b{z:08b}'
check! 0b00111100
print! '{z * 1000:e}'
check! 6e4
print! '{z * 1_000_000:E}'
check! 6E7
```

## Advanced Functions

Functions in Koto have some advanced features that are worth exploring.

### Captured Variables

When a variable is accessed in a function that wasn't declared locally,
then it gets _captured_ by copying it into the function.

```koto
x = 1

my_function = |n|
  # x is assigned outside the function,
  # so it gets captured when the function is created.
  n + x

# Reassigning x here doesn't modify the value
# of x that was captured when my_function was created.
x = 100

print! my_function 2
check! 3
```

This behavior is different to many other languages,
where captures are often taken by _reference_ rather than by _copy_.

It's also worth noting that captured variables will have the same starting value
each time the function is called.

```koto
x = 99
f = ||
  # Modifying x only happens with a local copy during a function call.
  # The value of x at the start of the call matches when the value it had when
  # it was captured.
  x += 1

print! f(), f(), f()
check! (100, 100, 100)
```

To modify captured values, use a container (like a map) to hold on to mutable
data.

```koto
data = {x: 99}

f = ||
  # The data map gets captured by the function,
  # and its contained values can be modified between calls.
  data.x += 1

print! f(), f(), f()
check! (100, 101, 102)
```

### Variadic Functions

A [_variadic function_][variadic] can be created by appending `...` to the last argument.
When the function is called, any extra arguments will be collected into a tuple.

```koto
f = |a, b, others...|
  print "a: {a}, b: {b}, others: {others}"

f 1, 2, 3, 4, 5
check! a: 1, b: 2, others: (3, 4, 5)
f 10, 20
check! a: 10, b: 20, others: ()
```

### Optional Arguments

Function arguments can be given default values, making them _optional_.

```koto
f = |a, b = 2, c = 3|
  print a, b, c

f 1
check! (1, 2, 3)
f 1, -2
check! (1, -2, 3)
f 1, -2, -3
check! (1, -2, -3)
```

Default argument values behave like [captured variables](#captured-variables),
with the same value being applied each time the function is called.

```koto
f = |x = 10|
  x += 1
  x

print! f()
check! 11
print! f()
check! 11
```

All arguments following an optional argument must also be optional,
unless the last argument is [variadic](#variadic-functions).

```koto
# f = |a = 1, b| a, b
#             ^ Error!

f = |a = 1, b...| a, b
#           ^ Ok!

print! f()
check! (1, ())
print! f(1, 2, 3)
check! (1, (2, 3))
```

#### Mutable Default Argument Values

It's worth noting that mutable values (like [lists](#lists) and [maps](#maps)) share their state between calls when used as default argument values.

```koto
f = |value, values = []|
  values.push value

print! f 1
check! [1]
print! f 2
check! [1, 2]
```

This might seem a bit strange at first (why doesn't the `values` argument start with an empty list on each call?), but it might help to consider what happens when a named variable is given as the default value.

```koto
z = [1, 2]
f = |value, values = z|
  values.push value

f 3
f 4

# z was used as the default value for the function's `values` argument.
print! z
check! [1, 2, 3, 4]
```

Lists usually share state between instances when [captured](#captured-variables) in functions, and a hidden [`copy`](./core_lib/koto.md#copy) on each call would be surprising, and potentially expensive.

### Unpacking Container Arguments

Functions that expect containers as arguments can _unpack_ the container's
elements directly by using parentheses.

```koto
# A function that sums a value that contains three values
f = |(a, b, c)| a + b + c

x = [100, 10, 1]
print! f x
check! 111
```

Any container that supports indexing operations (like lists and tuples)
with a matching number of elements will be unpacked,
otherwise an error will be thrown.

Unpacked arguments can also be nested.

```koto
# A function that sums elements from nested containers
f = |((a, b), (c, d, e))|
  a + b + c + d + e
x = ([1, 2], [3, 4, 5])
print! f x
check! 15
```

An ellipsis (`...`) can be used to unpack any number of elements at the start or end of a container.

```koto
f = |(..., last)| last * last
x = (1, 2, 3, 4)
print! f x
check! 16
```

A name can be added an ellipsis to capture the unpacked elements in a tuple.

```koto
f = |(first, others...)| first * others.sum()
x = (10, 1, 2, 3)
print! f x
check! 60
```

### Ignoring Arguments

As with [assignments](#ignoring-unpacked-values), `_` can be used to ignore function arguments.

```koto
# A function that sums the first and third elements of a container
f = |(a, _, c)| a + c

print! f [100, 10, 1]
check! 101
```

```koto
my_map = {foo1: 1, bar1: 2, foo2: 3, bar2: 4}

print! my_map
  .keep |(key, _value)| key.starts_with 'foo'
  .to_tuple()
check! (('foo1', 1), ('foo2', 3))
```

### Packed Call Arguments

When calling a function, a _packed argument_ is any argument to which `...` is appended.
The runtime will replace the packed argument with the output of iterating over the argument's contents.
Any iterable value can be unpacked.

```koto
f = |a, b, c| a + b + c

x = 10, 20, 30
print! f x...
check! 60

print! f (1..10).take(3)...
check! 6
```

This is especially useful when [variadic arguments](#variadic-functions) need
to be forwarded to another variadic function.

```koto
f = |args...|
  for i, arg in args.enumerate()
    print '{i}: {arg}'

g = |args...| f args...
g 2, 4, 6, 8
check! 0: 2
check! 1: 4
check! 2: 6
check! 3: 8
```

More than one argument can be unpacked during a call.

```koto
f = |args...|
  for i, arg in args.enumerate()
    print '{i}: {arg}'

x = 10, 20
y = 99, 100
f x..., -1, y...
check! 0: 10
check! 1: 20
check! 2: -1
check! 3: 99
check! 4: 100
```

## Generators

Generators are iterators that are made by calling _generator functions_,
which are any functions that contain a `yield` expression.

The generator is paused each time `yield` is encountered,
waiting for the caller to continue execution.

```koto
my_first_generator = ||
  yield 1
  yield 2

x = my_first_generator()
print! x.next()
check! IteratorOutput(1)
print! x.next()
check! IteratorOutput(2)
print! x.next()
check! null
```

Generator functions can accept arguments like any other function,
and each time they're called a new generator is created.

As with any other iterable value, the [`iterator`][iterator] module's functions
are made available to generators.

```koto
make_generator = |x|
  for y in (1, 2, 3)
    yield x + y

print! make_generator(0).to_tuple()
check! (1, 2, 3)

print! make_generator(10)
  # Keep odd numbers, and discard even numbers
  .keep |n| n % 2 == 1
  .to_list()
check! [11, 13]
```

When defining a generator, a `->` [type hint](#type-checks) is used to check
the type of the generator's `yield` expressions.

```koto
g = || -> Number
  yield 1
  yield 2
  yield 3
print! g().to_tuple()
check! (1, 2, 3)
```

### Custom Iterator Adaptors

Generators can also serve as _iterator adaptors_ by modifying the output of
another iterator.

Inserting a generator into the [`iterator`][iterator] module makes it available
in any iterator chain.

```koto
# Make an iterator adaptor that yields every
# other value from the adapted iterator
iterator.every_other = |iter = null|
  n = 0
  # If the iterator to be adapted is provided as an argument then use it,
  # otherwise defer to `self`, which is set by the runtime when the
  # generator is used in an iterator chain.
  for output in iter or self
    # If n is even, then yield a value
    if n % 2 == 0
      yield output
    n += 1

# The adaptor can be called directly...
print! iterator.every_other('abcdef').to_string()
check! ace

# ...or anywhere in an iterator chain
print! (1, 2, 3, 4, 5)
  .each |n| n * 10
  .every_other()
  .to_list()
check! [10, 30, 50]
```

## Objects and Metamaps

Value types with custom behaviour can be defined in Koto through the concept of
_objects_.

An object is any map that includes one or more _metakeys_
(keys prefixed with `@`), that are stored in the object's _metamap_.
Whenever operations are performed on the object, the runtime checks its metamap
for corresponding metakeys.

In the following example, the addition and multiply-assignment operators are
implemented for a custom `Foo` object:

```koto
# Declare a function that makes Foo objects
foo = |n|
  data: n

  # Declare the object's type
  @type: 'Foo'

  # Implement the addition operator
  @+: |other|
    # A new Foo is made using the result
    # of adding the two data values together
    foo self.data + other.data

  # Implement the multiply-assignment operator
  @*=: |other|
    self.data *= other.data
    self

a = foo 10

print! type a
check! Foo

b = foo 20

print! (a + b).data
check! 30

a *= b
print! a.data
check! 200
```

### Arithmetic Operators

All arithmetic operators used in binary expressions can be implemented in an object's metamap
by implementing functions for the appropriate metakeys.

When the object is on the left-hand side (_LHS_) of the expression the metakeys are
`@+`, `@-`, `@*`, `@/`, `@%`, and `@^`.

If the value on the LHS of the expression doesn't support the operation and the object is on the
right-hand side (_RHS_), then the metakeys are `@r+`, `@r-`, `@r*`, `@r/`, `@r%`, and `@r^`.

If your type only supports an operation when the input has a certain type,
then throw a [`koto.unimplemented`][koto-unimplemented] error to let the runtime know that
the RHS value should be checked. The runtime will catch the error and then attempt the operation
with the implementation provided by the RHS value.

```koto
foo = |n|
  data: n

  @type: 'Foo'

  # The * operator when the object is on the LHS
  @*: |rhs|
    match type rhs
      'Foo' then foo self.data * rhs.data
      'Number' then foo self.data * rhs
      else throw koto.unimplemented

  # The * operator when the object is on the RHS
  @r*: |lhs| foo lhs * self.data

a = foo 2
b = foo 3

print! (a * b).data
check! 6

print! (10 * a).data
check! 20
```

### Comparison Operators

Comparison operators can also be implemented in an object's metamap
by using the metakeys `@==`, `@!=`, `@<`, `@<=`, `@>`, and `@>=`.

By default, `@!=` will invert the result of calling `@==`,
so it's only necessary to implement it for types with special equality properties.

Types that represent a [total order][total-order] only need to implement `@<` and `@==`,
and the runtime will automatically derive results for `@<=`, `@>`, and `@>=`.

```koto
foo = |n|
  data: n

  @==: |other| self.data == other.data
  @<: |other| self.data < other.data

a = foo 100
b = foo 200

print! a == a
check! true

# The result of != is derived by inverting the result of @==
print! a != a
check! false

print! a < b
check! true

# The result of > is derived from the implementations of @< and @==
print! a > b
check! false
```

### Metakeys

#### `@negate`

The `@negate` metakey overrides the `-` negation operator.

```koto
foo = |n|
  data: n
  @negate: || foo -self.data

x = -foo(100)
print! x.data
check! -100
```

#### `@size` and `@index`

The `@size` metakey defines how an object should report its size,
while the `@index` metakey defines which values should be returned
when indexing is performed.

If `@size` is implemented, then `@index` should also be implemented.

```koto
foo = |data|
  data: data
  @size: || size self.data
  @index: |index| self.data[index]

x = foo ('a', 'b', 'c')
print! size x
check! 3
print! x[1]
check! b
```

Implementing `@size` and `@index` allows an object to participate in argument unpacking.

The `@index` implementation can support indexing by any input values that make
sense for your object type, however for argument unpacking to work correctly, the
runtime expects that indexing should be supported for at least single indices and ranges.

```koto
foo = |data|
  data: data
  @size: || size self.data
  @index: |index| self.data[index]

x = foo (10, 20, 30, 40, 50)

# Unpack the first two elements in the value passed to the function and multiply them
multiply_first_two = |(a, b, ...)| a * b
print! multiply_first_two x
check! 200

# Inspect the first element in the object
print! match x
  (first, others...) then 'first: {first}, remaining: {size others}'
check! first: 10, remaining: 4
```

#### `@index_assign`

The `@index_assign` metakey defines how an object should behave when index-assignment is used.

The given value should be a function that takes an index as the first argument,
with the second argument being the value to be assigned.

```koto
foo = |data|
  data: data
  @index: |index| self.data[index]
  @index_assign: |index, value| self.data[index] = value

x = foo ['a', 'b', 'c']
x[1] = 'hello'
print! x[1]
check! hello
```

#### `@access` and `@access_assign`

The `@access` and `@access_assign` metakeys allow objects so override how `.` access operations behave.

Note that care must be taken to avoid accessing members of `self` via `.` to avoid creating infinite loops!

```koto
foo =
  @access: |key|
    # Multiply values by 2 when accessed
    map.get(self, key) * 2

  @access_assign: |key, value|
    # Multiply values by 100 when assigned
    map.insert(self, key, value * 100)

foo.x = 1

# The assigned value was multiplied by 100 in @access_assign, and by 2 in @access.
print! foo.x
check! 200
```

#### `@call`

The `@call` metakey defines how an object should behave when its called as a
function.

```koto
foo = |n|
  data: n
  @call: |arg|
    self.data *= arg

x = foo 2
print! x(10)
check! 20
print! x(4)
check! 80
```

#### `@iterator`

The `@iterator` metakey defines how iterators should be created when an object
is used in an iterable context.

When called, `@iterator` should return an iterable value that will then be used
for iterator operations.

```koto
foo = |n|
  # Return a generator that yields the three numbers following n
  @iterator: ||
    yield n + 1
    yield n + 2
    yield n + 3

print! foo(0).to_tuple()
check! (1, 2, 3)

print! foo(100).to_list()
check! [101, 102, 103]
```

Note that the `@iterator` metakey will be ignored if the object also implements `@next`,
which implies that the object is _already_ an iterator.

#### `@next`

The `@next` metakey allows for objects to treated as iterators.

Whenever the runtime needs to produce an iterator from an object, it will first
check the metamap for an implementation of `@next`, before looking for `@iterator`.

The `@next` function will be called repeatedly during iteration,
with the returned value being used as the iterator's output.
When the returned value is `null` then the iterator will stop producing output.

```koto
foo = |start, end|
  start: start
  end: end
  @next: ||
    if self.start < self.end
      result = self.start
      self.start += 1
      result
    else
      null

print! foo(10, 15).to_tuple()
check! (10, 11, 12, 13, 14)
```

#### `@next_back`

The `@next_back` metakey is used by [`iterator.reversed`][iterator-reversed]
when producing a reversed iterator.

The runtime will only look for `@next_back` if `@next` is implemented.

```koto
foo =
  n: 0
  @next: || self.n += 1
  @next_back: || self.n -= 1

print! foo
  .skip 3 # 0, 1, 2
  .reversed()
  .take 3 # 2, 1, 0
  .to_tuple()
check! (2, 1, 0)
```

#### `@display`

The `@display` metakey defines how an object should be represented when
displaying the object as a string.

```koto
foo = |n|
  data: n
  @display: || 'Foo({self.data})'

print! foo 42
check! Foo(42)

x = foo -1
print! "The value of x is '{x}'"
check! The value of x is 'Foo(-1)'
```

#### `@debug`

The `@debug` metakey defines how an object should be represented when
displaying the object in a debug context, e.g. when using [`debug`](#debug-1),
or when the [`?` representation](#representation) is used in an interpolated expression.

```koto
foo = |n|
  data: n
  @display: || 'Foo({self.data})'
  @debug: || '!!{self}!!'

print! "{foo(123):?}"
check! !!Foo(123)!!
```

If `@debug` isn't defined, then `@display` will be used as a fallback.

#### `@type`

The `@type` metakey takes a string which is used when checking a
value's type, e.g. with [type checks](#type-checks) or [`koto.type`][koto-type].

```koto
foo = |n|
  data: n
  @type: "Foo"

let x: Foo = foo 42
print! koto.type x
check! Foo
```

#### `@base`

Objects can inherit properties and behavior from other values,
establishing a _base value_ through the `@base` metakey.
This allows objects to share common functionality while maintaining their own
unique attributes.

In the following example, two kinds of animals are created that share the
`speak` function from their base value.

```koto
animal = |name|
  @type: 'Animal'
  name: name
  speak: || '{self.noise}! My name is {self.name}!'

dog = |name|
  @base: animal name
  @type: 'Dog'
  noise: 'Woof'

cat = |name|
  @base: animal name
  @type: 'Cat'
  noise: 'Meow'

let fido: Dog = dog 'Fido'
print! fido.speak()
check! Woof! My name is Fido!

let smudge: Cat = cat 'Smudge'
print! smudge.speak()
check! Meow! My name is Smudge!

# Type checks will refer to base class @type entries when needed
let an_animal: Animal = if true then fido else smudge
print! an_animal.name
check! Fido
```

#### `@meta`

The `@meta` metakey allows named metakeys to be added to the metamap.

Metakeys defined with `@meta` are accessible via `.` access,
similar to regular object `keys`, but they don't appear as part of the object's
main data entries when treated as a regular map.

```koto
foo = |n|
  data: n
  @meta hello: "Hello!"
  @meta get_info: ||
    info = match self.data
      0 then "zero"
      n if n < 0 then "negative"
      else "positive"
    "{self.data} is {info}"

x = foo -1
print! x.hello
check! Hello!

print x.get_info()
check! -1 is negative

print map.keys(x).to_tuple()
check! ('data')
```

### Sharing Metamaps

Metamaps can be shared between objects by using
[`Map.with_meta`][map-with_meta], which helps to avoid inefficient
duplication when creating a lot of objects.

In the following example, behavior is overridden in a single metamap, which is
then shared between object instances.

```koto
# Create an empty map for global values
global = {}

# Define a function that makes a Foo object
foo = |data|
  # Make a new map that contains `data`,
  # and then attach a shared copy of the metamap from foo_meta.
  {data}.with_meta global.foo_meta

# Define some metakeys in foo_meta
global.foo_meta =
  # Declare the object's type
  @type: 'Foo'

  # Override the + operator
  @+: |other| foo self.data + other.data

  # Define how the object should be displayed
  @display: || "Foo({self.data})"

print! (foo 10) + (foo 20)
check! Foo(30)
```

## Error Handling

Errors can be _thrown_ in the Koto runtime, which then cause the runtime to stop
execution.

A _try_ / _catch_ expression can be used to catch any errors thrown while inside
the `try` block, allowing execution to continue.

An optional `finally` block can be used for cleanup actions that need to
performed whether or not an error was caught.

```koto
x = [1, 2, 3]
try
  # Accessing an invalid index will throw an error
  print x[100]
catch error
  print "Caught an error: '{error}'"
finally
  print "...and finally"
check! Caught an error: 'index out of bounds - index: 100, size: 3'
check! ...and finally
```

`throw` can be used to explicitly throw an error when an exceptional condition
has occurred.

`throw` accepts strings or objects that implement `@display`.

```koto
f = || throw "!Error!"

try
  f()
catch error
  print "Caught an error: '{error}'"
check! Caught an error: '!Error!'
```

### Type checks on `catch` blocks

Type hints can also be used in `try` expressions to implement different
error handling logic depending on the type of error that has been thrown.
A series of `catch` blocks can be added to the `try` expression, each catching
an error that has a particular type.

The final `catch` block needs to _not_ have a type check so that it can catch
any errors that were missed by the other blocks.

```koto
f = || throw 'Throwing a String'

try
  f()
catch n: Number
  print 'An error occurred: {n}'
catch error: String
  print error
catch other
  print 'Some other error occurred: {other}'
check! Throwing a String
```

## Modules

Koto includes a module system that helps you to organize and re-use your code
when your program grows too large for a single file.

### `import`

Values from other modules can be brought into the current scope using `import`.

```koto
from list import last
from number import abs

x = [1, 2, 3]
print! last x
check! 3

print! abs -42
check! 42
```

Multiple values from a single module can be imported at the same time.

```koto
from tuple import contains, first, last

x = 'a', 'b', 'c'
print! first x
check! a
print! last x
check! c
print! contains x, 'b'
check! true
```

Imported values can be renamed using `as` for clarity or to avoid conflicts.

```koto
from list import first as list_first
from tuple import first as tuple_first
print! list_first [1, 2]
check! 1
print! tuple_first (3, 2, 1)
check! 3
```

You can also use `*` to import all of a module's exported values at once (known as a _wildcard import_) .

```koto
from number import *

print! abs -1
check! 1
print! sqrt 25
check! 5.0
```

### `export`

A value can only be imported from a module if the module has _exported_ it.

`export` is used to add values to the current module's _exports map_,
making them available to be imported by other modules.

```koto,skip_run
##################
# my_module.koto #
##################

# hello is a local variable, and isn't exported
hello = 'Hello'

# Here, say_hello gets exported, making it available to other modules
export say_hello = |name| '{hello}, {name}!'

##################
#   other.koto   #
##################

from my_module import say_hello

say_hello 'Koto'
check! 'Hello, Koto!'
```

To add a [type check](#type_checks) to an exported assignment, use a `let` expression:

```koto,skip_run
export let foo: Number = -1
```

`export` also accepts maps, or any other iterable value that yields a series of key/value pairs.
This is convenient when exporting a lot of values, or generating exports programmatically.

```koto
##################
# my_module.koto #
##################

# Define some local values
a, b, c = 1, 2, 3

# Inline maps allow for shorthand syntax
export { a, b, c, foo: 42 }

# Map blocks can also be used with export
export
  bar: 99
  baz: 'baz'

# Any iterable value that yields key/value pairs can be used with export
export (1..=3).each |i| 'generated_{i}', i
```

Once a value has been exported, it becomes available anywhere in the module.

```koto
get_x = ||
  # x hasn't been created yet. When the function is called, the runtime
  # will check the exports map for a matching value.
  x

export x = 123

print! get_x()
check! 123

# A function that exports `y` with the given value
export_y = |value|
  export y = value

# y hasn't been exported yet, so attempting to access it now throws an error.
print! try
  y
catch _
  'y not found'
check! y not found

# Calling export_y adds y to the exports map
export_y 42
print! y
check! 42
```

Assigning a new value locally to a previously exported variable won't change
the exported value. If you need to update the exported value,
then it needs to be re-exported.

```koto
export x = 99

# Reassigning a new value to x locally doesn't affect the previously exported value
print! x = 123
check! 123

# x has a local value of 123, but the exported value of x is still 99.
export x = -1
# x now has an exported and local value of -1
print! x
check! -1
```

### `@main`

A module can export a `@main` function, which will be called after the module has been compiled and successfully initialized.

The use of `export` is optional when assigning to module metakeys like `@main`.

```koto,skip_run
##################
# my_module.koto #
##################

export say_hello = |name| 'Hello, {name}!'

# Equivalent to `export @main = ...`
@main = || print '`my_module` initialized'

##################
#   other.koto   #
##################

from my_module import say_hello
check! `my_module` initialized

say_hello 'Koto'
check! 'Hello, Koto!'
```

### Module Paths

When looking for a module, `import` will look for a `.koto` file with a matching
name, or for a folder with a matching name that contains a `main.koto` file.

For example, when the expression `import foo` is evaluated,
then the runtime will look for a `foo.koto` file in the same location as the current script,
and if one isn't found then the runtime will look for `foo/main.koto`.

## Testing

Koto includes a simple testing framework that allows you to automatically check that your code is behaving as you would expect.

### Assertions

The core library includes a collection of _assertion_ functions which
throw errors if a given condition isn't met.

The assertion functions are found in the [`test` module](./core_lib/test.md),
and are included by default in the [prelude](#prelude).

```koto
try
  assert 1 + 1 == 2
  print 'The assertion passed'
catch error
  print 'The assertion failed'
check! The assertion passed

try
  assert_eq 'hello', 'goodbye'
  print 'The assertion passed'
catch error
  print 'The assertion failed'
check! The assertion failed
```

### Module Tests

Tests can be added to a module by exporting `@test` functions. A test function is considered to have failed if it throws an error (e.g. from an assertion).

If Koto is configured to run tests, then the tests will be run after a module has been successfully initialized.

After all tests have run successfully, then the runtime will call the module's `@main` function if it's defined.

The CLI doesn't enable tests by default when running scripts, but they can be enabled [via a flag][cli-tests].

```koto,skip_run
##################
# my_module.koto #
##################

export say_hello = |name| 'Hello, {name}!'

@main = || print '`my_module` initialized'

@test say_hello = ||
  print 'Running @test say_hello'
  assert_eq say_hello('Test'), 'Hello, Test!'

##################
#   other.koto   #
##################

from my_module import say_hello
check! Running @test say_hello
check! `my_module` initialized
```

`@pre_test` and `@post_test` functions can be implemented alongside tests
for setup and cleanup operations.
`@pre_test` will be run before each `@test`, and `@post_test` will be run after.

```koto,skip_run
##################
# my_module.koto #
##################

export say_hello = |name| 'Hello, {name}!'

@main = || print '`my_module` initialized'

@pre_test = ||
  print 'In @pre_test'

@post_test = ||
  print 'In @post_test'

@test say_hello_1 = ||
  print 'Running @test say_hello_1'
  assert_eq say_hello('One'), 'Hello, One!'

@test say_hello_2 = ||
  print 'Running @test say_hello_2'
  assert_eq say_hello('Two'), 'Hello, Two!'

##################
#   other.koto   #
##################

from my_module import say_hello
check! In @pre_test
check! Running @test say_hello_1
check! In @post_test
check! In @pre_test
check! Running @test say_hello_2
check! In @post_test
check! `my_module` initialized
```

### Running Tests Manually

Tests can be run manually by calling [`test.run_tests`][test-run_tests]
with a map that contains `@test` functions.

```koto
my_tests =
  @test add: || assert_eq 1 + 1, 2
  @test subtract: || assert_eq 1 - 1, 0

test.run_tests my_tests
```

---

You've made it to the end of the guide! If you spotted any mistakes, or noticed any sections that were less clear than you would have liked,
then please open an [issue][issues] or create a [PR][prs].

For further reading, take a look at docs for the [core library][core], the [extra libs][extra-libs], or how Koto can be integrated into Rust applications in the [Rust API docs][rust-api].

[ascii]: https://en.wikipedia.org/wiki/ASCII
[associated]: https://en.wikipedia.org/wiki/Associative_array
[chars]: ./core_lib/string.md#chars
[cli]: ./cli.md
[cli-tests]: ./cli.md#running_tests
[compound-assignment]: https://en.wikipedia.org/wiki/Augmented_assignment
[core]: ./core_lib
[extra-libs]: ./libs
[immutable]: https://en.wikipedia.org/wiki/Immutable_object
[issues]: https://github.com/koto-lang/koto/issues
[iterator]: ./core_lib/iterator.md
[iterator-count]: ./core_lib/iterator.md#count
[iterator-reversed]: ./core_lib/iterator.md#reversed
[iterator-sum]: ./core_lib/iterator.md#sum
[koto-exports]: ./core_lib/koto.md#exports
[koto-type]: ./core_lib/koto.md#type
[koto-unimplemented]: ./core_lib/koto.md#unimplemented
[map-get]: ./core_lib/map.md#get
[map-insert]: ./core_lib/map.md#insert
[map-with_meta]: ./core_lib/map.md#with_meta
[prs]: https://github.com/koto-lang/koto/pulls
[lazy]: https://en.wikipedia.org/wiki/Lazy_evaluation
[next]: ./core_lib/iterator.md#next
[object-wiki]: https://en.wikipedia.org/wiki/Object_(computer_science)
[once]: ./core_lib/iterator.md#once
[optional-type]: https://en.wikipedia.org/wiki/Option_type
[operation-order]: https://en.wikipedia.org/wiki/Order_of_operations#Conventional_order
[repeat]: ./core_lib/iterator.md#repeat
[rust-api]: ./api.md
[rust-format-options]: https://doc.rust-lang.org/std/fmt/#formatting-parameters
[test-run_tests]: ./core_lib/test.md#run_tests
[to_list]: ./core_lib/iterator.md#to_list
[to_map]: ./core_lib/iterator.md#to_map
[to_string]: ./core_lib/iterator.md#to_string
[to_tuple]: ./core_lib/iterator.md#to_tuple
[total-order]: https://en.wikipedia.org/wiki/Total_order
[utf-8]: https://en.wikipedia.org/wiki/UTF-8
[variadic]: https://en.wikipedia.org/wiki/Variadic_function
