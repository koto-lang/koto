# Meta Maps

Maps can be used to create value types with custom behaviour.

Keys with `@` prefixes go into the map's 'meta map',
which is checked when the map is encountered in operations.

```koto
make_x = |n|
  data: n

  # Overloading the addition operator
  @+: |self, other|
    # A new instance is made with the result of adding the two values together
    make_x self.data + other.data

  # Overloading the subtraction operator
  @-: |self, other|
    make_x self.data - other.data

x1 = make_x 10
x2 = make_x 20

print! (x1 + x2).data
check! 30
print! (x1 - x2).data
check! -10
```

## Meta Operators

All binary operators can be overloaded following this pattern.

The following meta functions and values can also be defined:

- `@negate`
  - Overloads the unary negation operator:
    - `@negate: |self| make_x -self.data`
- `@not`
  - Overloads the unary `not` operator:
    - `@not: |self| self.data == 0`
- `@index`
  - Overloads `[]` indexing:
    - `@index: |self, index| self.data + index`
- `@iterator`
  - Customizes how iterators will be made from the map. The function returns an
    iterator that will be used in place of the default iteration behaviour.
    - `@iterator: |self| 0..self.data`
- `@display`
  - Customizes how the map will be displayed when formatted as a string:
    - `@display: |self| "X: {}".format self.data`
- `@type`
  - Provides a String that's used when checking the map's type:
    - `@type: "X"`

## Sharing Meta Maps

If you're creating lots of values, then it will likely be more efficient to create a single map with the meta logic, and then share it between values using [`Map.with_meta_map`](../../core/map/#with-meta-map).

```koto
# Create a map for global values
globals = {}

# Define a function that makes a Foo
make_foo = |x|
  # Make a map that contains x, and the meta map from foo_meta
  {x}.with_meta_map globals.foo_meta

# Define some meta behaviour in foo_meta
globals.foo_meta =
  # Override the + operator
  @+: |self, other| make_foo self.x + other.x

  # Define how the value should be displayed 
  @display: |self| "Foo (${self.x})"

print! make_foo(10) + make_foo(20)
check! Foo (30)
```
