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

## Optional Arguments

When calling a function, any missing arguments will be replaced by `null`.

```koto
f = |a, b, c|
  print (a, b, c)

f 1
check! (1, null, null)
f 1, 2
check! (1, 2, null)
f 1, 2, 3
check! (1, 2, 3)
```

In simple cases the function can check for missing arguments by using `or`.

```koto
f = |a, b, c|
  print (a or -1, b or -2, c or -3)

f 1
check! (1, -2, -3)
```

`or` will reject `false`, so if a 
