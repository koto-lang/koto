# Advanced Functions

Functions in Koto have some advanced features that are worth exploring.

## Captured Variables

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

It's also worth noting that capture variables will have the same starting value
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

To modify captured state that can be modified, 
use a container (like a map) to hold on to mutable values.

```koto
data = {x: 99}

f = || 
  # The data map gets captured by the function, 
  # and its contained values can be modified between calls.
  data.x += 1

print! f(), f(), f()
check! (100, 101, 102)
```

## Optional Arguments

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

## Variadic Functions

A [_variadic function_][variadic] can be created by appending `...` to the 
last argument. 
When the function is called any extra arguments will be collected into a tuple.

```koto
f = |a, b, others...|
  print "a: $a, b: $b, others: $others"

f 1, 2, 3, 4, 5
check! a: 1, b: 2, others: (3, 4, 5)
```

## Argument Unpacking

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

## Ignoring Arguments

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

[variadic]: https://en.wikipedia.org/wiki/Variadic_function
