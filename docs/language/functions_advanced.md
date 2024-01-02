# Advanced Functions

Functions in Koto stored by the runtime as values and can hold internal captured state.

If a value is accessed in a function that wasn't assigned locally, 
then the value is copied into the function (or _captured_) when it's created. 

```koto
x = 1

# x is assigned outside the function,
# so it gets captured when it's created.
f = |n| n + x 

# Reassigning x here doesn't modify the value 
# of x that was captured when f was created.
x = 100

print! f 2
check! 3
```

It's worth noting that this behavior is different to many other scripting languages, where captures are often taken by _reference_ rather than by _value_.

## Optional Arguments

When calling a function, any missing arguments will be replaced by `null`.

```koto
f = |a, b, c|
  print (a, b, c)

f 1
check! (1, null, null)
f 1, 2
check! (1, 2, null)
f 1, 2, 3
check! (1, 2, 3)
```

In simple cases the function can check for missing arguments by using `or`.

```koto
f = |a, b, c|
  print (a or -1, b or -2, c or -3)

f 1
check! (1, -2, -3)
```

`or` will reject `false`, so if `false` might be a valid input then a
more-verbose direct comparison against `null` can be used instead.

```koto
f = |a| print if a == null then -1 else a

f()
check! -1
f false
check! false
```

## Variadic Functions

A function can accept any number of arguments by adding `...` to the last argument. 
Any additional arguments will be collected into a Tuple which will be assigned to the last argument.

```koto
f = |a, b, others...|
  print "a: $a, b: $b, others: $others"

f 1, 2, 3, 4, 5
check! a: 1, b: 2, others: (3, 4, 5)
```

## Argument Unpacking

Functions that expect List or Tuple arguments can _unpack_ their values directly in the argument declaration.

```koto
# A function that sums a List of three values
f = |[a, b, c]| a + b + c

x = [100, 10, 1]
print! f x
check! 111
```

In the above example, if anything other than a List with three values is used as
an argument, then an error will be thrown. 

Unpacked values can contain nested unpacked values.

```koto
# A function that takes a Tuple of Lists
# and sums their entries
f = |([a, b], [c, d, e])| 
  a + b + c + d + e
x = ([1, 2], [3, 4, 5])
print! f x
check! 15
```

Ellipses can be used to unpack any number of elements at the start or end of a List or Tuple. 

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

As a performance consideration, when assigning elements this way from a List, a new list will be created with copies of the elements. Unpacking elements from a Tuple is cheaper because the underlying data is shared between sub-tuples.

## Ignoring Arguments

The wildcard `_` can be used as a placeholder for arguments that the function ignores. 

```koto
# A function that takes a List,
# and sums its first and third values 
f = |[a, _, c]| a + c

print! f [100, 10, 1]
check! 101
```

If you would like to keep the name of the ignored value as a reminder, 
then `_` can be used as a prefix for an identifier (Identifiers starting with 
`_` can be written to but can't be accessed).

```koto
my_map = {foo_a: 1, bar_a: 2, foo_b: 3, bar_b: 4}
print! my_map
  .keep |(key, _value)| key.starts_with 'foo'
  .to_tuple()
check! (('foo_a', 1), ('foo_b', 3))
```

## Captured Values

