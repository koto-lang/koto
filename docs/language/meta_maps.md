# Objects and Metamaps

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

## Meta Operators

All of the binary arithmetic and logic operators (`*`, `<`, `>=`, etc) can be 
implemented following this pattern.

Additionally, the following metakeys can also be defined:

### `@negate`

The `@negate` metakey overrides the negation operator.

```koto
foo = |n|
  data: n
  @negate: || foo -self.data

x = -foo(100)
print! x.data
check! -100
```

### `@not`

The `@not` metakey overrides the `not` operator.

```koto
foo = |n|
  data: n
  @not: || self.data == 0

print! not (foo 10)
check! false
```

### `@[]`

The `@[]` metakey defines how indexing the object with `[]` should behave.

```koto
foo = |n|
  data: n
  @[]: |index| self.data + index

print! (foo 10)[7]
check! 17
```

### `@||`

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

### `@iterator`

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


### `@next`

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

### `@next_back`

The `@next_back` metakey is used by
[`iterator.reversed`](../core_lib/iterator/#reversed) when producing a reversed
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

### `@display`

The `@display` metakey defines how the object should be represented when
displaying the object as a string.

```koto
foo = |n|
  data: n
  @display: || 'Foo(${self.data})'

print! foo 42
check! Foo(42)

x = foo -1
print! "The value of x is '$x'"
check! The value of x is 'Foo(-1)'
```

### `@type`

The `@type` metakey takes a string as a value which is used when checking the
value's type, e.g. with [`koto.type`](../core_lib/koto/#type)

```koto
foo = |n|
  data: n
  @type: "Foo"

print! koto.type (foo 42)
check! Foo
```

### `@base`

Objects can inherit properties and behavior from other values, 
establishing a _base value_ through the `@base` metakey.
This allows objects to share common functionality while maintaining their own
unique attributes.

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
    "${self.data} is $info"

x = foo -1
print! x.hello
check! Hello!

print x.get_info()
check! -1 is negative

print map.keys(x).to_tuple()
check! ('data')
```

## Sharing Metamaps

Metamaps can be shared between objects by using 
[`Map.with_meta`](../core_lib/map/#with-meta), which helps to avoid inefficient
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
  @display: || "Foo(${self.data})"

print! (foo 10) + (foo 20)
check! Foo(30)
```
