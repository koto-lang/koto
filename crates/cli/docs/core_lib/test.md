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

Runs the tests contained in the map.

### Example

```koto,skip_check
my_tests =
  @pre_test: || self.test_data = 1, 2, 3
  @post_test: || self.test_data = null

  @test data_size: || assert_eq self.test_data.size(), 3
  @test failure: || assert_eq self.test_data.size(), 0

try
  test.run_tests my_tests
catch error
  print "An error occurred while running my_tests:\n  {error}"
```
