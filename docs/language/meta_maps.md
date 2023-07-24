# Objects and Meta Maps

The behaviour of values in Koto can be customized by making an _Object_.

An Object is created by making a map that contains at least one key with a `@` 
prefix, known as a _metakey_.

Metakeys go in to the object's _metamap_. When the runtime encounters the object
while performing operations, the object's metamap will be checked for an entry 
corresponding to the operation.

In the following example, addition and subtraction are overridden for a custom 
`Foo` object.

```koto
# foo is a function that makes Foo values
foo = |n|
  data: n

  # Overloading the addition operator
  @+: |other|
    # A new Foo is made using the result 
    # of adding the two data values together
    foo self.data + other.data

  # Overloading the subtraction operator
  @-: |other|
    foo self.data - other.data

  # Overloading the multiply-assignment operator
  @*=: |other|
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
  @negate: || foo -self.data

x = -foo(100)
print! x.data
check! -100
```

### `@not`

The `@not` meta key overloads the unary `not` operator.

```koto
foo = |n|
  data: n
  @not: || self.data == 0

print! not (foo 10)
check! false
```

### `@[]`

The `@[]` meta key defines how indexing the value with `[]` should behave.

```koto
foo = |n|
  data: n
  @[]: |index| self.data + index

print! (foo 10)[7]
check! 17
```

### `@||`

The `@||` meta key defines how the value should behave when called as a
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

### `@next`

The `@next` meta key allows for values to behave as iterators.

Whenever the runtime needs to produce an iterator from a value, it will first 
check the value for an implementation of `@next`.

The `@next` function will be called repeatedly during iteration, 
with the returned value being used as the iterator output. 
When the returned value is `null` then the iterator will stop producing output. 

```koto
no_next =
  foo: 42
  bar: 99

print! no_next.to_tuple()
check! (('foo', 42), ('bar', 99))

with_next = 
  start: 10
  end: 15
  @next: || 
    if self.start < self.end
      result = self.start
      self.start += 1
      result
    else 
      null
  
print! with_next.to_tuple()
check! (10, 11, 12, 13, 14)
```

### `@next_back`

The `@next_back` meta key is used by
[`iterator.reversed`](../../core/iterator/#reversed) when producing a reversed
iterator. 

An implementation of `@next_back` is only looked for if `@next` is also 
implemented.

```koto
iter =
  foo: 0
  @next: || self.foo += 1
  @next_back: || self.foo -= 1

print! iter
  .skip 3
  .reversed()
  .take 3
  .to_tuple()
check! (2, 1, 0)
```

### `@iterator`

The `@iterator` meta key defines how iterators should be created when the value 
is used in an iterable context. 
The function returns an iterable value that is then used during iterator 
operations.

```koto
foo = |n|
  @iterator: || 
    yield n + 1
    yield n + 2
    yield n + 3

print! (foo 0).to_tuple()
check! (1, 2, 3)

print! (foo 100).to_list()
check! [101, 102, 103]
```

Note that this key will be ignored if the value also implements `@next`, 
which implies that the value is _already_ an iterator. 

### `@display`

The `@display` meta key defines how the value should be represented when
displaying the value with functions like [`io.print`](../../core/io/#print) 
or [`string.format`](../../core/string/#format).

```koto
foo = |n|
  data: n
  @display: || 'Foo: {}'.format self.data

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

### `@base`

Objects can inherit the entries of another value by declaring it as the object's
_base value_ using the `@base` metakey.

In the following example, two kinds of animals are created that share the
`speak` function from their base value.

```koto
animal = |name|
  name: name
  speak: || '${self.noise}! My name is ${self.name}!'

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

### `@meta`

Named meta entries can be inserted into the map, which will be accessible via
`.` access while not being listed as one of the map's main entries.

```koto
foo = |n|
  data: n
  @meta hello: "Hello!"
  @meta get_info: || 
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

print map.keys(x).to_tuple()
check! ('data')
```

## Sharing Meta Maps

If you're creating lots of values, then it will likely be more efficient to create a single value with the meta logic, and then share it between values using [`Map.with_meta_map`](../../core/map/#with-meta-map).

```koto
# Create an empty map for global values 
globals = {}

# Define a function that makes a Foo
foo = |data|
  # Make a map that contains `data`, 
  # along with the meta map from foo_meta
  {data}.with_meta_map globals.foo_meta

# Define some meta behaviour in foo_meta
globals.foo_meta =
  # Override the + operator
  @+: |other| foo self.data + other.data

  # Define how the value should be displayed 
  @display: || "Foo (${self.data})"

print! (foo 10) + (foo 20)
check! Foo (30)
```
