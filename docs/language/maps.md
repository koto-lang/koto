# Maps

Maps are Koto's associative containers, containing a series of key/value entries.

They can be declared with `{}` braces (known as _inline syntax_), or by using indented blocks (known as _block syntax_).

With braces:

```koto
m = {apples: 42, oranges: 99, lemons: 63}
print! m.oranges
check! 99
```

...and as an indented block:

```koto
m = 
  apples: 42
  oranges: 99
  lemons: 63
print! m.apples
check! 42
```

Nested maps can be declared with additional indentation:

```koto
m =
  hello:
    world: 99
    everybody: 123
    to:
      you: -1
print! m.hello.world
check! 99
print! m.hello.to.you
check! -1
```

Maps can be joined together with the `+` operator.

```koto
a = {red: 100, blue: 150}
b = {green: 200}
c = a + b
print! c
check! {red: 100, blue: 150, green: 200}
```

## Shorthand Values

When using inline syntax, if there's a value available that matches a key's name, then declaring the value is optional.

When using inline syntax, declaring a value for a key is optional. The runtime will look for a value that matches the key's name, and then copy it into the map.

```koto
bar = 'hi!'
m = {foo: 42, bar, baz: -1}
print! m.bar
check! hi!
```

## Data Sharing

Once a map has been created, any additional instances of the map share the same data.

```koto
a = {foo: 99, bar: -1}
print! a.foo
check! 99
z = a
z.foo = 'Hi!'
print! a.foo
check! Hi!
```

## Maps and Functions

Any value type can be stored in Maps, including Functions.

```koto
m = 
  hello: |name| 'Hello, $name!'
  bye: |name| 'Bye, $name!'

print! m.hello 'World'
check! Hello, World!
print! m.bye 'Friend'
check! Bye, Friend!
```

`self` is a special identifier that refers to the instance of the map that the function is contained in. 

```koto
m = 
  name: 'World'
  hello: || 'Hello, ${self.name}!'

print! m.hello()
check! Hello, World!

m.name = 'Friend'
print! m.hello()
check! Hello, Friend!
```
