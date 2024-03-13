# Testing

Koto includes a simple testing framework that help you to check that your code 
is behaving as you expect through automated checks.

## Assertions

The core library includes a collection of _assertion_ functions in the 
[`test` module](../core_lib/test),
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

Tests can be organized by collecting `@test` functions in an object. 

The tests can then be run manually with 
[`test.run_tests`](../core_lib/test#run-tests).
For automatic testing, see the description of exporting `@tests` in the
[following section](./modules#tests-and-main).

```koto
basic_tests = 
  @test add: || assert_eq 1 + 1, 2 
  @test subtract: || assert_eq 1 - 1, 0 

test.run_tests basic_tests
```

For setup and cleanup operations shared across tests, 
`@pre_test` and `@post_test` metakeys can be implemented.
`@pre_test` will be run before each `@test`, and `@post_test` will be run after.

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
