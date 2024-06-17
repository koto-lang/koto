A rendered version of this document can be found
[here](https://koto.dev/docs/next/language).

See the neighbouring [readme](./README.md) for an explanation of the
`print!` and `check!` commands used in the following example.

---

# The Koto Language Guide

## Language Basics

Koto programs contain a series of expressions that are evaluated by Koto's runtime.

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

### Numbers 

Numbers and arithmetic are expressed in a familiar way.

```koto
print! 1
check! 1

print! 1 + 1
check! 2

print! -1 - 10
check! -11

print! 3 * 4
check! 12

print! 9 / 2
check! 4.5

print! 12 % 5
check! 2
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

Tuples in Koto are similiar to lists, 
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

### Joining Tuples

The `+` operator allows tuples to be joined together, 
creating a new tuple containing their concatenated elements.

```koto
a = 1, 2, 3
b = a + (4, 5, 6)
print! b
check! (1, 2, 3, 4, 5, 6)
```

### Creating Empty Tuples 

An empty pair of parentheses in Koto resolves to `null`.
If an empty tuple is needed then use a single `,` inside parentheses.

```koto
# An empty pair of parentheses resolves to null
print! () 
check! null

# A comma inside parentheses creates a tuple 
print! (,) 
check! ()
```

### Tuple Mutability

While tuples have a fixed structure and its contained elements can't be 
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

### Single or Double Quotes

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
error will be thrown. To access unicode characters see [`string.chars`][chars].

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

The parentheses for arguments when calling a function are optional and can be 
ommitted in simple expressions.

```koto
square = |x| x * x
print! square 8
check! 64

add = |x, y| x + y
print! add 2, 3
check! 5

# Equivalent to square(add(2, 3))
print! square add 2, 3 
check! 25
```

Something to watch out for is that whitespace is important in Koto, and because
of optional parentheses, `f(1, 2)` is _not the same_ as `f (1, 2)`. The former
is parsed as a call to `f` with two arguments, whereas the latter is a call to
`f` with a tuple as the single argument.


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

## Maps

_Maps_ in Koto are containers that contain a series of 
_entries_ with _keys_ that correspond to [associated][associated] _values_.

The `.` dot operator returns the value associated with a particular key.

Maps can be created using _inline syntax_ with `{}` braces:

```koto
m = {apples: 42, oranges: 99, lemons: 63}

# Get the value associated with the `oranges` key
print! m.oranges
check! 99
```

...or using _block syntax_ with indented entries:

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

### Shorthand Values

Koto supports a shorthand notation when creating maps with inline syntax. 
If a value isn't provided for a key, then Koto will look for a value in scope
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
enabling object-like behaviour.

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
Quoted keys also allow dynamic keys to be generated by using string
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
used as a map key by using the [map.insert][map-insert] and [map.get][map-get] 
functions.

The immutable value types in Koto are [strings](#strings), [numbers](#numbers), 
[booleans](#booleans), [ranges](#ranges), and [`null`](#null).

A [tuple](#tuples) is also considered to be immutable when its contained
elements are also immutable.


## Core Library

The [_Core Library_][core] provides a collection of fundamental functions
and values for working with the Koto language, organized within _modules_. 

```koto
# Get the size of a string
print! string.to_lowercase 'HELLO'
check! hello

# Return the first element of the list
print! list.first [99, -1, 3]
check! 99
```

Values in Koto automatically have access to their corresponding core modules 
via `.` access.

```koto
print! 'xyz'.to_uppercase()
check! XYZ

print! ['abc', 123].first()
check! abc

print! (7 / 2).round()
check! 4

print! {apples: 42, pears: 99}.contains_key 'apples'
check! true
```

The [documentation][core] for the Core library (along with this guide) are
available in the `help` command of the [Koto CLI][cli].

### Prelude

Koto's _prelude_ is a collection of core library items that are automatically 
made available in a Koto script without the need for first calling `import`.

The modules that make up the core library are all included by default in the 
prelude. The following functions are also added to the prelude by default:

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

### if

`if` expressions come in two flavours; single-line:

```koto
x = 99
if x % 2 == 0 then print 'even' else print 'odd'
check! odd
```

...and multi-line using indented blocks:

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

### switch

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

### match

`match` expressions can be used to match a value against a series of patterns, 
with the matched pattern causing a specific branch of code to be executed.

Patterns can be literals or identifiers. An identifier will accept any value, 
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

The `_` wildcard match can be used to match against any value 
(when the matched value itself can be ignored), 
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
  ('a', 'b') then "A list containing 'a' and 'b'"
  (1, ...) then "Starts with '1'"
  (..., 'y', last) then "Ends with 'y' followed by '{last}'"
  ('a', x, others...) then
    "Starts with 'a', followed by '{x}', then {size others} others"
  unmatched then "other: {unmatched}"
check! Starts with 'a', followed by 'b', then 4 others
```

## Loops

Koto includes several ways of evaluating expressions repeatedly in a loop.

### for

`for` loops are repeated for each element in a sequence, 
such as a list or tuple.

```koto
for n in [10, 20, 30]
  print n
check! 10
check! 20
check! 30
```

### while

`while` loops continue to repeat _while_ a condition is true.

```koto
x = 0
while x < 5
  x += 1
print! x
check! 5
```

### until

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

### continue 

`continue` skips the remaining part of a loop's body and proceeds with the next repetition of the loop.

```koto
for n in (-2, -1, 1, 2)
  # Skip over any values less than 0
  if n < 0
    continue
  print n
check! 1
check! 2
```

### break

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

### loop

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

The [`iterator` module][iterator] contains iterator _generators_ like
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
[`iterator` module][iterator].

```koto
# Create an iterator that keeps any value above 3
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

Iterator adaptors can be passed into other adaptors, creating _iterator chains_
that act as data processing pipelines.

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
  .keep |n| n % 2 == 1
  .to_list()
check! [11, 13]
```

### Custom Iterator Adaptors

Generators can also serve as _iterator adaptors_ by modifying the output of
another iterator.

Inserting a generator into the [`iterator`][iterator] module makes it available
in any iterator chain.

```koto
# Make an iterator adaptor that yields every
# other value from the adapted iterator
iterator.every_other = ||
  n = 0
  # When the generator is created, self is initialized with the previous
  # iterator in the chain, allowing its output to be adapted.
  for output in self
    # If n is even, then yield a value
    if n % 2 == 0
      yield output
    n += 1

print! (1, 2, 3, 4, 5)
  .each |n| n * 10
  .every_other() # Skip over every other value in the iterator chain
  .to_list()
check! [10, 30, 50]
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
if the start of the range if ommitted then the slice starts from the beginning 
of the container. 
If the end of the range is ommitted, then the slice includes all remaining 
elements in the container.

```koto
z = 'Hëllø'.to_tuple()
print! z[..2]
check! ('H', 'ë')
print! z[2..]
check! ('l', 'l', 'ø')
```

## Type Checks

Koto is a primarily a dynamically typed language, however in more complex programs 
you might find it beneficial to add type checks.

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

For [generator functions](#generators), the `->` type hint is used to check
the generator's `yield` expressions.

```koto
g = || -> Number 
  yield 1
  yield 2
  yield 3
print! g().to_tuple()
check! (1, 2, 3)
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

### Special Types

#### `Any`

The `Any` type will result in a successful check with any value.

```koto
print! let x: Any = 'hello'
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

The `Iterable` type is useful when any iterable value can be accepted. 

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

### Optional Arguments

When calling a function, any missing arguments will be replaced by `null`.

```koto
f = |a, b, c|
  print a, b, c

f 1
check! (1, null, null)
f 1, 2
check! (1, 2, null)
f 1, 2, 3
check! (1, 2, 3)
```

Missing arguments can be replaced with default values by using `or`.

```koto
f = |a, b, c|
  print a or -1, b or -2, c or -3

f 42
check! (42, -2, -3)
f 99, 100
check! (99, 100, -3)
```

`or` will reject `false`, so if `false` would be a valid input then a
direct comparison against `null` can be used instead.

```koto
f = |a| 
  print if a == null then -1 else a

f()
check! -1
f false
check! false
```

### Variadic Functions

A [_variadic function_][variadic] can be created by appending `...` to the 
last argument. 
When the function is called any extra arguments will be collected into a tuple.

```koto
f = |a, b, others...|
  print "a: {a}, b: {b}, others: {others}"

f 1, 2, 3, 4, 5
check! a: 1, b: 2, others: (3, 4, 5)
```

### Argument Unpacking

Functions that expect containers as arguments can _unpack_ the contained
elements directly in the argument declaration by using parentheses.

```koto
# A function that sums a container with three contained values
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

Ellipses can be used to unpack any number of elements at the start or end of a 
container.

```koto
f = |(..., last)| last * last
x = (1, 2, 3, 4)
print! f x
check! 16
```

A name can be added to ellipses to assign the unpacked elements. 

```koto
f = |(first, others...)| first * others.sum()
x = (10, 1, 2, 3)
print! f x
check! 60
```

### Ignoring Arguments

The wildcard `_` can be used to ignore function arguments.

```koto
# A function that sums the first and third elements of a container
f = |(a, _, c)| a + c

print! f [100, 10, 1]
check! 101
```

If you would like to keep the name of the ignored value as a reminder, 
then `_` can be used as a prefix for an identifier. Identifiers starting with 
`_` can be written to, but can't be accessed.

```koto
my_map = {foo_a: 1, bar_a: 2, foo_b: 3, bar_b: 4}
print! my_map
  .keep |(key, _value)| key.starts_with 'foo'
  .to_tuple()
check! (('foo_a', 1), ('foo_b', 3))
```

## Objects and Metamaps

Value types with custom behaviour can be defined in Koto through the concept of 
_objects_.

An object is any map that includes one or more _metakeys_ 
(keys prefixed with `@`), that are stored in the object's _metamap_.
Whenever operations are performed on the object, the runtime checks its metamap 
for corresponding metakeys.

In the following example, addition and subtraction operators are overridden for 
a custom `Foo` object:

```koto
# Declare a function that makes Foo objects
foo = |n|
  data: n

  # Overriding the addition operator
  @+: |other|
    # A new Foo is made using the result 
    # of adding the two data values together
    foo self.data + other.data

  # Overriding the subtraction operator
  @-: |other|
    foo self.data - other.data

  # Overriding the multiply-assignment operator
  @*=: |other|
    self.data *= other.data
    self

a = foo 10
b = foo 20

print! (a + b).data
check! 30
print! (a - b).data
check! -10
a *= b
print! a.data
check! 200
```

### Meta Operators

All of the binary arithmetic and logic operators (`*`, `<`, `>=`, etc) can be 
implemented following this pattern.

Additionally, the following metakeys can also be defined:

#### `@negate`

The `@negate` metakey overrides the negation operator.

```koto
foo = |n|
  data: n
  @negate: || foo -self.data

x = -foo(100)
print! x.data
check! -100
```

#### `@size` and `@[]`

The `@size` metakey defines how the object should report its size,
while the `@[]` metakey defines what values should be returned when indexing is
performed on the object. 

If `@size` is implemented, then `@[]` should also be implemented.

The `@[]` implementation can support indexing by any input values that make 
sense for your object type, but for argument unpacking to work correctly the
runtime expects that indexing by both single indices and ranges should be 
supported.

```koto
foo = |data|
  data: data
  @size: || size self.data
  @[]: |index| self.data[index]

x = foo (100, 200, 300)
print! size x
check! 3
print! x[1]
check! 200

# Unpack the first two elements in the argument and multiply them
multiply_first_two = |(a, b, ...)| a * b
print! multiply_first_two x
check! 20000

# Inspect the first element in the object
print! match x
  (first, others...) then 'first: {first}, remaining: {size others}'
check! first: 100, remaining: 2
```


#### `@||`

The `@||` metakey defines how the object should behave when its called as a
function.

```koto
foo = |n|
  data: n
  @||: || 
    self.data *= 2
    self.data

x = foo 2
print! x()
check! 4
print! x()
check! 8
```

#### `@iterator`

The `@iterator` metakey defines how iterators should be created when the object 
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

print! (foo 0).to_tuple()
check! (1, 2, 3)

print! (foo 100).to_list()
check! [101, 102, 103]
```

Note that this key will be ignored if the object also implements `@next`, 
which implies that the object is _already_ an iterator. 


#### `@next`

The `@next` metakey allows for objects to behave as iterators.

Whenever the runtime needs to produce an iterator from an object, it will first 
check the metamap for an implementation of `@next`, before looking for
`@iterator`.

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

The `@next_back` metakey is used by
[`iterator.reversed`](./core_lib/iterator.md#reversed) when producing a reversed
iterator. 

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

The `@display` metakey defines how the object should be represented when
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

#### `@type`

The `@type` metakey takes a string as a value which is used when checking the
value's type, e.g. with [`koto.type`](./core_lib/koto.md#type)

```koto
foo = |n|
  data: n
  @type: "Foo"

print! koto.type (foo 42)
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
  name: name
  speak: || '{self.noise}! My name is {self.name}!'

dog = |name|
  @base: animal name
  noise: 'Woof'

cat = |name|
  @base: animal name
  noise: 'Meow'

print! dog('Fido').speak()
check! Woof! My name is Fido!

print! cat('Smudge').speak()
check! Meow! My name is Smudge!
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
[`Map.with_meta`](./core_lib/map.md#with_meta), which helps to avoid inefficient
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

A `try` / `catch` expression can be used to _catch_ any thrown errors,
allowing execution to continue. 
An optional `finally` block can be used for cleanup actions that need to 
performed whether or not an error was caught.

```koto
x = [1, 2, 3]
try
  # Accessing an invalid index will throw an error
  print x[100]
catch error 
  print "Caught an error"
finally
  print "...and finally"
check! Caught an error
check! ...and finally
```

`throw` can be used to explicity throw an error when an exceptional condition
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

## Testing

Koto includes a simple testing framework that help you to check that your code 
is behaving as you expect through automated checks.

### Assertions

The core library includes a collection of _assertion_ functions in the 
[`test` module](./core_lib/test.md),
which are included by default in the [prelude](#prelude).

```koto
try 
  assert 1 + 1 == 3
catch error
  print 'An assertion failed'
check! An assertion failed

try 
  assert_eq 'hello', 'goodbye'
catch error
  print 'An assertion failed'
check! An assertion failed
```

### Organizing Tests

Tests can be organized by collecting `@test` functions in an object. 

The tests can then be run manually with 
[`test.run_tests`](./core_lib/test.md#run_tests).
For automatic testing, see the description of exporting `@tests` in the
[following section](#modules).

```koto
basic_tests = 
  @test add: || assert_eq 1 + 1, 2 
  @test subtract: || assert_eq 1 - 1, 0 

test.run_tests basic_tests
```

For setup and cleanup operations shared across tests, 
`@pre_test` and `@post_test` metakeys can be implemented.
`@pre_test` will be run before each `@test`, and `@post_test` will be run after.

```koto
make_x = |n|
  data: n
  @+: |other| make_x self.data + other.data
  @-: |other| make_x self.data - other.data

x_tests =
  @pre_test: || 
    self.x1 = make_x 100
    self.x2 = make_x 200
      
  @post_test: ||
    print 'Test complete'

  @test addition: ||
    print 'Testing addition'
    assert_eq self.x1 + self.x2, make_x 300

  @test subtraction: ||
    print 'Testing subtraction'
    assert_eq self.x1 - self.x2, make_x -100

  @test failing_test: ||
    print 'About to fail'
    assert false

try
  test.run_tests x_tests
catch _
  print 'A test failed'
check! Testing addition
check! Test complete
check! Testing subtraction
check! Test complete
check! About to fail
check! A test failed
```

## Modules

Koto includes a module system that helps you to organize and re-use your code 
when your program grows too large for a single file.

### `import`

Items from modules can be brought into the current scope using `import`.

```koto
from list import last
from number import abs

x = [1, 2, 3]
print! last x
check! 3

print! abs -42
check! 42
```

Multiple items from a single module can be imported at the same time.

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

Imported items can be renamed using `as` for clarity or to avoid conflicts.

```koto
from list import first as list_first
from tuple import first as tuple_first
print! list_first [1, 2]
check! 1
print! tuple_first (3, 2, 1)
check! 3
```

### `export`

`export` is used to add values to the current module's _exports map_.

Values can be assigned to and exported at the same time:

```koto,skip_run
##################
# my_module.koto #
##################

export hello, goodbye = 'Hello', 'Goodbye'
export say_hello = |name| '{hello}, {name}!'

##################
#   other.koto   #
##################

from my_module import say_hello, goodbye

say_hello 'Koto'
check! 'Hello, Koto!' 
print! goodbye
check! Goodbye
```

To add a [type check](#type_checks) to an exported value, use a `let` expression:

```koto,skip_run
export let foo: Number = -1
```

When exporting a lot of values, it can be convenient to use map syntax:

```koto,skip_run
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
```

### `@tests` and `@main`

A module can export a `@tests` object containing `@test` functions, which 
will be automatically run after the module has been compiled and initialized.

Additionally, a module can export a `@main` function. 
The `@main` function will be called after the module has been compiled and
initialized, and after exported `@tests` have been successfully run.

Note that because metakeys can't be assigned locally, 
the use of `export` is optional when adding entries to the module's metamap.

```koto,skip_run
##################
# my_module.koto #
##################

export say_hello = |name| 'Hello, {name}!'

@main = || # Equivalent to export @main =
  print '`my_module` initialized'

@tests =
  @test hello_world: ||
    print 'Testing...'
    assert_eq (say_hello 'World'), 'Hello, World!'

##################
#   other.koto   #
##################

from my_module import say_hello
check! Testing...
check! Successfully initialized `my_module`

say_hello 'Koto'
check! 'Hello, Koto!' 
```

### Module Paths

When looking for a module, `import` will look for a `.koto` file with a matching 
name, or for a folder with a matching name that contains a `main.koto` file.

e.g. When an `import foo` expression is run, then a `foo.koto` file will be 
looked for in the same location as the current script, 
and if `foo.koto` isn't found then the runtime will look for `foo/main.koto`.

---

[ascii]: https://en.wikipedia.org/wiki/ASCII
[associated]: https://en.wikipedia.org/wiki/Associative_array
[chars]: ./core_lib/string.md#chars
[cli]: ..
[compound-assignment]: https://en.wikipedia.org/wiki/Augmented_assignment
[core]: ./core_lib
[immutable]: https://en.wikipedia.org/wiki/Immutable_object
[iterator]: ./core_lib/iterator.md
[koto-type]: ./core_lib/koto.md#type
[map-get]: ./core_lib/map.md#get
[map-insert]: ./core_lib/map.md#insert
[lazy]: https://en.wikipedia.org/wiki/Lazy_evaluation
[next]: ./core_lib/iterator.md#next
[once]: ./core_lib/iterator.md#once
[operation-order]: https://en.wikipedia.org/wiki/Order_of_operations#Conventional_order
[repeat]: ./core_lib/iterator.md#repeat
[rust-format-options]: https://doc.rust-lang.org/std/fmt/#formatting-parameters
[to_list]: ./core_lib/iterator.md#to_list
[to_tuple]: ./core_lib/iterator.md#to_tuple
[utf-8]: https://en.wikipedia.org/wiki/UTF-8
[variadic]: https://en.wikipedia.org/wiki/Variadic_function
