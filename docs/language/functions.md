# Functions

Functions are values, and are created using a pair of `|` characters (often called [_pipe_](https://en.wikipedia.org/wiki/Vertical_bar#Pipe) characters in computing), with the function arguments listed between the start and end `|`. 

The _body_ of the function follows, with the result of the body used as the function's result.

```koto
hi = || 'Hello!'
add = |x, y| x + y
```

Functions are called with arguments contained in `()` parentheses.

```koto
hi = || 'Hello!'
print! hi()
check! Hello!

add = |x, y| x + y
print! add(50, 5)
check! 55
```

When calling a function with arguments, the parentheses are optional.

```koto
square = |x| x * x
print! square 8
check! 64

pow = |x, y| x.pow y
print! pow 2, 3
check! 8
```

## Return 

A function's body can be an indented block, with the last expression used as
the function's result.

```koto
f = |x, y, z|
  x *= 100
  y *= 10
  x + y + z
print! f 2, 3, 4
check! 234
```

The `return` keyword can be used to exit the function early with a result.

```koto
f = |n|
  return 42
  # This expression won't be reached
  n * n
print! f -1
check! 42
print! f 10
check! 42
```

## Function Piping

When passing the result of a function into another function, it can become a bit
hard to read, especially when a chain of functions is involved.

Using parentheses can help to disambiguate the expression for the reader, but an
alternative is available in _function piping_, where the `>>` operator can be
used to pass the result of one function to another, working from left to right.

```koto
add = |x, y| x + y
multiply = |x, y| x * y
square = |x| x * x

# Chained function calls can be a bit hard to follow
x = multiply 2, square add 1, 3
print! x
check! 32

# Parentheses don't help all that much...
x = multiply(2, square(add(1, 3)))
print! x
check! 32

# Piping allows for a left-to-right flow of results
x = add(1, 3) >> square >> multiply 2
print! x
check! 32

# Call chains can also be broken across lines 
x = 
  add 1, 3
  >> square 
  >> multiply 2
print! x
check! 32
```
