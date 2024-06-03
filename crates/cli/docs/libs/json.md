# json

[JSON](https://www.json.org) support for Koto.

## from_string

```kototype
|String| -> Any
```

Deserializes a string containing JSON data, returning a structured Koto value.

### Example

```koto
data = r'
{
  "string": "O_o",
  "nested": {
    "number": -1.2
  },
  "entries": [
    {
      "foo": "bar"
    },
    {
      "foo": "baz"
    }
  ]
}'

result = json.from_string data
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

Returns a string containing the input value serialized as JSON data.

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

print! json.to_string data
check! {
check!   "string": ">_>",
check!   "nested": {
check!     "number": 99
check!   },
check!   "entries": [
check!     {
check!       "foo": "bar"
check!     },
check!     {
check!       "foo": "baz"
check!     }
check!   ]
check! }
```
