# Basic Types

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

Parentheses can be used to group expressions.

```koto
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

The `null` keyword is used to declare a Null value,
which is used to represent the absence of a value.

```koto
print! null
check! null
```

## Truthiness

When `null` is encountered in a boolean context, it evaluates as `false`.

Every value except for `false` and `null` evaluates as `true`.

```koto
print! not null
check! true

print! null or 42
check! 42
```
