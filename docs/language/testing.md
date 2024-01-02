# Testing

Koto includes some features that help you to automatically check that your code is behaving as you expect.

## Assertions

A collection of assertion functions are available in the [`test` core library module](../../core/test),
which are included by default in the [prelude](./prelude).

```koto
try 
  assert 1 + 1 == 3
catch error
  print 'An assertion failed'
check! An assertion failed

try 
  assert_eq 'hello', 'goodbye'
catch error
  print 'An assertion failed'
check! An assertion failed
```

## Organizing Tests

Tests can be organized in a Map by defining `@test` functions. 

The runtime can be configured to automatically run tests when importing modules, 
e.g. the CLI will run module tests when the `--import_tests` option is used.

The tests can then be run with [`test.run_tests`](../../core/test#run-tests).

```koto
basic_tests = 
  @test add: || assert_eq 1 + 1, 2 
  @test subtract: || assert_eq 1 - 1, 0 

test.run_tests basic_tests
```

`@pre_test` and `@post_test` functions can be used to define shared setup and cleanup steps.

```koto
make_x = |n|
  data: n
  @+: |other| make_x self.data + other.data
  @-: |other| make_x self.data - other.data

x_tests =
  @pre_test: || 
    self.x1 = make_x 100
    self.x2 = make_x 200

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
check! Testing subtraction
check! About to fail
check! A test failed
```

