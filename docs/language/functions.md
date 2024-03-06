# Functions

Functions in Koto are created using a pair of vertical bars (`|`), 
with the function's _arguments_ listed between the bars. 
The _body_ of the function follows the vertical bars.

```koto
hi = || 'Hello!'
add = |x, y| x + y
```

Functions are _called_ with arguments contained in `()` parentheses.
The body of the function is evaluated and the result is returned to the caller.

```koto
hi = || 'Hello!'
print! hi()
check! Hello!

add = |x, y| x + y
print! add(50, 5)
check! 55
```

The parentheses for arguments when calling a function are optional and can be 
ommitted in simple expressions.

```koto
square = |x| x * x
print! square 8
check! 64

add = |x, y| x + y
print! add 2, 3
check! 5

# Equivalent to square(add(2, 3))
print! square add 2, 3 
check! 25
```

A function's body can be an indented block, where the last 
expression in the body is evaluated as the function's result.

```koto
f = |x, y, z|
  x *= 100
  y *= 10
  x + y + z
print! f 2, 3, 4
check! 234
```

## Return 

When the function should be exited early, the `return` keyword can be used.

```koto
f = |n|
  return 42
  # This expression won't be reached
  n * n
print! f -1
check! 42
```

If a value isn't provided to `return`, then the returned value is `null`.

```koto
f = |n|
  return
  n * n
print! f 123
check! null
```

## Function Piping

The pipe operator (`>>`) can be used to pass the result of one function to 
another, working from left to right. This is known as _function piping_, 
and can aid readability when working with a long chain of function calls.

```koto
add = |x, y| x + y
multiply = |x, y| x * y
square = |x| x * x

# Chained function calls can be a bit hard to follow for the reader.
x = multiply 2, square add 1, 3
print! x
check! 32

# Parentheses don't help all that much...
x = multiply(2, square(add(1, 3)))
print! x
check! 32

# Piping allows for a left-to-right flow of results.
x = add(1, 3) >> square >> multiply 2
print! x
check! 32

# Call chains can also be broken across lines.
x = add 1, 3
  >> square 
  >> multiply 2
print! x
check! 32
```
