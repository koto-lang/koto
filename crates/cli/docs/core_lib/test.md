# test

A collection of utilities for writing tests.

## assert

```kototype
|Bool| -> Null
```

Throws a runtime error if the argument if false.

### Example

```koto,skip_check
# This assertion will pass, and no error will be thrown
assert 1 < 2

# This assertion will fail and throw an error
try
  assert 1 > 2
catch error
  print error
```

## assert_eq

```kototype
|a: Any, b: Any| -> Null
```

Checks the two input values for equality and throws an error if they're not
equal.

### Example

```koto,skip_check
# This assertion will pass, and no error will be thrown
assert_eq 1 + 1, 2

# This assertion will fail and throw an error
try
  assert_eq 2 + 2, 5
catch error
  print error
```

## assert_ne

```kototype
|a: Any, b: Any| -> Null
```

Checks the two input values for inequality and throws an error if they're equal.

### Example

```koto,skip_check
# This assertion will pass, and no error will be thrown
assert_ne 1 + 1, 3

# This assertion will fail and throw an error
try
  assert_ne 2 + 2, 4
catch error
  print error
```

## assert_near

```kototype
|a: Number, b: Number| -> Null
```

```kototype
|a: Number, b: Number, error_margin: Number| -> Null
```

Checks that the two input numbers are equal, within an allowed margin of error.

This is useful when testing floating-point operations, where the result can be
close to a target with some acceptable imprecision.

The margin of error is optional, defaulting to `1.0e-12`.

### Example

```koto,skip_check
allowed_error = 0.01
# This assertion will pass, and no error will be thrown
assert_near 1.3, 1.301, allowed_error

# This assertion will fail and throw an error
try
  assert_near 1.3, 1.32, allowed_error
catch error
  print error
# error: Assertion failed, '1.3' and '1.32' are not within 0.01 of each other

# The allowed margin of error is optional, defaulting to a very small value
assert_near 1 % 0.2, 0.2
```

## run_tests

```kototype
|tests: Map| -> Null
```

Runs the `@test` functions contained in the map.

`@pre_test` and `@post_test` functions can be implemented in the same way as when exporting
[module tests](../language_guide.md#module-tests).
`@pre_test` will be run before each `@test`, and `@post_test` will be run after.


### Example

```koto
make_x = |n|
  data: n
  @+: |other| make_x self.data + other.data
  @-: |other| make_x self.data - other.data

x_tests =
  @pre_test: ||
    self.x1 = make_x 100
    self.x2 = make_x 200

  @post_test: ||
    print 'Test complete'

  @test addition: ||
    print 'Testing addition'
    assert_eq self.x1 + self.x2, make_x 300

  @test subtraction: ||
    print 'Testing subtraction'
    assert_eq self.x1 - self.x2, make_x -100

  @test failing_test: ||
    print 'About to fail'
    assert false

try
  test.run_tests x_tests
catch _
  print 'A test failed'
check! Testing addition
check! Test complete
check! Testing subtraction
check! Test complete
check! About to fail
check! A test failed
```
