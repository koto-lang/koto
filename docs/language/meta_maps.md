# Meta Maps

The behaviour of maps in Koto can be customized.

Keys with `@` prefixes go into the map's 'meta map',
and when the map is used by the runtime for a particular operation, its meta map 
will be checked for an entry corresponding to the operation.

In the following example, addition and subtraction are overridden for a custom `Foo` value type.

```koto
# foo is a function that makes Foo values
foo = |n|
  data: n

  # Overloading the addition operator
  @+: |self, other|
    # A new Foo is made using the result 
    # of adding the two data values together
    foo self.data + other.data

  # Overloading the subtraction operator
  @-: |self, other|
    foo self.data - other.data

  # Overloading the multiply-assignment operator
  @*=: |self, other|
    self.data *= other.data
    self

foo_a = foo 10
foo_b = foo 20

print! (foo_a + foo_b).data
check! 30
print! (foo_a - foo_b).data
check! -10
foo_a *= foo_b
print! foo_a.data
check! 200
```

## Meta Operators

All the binary arithmetic operators can be overloaded following this pattern.

The following meta functions and values can also be defined:

### `@negate`

The `@negate` meta key overloads the unary negation operator.

```koto
foo = |n|
  data: n
  @negate: |self| foo -self.data

x = -foo(100)
print! x.data
check! -100
```

### `@not`

The `@not` meta key overloads the unary `not` operator.

```koto
foo = |n|
  data: n
  @not: |self| self.data == 0

print! not (foo 10)
check! false
```

### `@[]`

The `@[]` meta key defines how indexing the value with `[]` should behave.

```koto
foo = |n|
  data: n
  @[]: |self, index| self.data + index

print! (foo 10)[7]
check! 17
```

### `@iterator`

The `@iterator` meta key defines how iterators should be made when the value is
used in an iterable context. The function returns an iterable value that is used
instead of the default behaviour of iterating over the map's entries.

```koto
foo = |n|
  data: n
  @iterator: |self| 0..=self.data

print! (foo 5).to_tuple()
check! (0, 1, 2, 3, 4, 5)

print! (foo -3).to_list()
check! [0, -1, -2, -3]
```

### `@display`

The `@display` meta key defines how the value should be represented when
displaying the value with functions like [`io.print`](../../core/io/#print) 
or [`string.format`](../../core/string/#format).

```koto
foo = |n|
  data: n
  @display: |self| 'Foo: {}'.format self.data

print! foo 42
check! Foo: 42

x = foo -1
print! "The value of x is '$x'"
check! The value of x is 'Foo: -1'
```

### `@type`

The `@type` meta key takes a String as a value which is used when checking the
value's type, e.g. with [`koto.type`](../../core/koto/#type)

```koto
foo = |n|
  data: n
  @type: "Foo"

print! koto.type (foo 42)
check! Foo
```

### `@meta`

Named meta entries can be inserted into the map, which will be accessible via
`.` access while not being listed as one of the map's main entries.

```koto
foo = |n|
  data: n
  @meta hello: "Hello!"
  @meta get_info: |self| 
    info = match self.data 
      0 then "zero"
      n if n < 0 then "negative"
      else "positive"
    "${self.data} is $info"

x = foo -1
print! x.hello
check! Hello!

print x.get_info()
check! -1 is negative

print x.keys().to_tuple()
check! ('data')
```

## Sharing Meta Maps

If you're creating lots of values, then it will likely be more efficient to create a single map with the meta logic, and then share it between values using [`Map.with_meta_map`](../../core/map/#with-meta-map).

```koto
# Create an empty map for global values 
globals = {}

# Define a function that makes a Foo
foo = |data|
  # Make a map that contains `data`, 
  # and the meta map from foo_meta
  {data}.with_meta_map globals.foo_meta

# Define some meta behaviour in foo_meta
globals.foo_meta =
  # Override the + operator
  @+: |self, other| foo self.data + other.data

  # Define how the value should be displayed 
  @display: |self| "Foo (${self.data})"

print! (foo 10) + (foo 20)
check! Foo (30)
```
