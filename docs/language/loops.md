# Loops

## for

`for` loops can be used to iterate over any iterable value.

```koto
for n in [10, 20, 30]
  print n
check! 10
check! 20
check! 30
```

## break

Loops can be stopped early with `break`.

```koto
x = for n in (11, 22, 33, 44, 55)
  if n > 30 
    break n
print! x
check! 33
```

## continue 

`continue` can be used to skip ahead to the next iteration of the loop.

```koto
for n in (-2, -1, 1, 2)
  if n < 0
    continue
  print n
check! 1
check! 2
```

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
  print z.pop()
check! 3
check! 2
check! 1
```

## loop

`loop` creates a loop that will repeat indefinitely.

```koto
x = 0
y = loop
  x += 1
  if x > 4
    break x
print! y
check! 5
```

