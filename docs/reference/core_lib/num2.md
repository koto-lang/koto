# num2

A `Num2` in Koto is a packed pair of 64bit floating-point numbers,
which can be useful when dealing with operations that require pairs of numbers,
like 2D coordinates.

Element-wise arithmetic operations between Num2s are available,
while operations with Numbers apply the number to each element.

## Example

```koto
x = num2 1, 2
y = num2 3, 4
x + y
# num2(4, 6)

x + 10
# num2(11, 12)
```

# Reference

## sum

`|Num2| -> Float`

Returns the result of adding the Num2's elements together.

### Example

```koto
x = num2(10, 20)
x.sum()
# 30
```
