# Conditional Expressions

Koto includes several ways of producing values that depend on _conditions_.

## if

`if` expressions come in two flavours; single-line:

```koto
x = 99
if x % 2 == 0 then print 'even' else print 'odd'
check! odd
```

...and multi-line using indented blocks:

```koto
x = 24
if x < 0
  print 'negative'
else if x > 24
  print 'no way!'
else 
  print 'ok'
check! ok
```

The result of an `if` expression is the final expression in the branch that gets
executed.

```koto
x = if 1 + 1 == 2 then 3 else -1
print! x, x
check! (3, 3)

# Assign the result of the if expression to foo
foo = if x > 0
  y = x * 10
  y + 3
else 
  y = x * 100
  y * y

print! foo, foo 
check! (33, 33)
```

## switch

`switch` expressions can be used as a cleaner alternative to 
`if`/`else if`/`else` cascades.

```koto
fib = |n|
  switch
    n <= 0 then 0
    n == 1 then 1
    else (fib n - 1) + (fib n - 2)

print! fib 7
check! 13
```

## match

`match` expressions can be used to match a value against a series of patterns, 
with the matched pattern causing a specific branch of code to be executed.

Patterns can be literals or identifiers. An identifier will accept any value, 
so they're often used with `if` conditions to refine the match.

```koto
print! match 40 + 2
  0 then 'zero'
  1 then 'one'
  x if x < 10 then 'less than 10: $x'
  x if x < 50 then 'less than 50: $x'
  x then 'other: $x'
check! less than 50: 42
```

The `_` wildcard match can be used to match against any value 
(when the matched value itself can be ignored), 
and `else` can be used for fallback branches.

```koto
fizz_buzz = |n|
  match n % 3, n % 5
    0, 0 then "Fizz Buzz"
    0, _ then "Fizz"
    _, 0 then "Buzz"
    else n

print! (10, 11, 12, 13, 14, 15)
  .each |n| fizz_buzz n
  .to_tuple()
check! ('Buzz', 11, 'Fizz', 13, 14, 'Fizz Buzz')
```

List and tuple entries can be matched against, 
with `...` available for capturing the rest of the sequence.

```koto
print! match ['a', 'b', 'c'].extend [1, 2, 3]
  ['a', 'b'] then "A list containing 'a' and 'b'"
  [1, ...] then "Starts with '1'"
  [..., 'y', last] then "Ends with 'y' followed by '$last'"
  ['a', x, others...] then
    "Starts with 'a', followed by '$x', then ${others.size()} others"
  unmatched then "other: $unmatched"
check! Starts with 'a', followed by 'b', then 4 others
```
