# test

A collection of utilities for writing tests.

## Writing tests

To add tests to a Koto script, create a Map named `@tests`, and then any
functions in the Map tagged with `@test` will be run as tests.

If a function named `@pre_test` is in the `@tests` Map, then it will be run
before each test. Similarly, if a function named `@post_test` is present then it
will be run after each test.

These functions are useful if some setup work is needed before each test, and
then maybe there's some cleanup work to do after the test has finished.

To access the result of the setup work, if the test function takes `self` as its
first argument, then the `@tests` Map itself will be passed in as `self`.

### Example

```koto
# A module's tests are defined as a map named `@tests`
@tests =
  # '@pre_test' will be run before each test
  @pre_test: |self|
    self.test_data = 1, 2, 3

  # '@post_test' will be run after each test
  @post_test: |self|
    self.test_data = null

  # Functions that are tagged with @test are automatically run as tests
  @test basic_assertions: ||
    # assert checks that its argument is true
    assert 1 > 0
    # assert_near checks that its arguments are equal, within a specied margin
    allowed_error = 0.1
    assert_near 1.3, 1.301, allowed_error

  # Instance test functions receive the tests map as `self`
  @test data_size: |self|
    # assert_eq checks that its two arguments are equal
    assert_eq self.test_data.size(), 3
    # assert_ne checks that its two arguments are not equal
    assert_ne self.test_data.size(), 1
```

## Running tests

### Enabling tests in the runtime

When the Koto runtime has the `run_tests` setting enabled, then after a module
is compiled and initialized then tests will be run before calling the `main`
function.

### Enabling tests in the CLI

The `run_tests` setting can be enabled when using the `koto` CLI with
the `--tests` flag.

### Running tests from a Koto script


Tests can be run from a Koto script by calling [`test.run_tests`](#run-tests).

# Reference

## assert

```kototype
|Bool| -> Null
```

Throws a runtime error if the argument if false.

### Example

```koto
# This assertion will pass, and no error will be thrown
assert 1 < 2

# This assertion will fail and throw an error
assert 1 > 2
# error: Assertion failed
```

## assert_eq

```kototype
|Value, Value| -> Null
```

Checks the two input values for equality and throws an error if they're not
equal.

### Example

```koto
# This assertion will pass, and no error will be thrown
assert_eq 1 + 1, 2

# This assertion will fail and throw an error
assert_eq 2 + 2, 5
# error: Assertion failed, '4' is not equal to '5'
```

## assert_ne

```kototype
|Value, Value| -> Null
```

Checks the two input values for inequality and throws an error if they're equal.

### Example

```koto
# This assertion will pass, and no error will be thrown
assert_ne 1 + 1, 3

# This assertion will fail and throw an error
assert_ne 2 + 2, 4
# error: Assertion failed, '4' should not be equal to '4'
```

## assert_near

```kototype
|Number, Number| -> Null
```

```kototype
|Number, Number, Number| -> Null
```

```kototype
|Num2, Num2| -> Null
```

```kototype
|Num2, Num2, Number| -> Null
```

```kototype
|Num4, Num4| -> Null
```

```kototype
|Num4, Num4, Number| -> Null
```

Checks that the two input numbers are equal, within an allowed margin of error.

This is useful when testing floating-point operations, where the result can be
close to a target with some acceptable imprecision.

The margin of error is optional, defaulting to `1.0e-12` for `Number` and `Num2`
comparisons, and `1.0e-6` for `Num4` comparisons.

### Example

```koto
allowed_error = 0.01
# This assertion will pass, and no error will be thrown
assert_near 1.3, 1.301, allowed_error

# This assertion will fail and throw an error
assert_near 1.3, 1.32, allowed_error
# error: Assertion failed, '1.3' and '1.32' are not within 0.01 of each other

# The allowed margin of error is optional, defaulting to a very small value
assert_near 1 % 0.2, 0.2
```

## run_tests

```kototype
|Map| -> Null
```

Runs the tests contained in the map.

### Example

```koto
my_tests =
  @pre_test: |self| self.test_data = 1, 2, 3
  @post_test: |self| self.test_data = null

  @test data_size: |self| assert_eq self.test_data.size(), 3
  @test failure: |self| assert_eq self.test_data.size(), 0

try
  test.run_tests my_tests
catch error
  print "An error occurred while running my_tests:\n  {}", error
```
