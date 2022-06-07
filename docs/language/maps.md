# Maps

Maps are Koto's associative containers, containing a series of key/value entries.

They can be declared with `{}` braces, or without using indented blocks.

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

Nested Maps can be declared with additional indentation:

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

Once a Map has been created, its data is shared between instances of the Map.

```koto
a = {foo: 99, bar: -1}
z = a
z.foo = 'Hi!'
print! a.foo
check! Hi!
```

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

When the first argument in a Map's function is `self`,
then `self` will automatically be assigned as an instance of the Map that the Function's contained in.

```koto
m = 
  name: 'World'
  hello: |self| 'Hello, ${self.name}!'

print! m.hello()
check! Hello, World!

m.name = 'Friend'
print! m.hello()
check! Hello, Friend!
```

Maps can be merged together using the `+` operator.

```koto
x = {hello: 123}
y = {goodbye: 99}
print! x + y
check! {hello: 123, goodbye: 99}
```
