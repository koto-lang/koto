# Language Basics

Koto programs contain a series of expressions that are evaluated by Koto's runtime.

As an example, this simple script prints a friendly greeting.

```koto,skip_run
name = 'World'
print 'Hello, $name!'
```

To see this in action, save the script as `hello.koto`, and then run it with 
`koto hello.koto`. 
Alternatively you can try entering the expressions one at a time in the REPL.

## Comments

Single-line comments start with a `#`. 

```koto
# This is a comment, everything until the end of the line is ignored.
```

Multi-line comments start with `#-` and end with `-#`.

```koto
#- 
This is a 
multi-line 
comment.
-#
```

## Numbers 

Numbers and arithmetic are expressed in a familiar way.

```koto
print! 1
check! 1

print! 1 + 1
check! 2

print! -1 - 10
check! -11

print! 3 * 4
check! 12

print! 9 / 2
check! 4.5

print! 12 % 5
check! 2
```

### Parentheses

Arithmetic operations follow the 
[conventional order of precedence][operation-order]. 
Parentheses can be used to group expressions as needed.

```koto
# Without parentheses, multiplication is performed before addition
print! 1 + 2 * 3 + 4
check! 11
# With parentheses, the additions are performed first
print! (1 + 2) * (3 + 4)
check! 21
```

## Booleans 

Booleans are declared with the `true` and `false` keywords, and combined using
the `and` and `or` operators.

```koto
print! true and false
check! false

print! true or false
check! true
```

Booleans can be negated with the `not` operator.

```koto
print! not true
check! false

print! not false
check! true
```

Values can be compared for equality with the `==` and `!=` operators.

```koto
print! 1 + 1 == 2
check! true

print! 99 != 100
check! true
```

## Null

The `null` keyword is used to declare a value of type `Null`,
which indicates the absence of a value.

```koto
print! null
check! null
```

### Truthiness

In boolean contexts (such as logical operations), `null` is treated as being
equivalent to `false`. Every other value in Koto evaluates as `true`.

```koto
print! not null
check! true

print! null or 42
check! 42
```

## Assigning Variables

Values are assigned to named identifiers with `=`, and can be freely reassigned.
Named values like this are known as _variables_.

```koto
# Assign the value `42` to `x`
x = 42
print! x
check! 42

# Replace the existing value of `x` 
x = true
print! x
check! true
```

Compound assignment operators are also available. 
For example, `x *= y` is a simpler way of writing `x = x * y`.

```koto
a = 100
print! a += 11
check! 111
print! a
check! 111

print! a *= 10
check! 1110
print! a
check! 1110
```

## Debug

The `debug` keyword allows you to quickly display a value while working on a 
program. 

It prints the result of an expression, prefixed with its line number and the
original expression as a string.

```koto
x = 10 + 20
debug x / 10
check! [2] x / 10: 3.0
```

When using `debug`, the displayed value is also the result of the expression, 
which can be useful if you want to quickly get feedback during development.

```koto
x = debug 2 + 2
check! [1] 2 + 2: 4
print! x
check! 4
```

[operation-order]: https://en.wikipedia.org/wiki/Order_of_operations#Conventional_order
