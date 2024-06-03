# yaml

[YAML](https://yaml.org) support for Koto.

## from_string

```kototype
|String| -> Any
```

Deserializes a string containing YAML data, returning a structured Koto value.

### Example

```koto
data = r'
string: O_o

nested:
  number: -1.2

entries:
  - foo: bar
  - foo: baz
'

result = yaml.from_string data
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

Returns a string containing the input value serialized as YAML data.

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

print! yaml.to_string data
check! ---
check! string: ">_>"
check! nested:
check!   number: 99
check! entries:
check!   - foo: bar
check!   - foo: baz
check! 
```
