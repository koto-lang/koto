# map

## clear

```kototype
|Map| -> Map
```

Clears the map by removing all of its elements, and returns the map.

### Example

```koto
x = {x: -1, y: 42}
print! x.clear()
check! {}
print! x
check! {}
```

## contains_key

```kototype
|Map, key: Any| -> Bool
```

Returns `true` if the map contains a value with the given key,
and `false` otherwise.

## extend

```kototype
|Map, new_entries: Iterable| -> Map
```

Extends the map with the output of the iterator, and returns the map.

### Example

```koto
x = {foo: 42, bar: 99}
print! x.extend {baz: 123}
check! {foo: 42, bar: 99, baz: 123}
print! x.baz 
check! 123

x = {}
print! x.extend 'abc'.each |c| c, '{c}!'
check! {a: 'a!', b: 'b!', c: 'c!'}
print! x.c
check! c!
```

### See also

- [`map.insert`](#insert)

## get

```kototype
|Map, key: Any| -> Any
```
```kototype
|Map, key: Any, default: Any| -> Any
```

Returns the value corresponding to the given key, or the provided default value
if the map doesn't contain the key.

If no default value is provided then Null is returned.

### Example

```koto
x = {hello: -1}
print! x.get 'hello'
check! -1

print! x.get 'goodbye'
check! null

print! x.get 'goodbye', 'byeeee'
check! byeeee

x.insert 99, 'xyz'
print! x.get 99
check! xyz
```

### See also

- [`map.get_index`](#get_index)

## get_index

```kototype
|Map, index: Number| -> Tuple
```
```kototype
|Map, index: Number, default: Any| -> Tuple
```

Returns the entry at the given index as a key/value tuple, or the provided
default value if the map doesn't contain an entry at that index.

If no default value is provided then Null is returned.

### Example

```koto
x = {foo: -1, bar: -2}
print! x.get_index 1
check! ('bar', -2)

print! x.get_index -99
check! null

print! x.get_index 99, 'xyz'
check! xyz
```

### See also

- [`map.get`](#get)


## get_meta

```kototype
|Map| -> Map
```

Returns a Map that contains the input's Meta Map, and no data.

### Example

```koto
my_map =
  data: 42
  @type: 'My Map'

meta = map.get_meta my_map

print! map.keys(my_map).count()
check! 1
print! map.keys(meta).count()
check! 0

print! koto.type meta
check! My Map
```

### See also

- [`map.with_meta`](#with_meta)

## insert

```kototype
|Map, key: Any, value: Any| -> Any
```

Inserts an entry into the map with the given key and value. 

```kototype
|Map, key: Any| -> Any
```

Inserts an entry into the map with the given key, and `null` as its value.

If the key already existed in the map, then the old value is returned.
If the key didn't already exist, then Null is returned.

See the [language guide](../language_guide.md#map-key-types) for a description 
of the types of values that can be used as map keys.

### Example

```koto
x = {hello: -1}

print! x.insert 'hello', 99 # -1 already exists at `hello`, so it's returned here
check! -1

print! x.hello # hello is now 99
check! 99

print! x.insert 'goodbye', 123 # No existing value at `goodbye`, so null is returned
check! null

print! x.goodbye
check! 123

print! x.insert 123, 'hi!' # Numbers can be used as map keys 
check! null

print! x.get 123
check! hi!

print! x.insert ('a', 'b'), -1 # Tuples can be used as map keys 
check! null

print! x.get ('a', 'b')
check! -1
```

### See also

- [`map.get`](#get)
- [`map.remove`](#remove)
- [`map.update`](#update)

## is_empty

```kototype
|Map| -> Bool
```

Returns `true` if the map contains no entries, otherwise `false`.

### Example

```koto
print! {}.is_empty()
check! true

print! {hello: -1}.is_empty()
check! false
```

## keys

```kototype
|Map| -> Iterator
```

Returns an iterator that iterates in order over the map's keys.

### Example

```koto
m =
  hello: -1
  goodbye: 99

x = m.keys()

print! x.next().get()
check! hello

print! x.next().get()
check! goodbye

print! x.next()
check! null
```

### See also

- [`map.values`](#values)

## remove

```kototype
|Map, key: Any| -> Any
```

Removes the entry that matches the given key.

If the entry existed then its value is returned, otherwise Null is returned.

### Example

```koto
x =
  hello: -1
  goodbye: 99

print! x.remove 'hello'
check! -1

print! x.remove 'xyz'
check! null

print! x.remove 'goodbye'
check! 99

print! x.is_empty()
check! true
```

### See also

- [`map.insert`](#insert)

## sort

```kototype
|Map| -> Map
```

Sorts the map's entries in place by key, and then returns the map.

```kototype
|
  Map, 
  sort_key: |key: Any, value: Any| -> Any
| -> Null
```

Sorts the map's entries in place based on the output of calling a 'sort' 
function for each entry, and then returns the map. 

The entry's key and value are passed into the `sort_key` function as separate 
arguments. 

The function's result is cached, so it only gets called once per entry.

### Example

```koto
x =
  hello: 123
  bye: -1
  tschüss: 99

# Sort the map by key
print! x.sort() 
check! {bye: -1, hello: 123, tschüss: 99}

# Sort the map by value
print! x.sort |_, value| value 
check! {bye: -1, tschüss: 99, hello: 123}

# Sort the map by reversed key length
print! x.sort |key, _| -(size key)
check! {tschüss: 99, hello: 123, bye: -1}
```

## update

```kototype
|Map, key: Any, updater: |Any| -> Any| -> Any
```

Updates the value associated with a given `key` by calling the `updater`
function.

If an entry exists with the given `key`, then `updater` will be called with the
existing entry's value, and the result of the function will replace the existing
value.

If no entry exists with the given `key`, then `updater` will be called with
`null`, and the result will be inserted into the map as a new entry. 

The return value is the result of calling the `updater` function.

```kototype
|Map, key: Any, default: Any, updater: |Any| -> Any| -> Any
```

This variant of `update` takes a `default` value that is provided to the
`updater` function if no entry exists with the given `key`.

### Example

```koto
x =
  hello: -1
  goodbye: 99

print! x.update 'hello', |n| n * 2
check! -2
print! x.hello
check! -2

print! x.update 'tschüss', 10, |n| n * 10
check! 100
print! x.tschüss
check! 100
```

### See also

- [`map.insert`](#insert)

## values

```kototype
|Map| -> Iterator
```

Returns an iterator that iterates in order over the map's values.

### Example

```koto
m =
  hello: -1
  goodbye: 99

x = m.values()

print! x.next().get()
check! -1

print! x.next().get()
check! 99

print! x.next()
check! null
```

### See also

- [`map.keys`](#keys)

## with_meta

```kototype
|data: Map, meta: Map| -> Map
```

Returns a new Map that contains the data from the first argument, 
along with the Meta Map from the second argument.

### Example

```koto
my_meta =
  @type: 'MyMeta'

x = {foo: 42}.with_meta my_meta

print! koto.type x
check! MyMeta
```

### See also

- [`map.get_meta`](#get-meta)
