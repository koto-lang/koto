# toml

[TOML](https://toml.io) support for Koto.

## from_string

```kototype
|String| -> Any
```

Deserializes a string containing TOML data, returning a structured Koto value.

### Example

```koto
data = r"
string = 'O_o'

[nested]
number = -1.2

[[entries]]
foo = 'bar'

[[entries]]
foo = 'baz'
"

result = toml.from_string data
print! result.string
check! O_o
print! result.nested.number
check! -1.2
print! result.entries[0].foo
check! bar
print! result.entries[1].foo
check! baz
```

## to_string

```kototype
|Any| -> String
```

Returns a string containing the input value serialized as TOML data.

### Example

```koto
data = 
  string: '>_>'
  nested:
    number: 99
  entries: (
    {foo: 'bar'},
    {foo: 'baz'},
  )

print! toml.to_string data
check! string = '>_>'
check! 
check! [nested]
check! number = 99
check! 
check! [[entries]]
check! foo = 'bar'
check! 
check! [[entries]]
check! foo = 'baz'
check! 
```
