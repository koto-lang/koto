# Loops

Koto includes several ways of evaluating expressions repeatedly in a loop.

## while

`while` loops continue to repeat _while_ a condition is true.

```koto
x = 0
while x < 5
  x += 1
print! x
check! 5
```

## until

`until` loops continue to repeat _until_ a condition is true.

```koto
z = [1, 2, 3]
until z.is_empty()
  # Remove the last element of the list
  print z.pop()
check! 3
check! 2
check! 1
```

## break

Loops can be terminated with the `break` keyword.

```koto
x = 0
while x < 100000
  if x >= 3
    # Break out of the loop when x is greater or equal to 3
    break
  x += 1
print! x
check! 3
```

A value can be provided to `break`, which is then used as the result of the loop.

```koto
x = 0
y = while x < 100000
  if x >= 3
    # Break out of the loop, providing x + 100 as the loop's result
    break x + 100
  x += 1
print! y
check! 103
```

## loop

`loop` creates a loop that will repeat indefinitely.

```koto
x = 0
y = loop
  x += 1
  # Stop looping when x is greater than 4
  if x > 4
    break x * x
print! y
check! 25
```

## for

`for` loops are repeated for each element in a sequence, 
such as a list or tuple.

```koto
for n in [10, 20, 30]
  print n
check! 10
check! 20
check! 30
```

## continue 

`continue` skips the remaining part of a loop's body and proceeds with the next repetition of the loop.

```koto
for n in (-2, -1, 1, 2)
  # Skip over any values less than 0
  if n < 0
    continue
  print n
check! 1
check! 2
```
