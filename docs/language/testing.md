# Testing

## Assertions

A collection of [assertion functions](../../core/test) are available. 

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

The tests can then be run with [`test.run_tests`](../../core/test#run-tests).

```koto
basic_tests = 
  @test add: || assert_eq 1 + 1, 2 
  @test subtract: || assert_eq 1 - 1, 0 

test.run_tests basic_tests
```

If a test function takes `self` as its first argument, then the test map will be passed in as `self`. 
`@pre_test` and `@post_test` functions can be used to define shared setup and cleanup steps.

```koto
make_x = |n|
  data: n
  @+: |self, other| make_x self.data + other.data
  @-: |self, other| make_x self.data - other.data

x_tests =
  @pre_test: |self| 
    self.x1 = make_x 100
    self.x2 = make_x 200

  @test addition: |self|
    print 'Testing addition'
    assert_eq self.x1 + self.x2, make_x 300

  @test subtraction: |self|
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

