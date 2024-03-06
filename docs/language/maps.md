# Maps

_Maps_ in Koto are containers that contain a series of 
_entries_ with _keys_ that correspond to [associated][associated] _values_.

Maps can be created using _inline syntax_ with `{}` braces:

```koto
m = {apples: 42, oranges: 99, lemons: 63}
print! m.oranges
check! 99
```

...or with _block syntax_ using indented entries.

```koto
m = 
  apples: 42
  oranges: 99
  lemons: 63
print! m.apples
check! 42
```

The `+` operator allows maps to be joined together, creating a new map that
combines their entries. 

```koto
a = {red: 100, blue: 150}
b = {green: 200, blue: 99}
c = a + b
print! c
check! {red: 100, blue: 99, green: 200}
```

## Shorthand Values

Koto supports a shorthand notation when creating maps with inline syntax. 
If a value isn't provided for a key, then Koto will look for a value in scope
that matches the key's name, and if one is found then it will be used as that
entry's value.

```koto
bar = 'hi!'
m = {foo: 42, bar}
print! m.bar
check! hi!
```

## Data Sharing

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

## Maps and Self

Maps can store any type of value, including functions, 
which provides a convenient way to group functions together.

```koto
m = 
  hello: |name| 'Hello, $name!'
  bye: |name| 'Bye, $name!'

print! m.hello 'World'
check! Hello, World!
print! m.bye 'Friend'
check! Bye, Friend!
```

`self` is a special identifier that refers to the instance of the container in
which the function is contained. 

`self` allows functions to access and modify data from the map, 
enabling object-like behaviour.

```koto
m = 
  name: 'World'
  say_hello: || 'Hello, ${self.name}!'

print! m.say_hello()
check! Hello, World!

m.name = 'Friend'
print! m.say_hello()
check! Hello, Friend!
```

[associated]: https://en.wikipedia.org/wiki/Associative_array
