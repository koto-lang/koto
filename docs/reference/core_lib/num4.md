# num4

A Num4 in Koto is a packed group of 32bit floating-point numbers,
which can be useful when working with operations that require 3D coordinates,
RGBA colour values.

Element-wise arithmetic operations between Num4s are available,
while operations with Numbers apply the number to each element.

## Example

```koto
x = num4 1, 2, 3, 4
y = num4 5, 6, 7, 8
x + y
# num4(6, 8, 10, 12)

x[2]
# 10

x * 0.5
# num4(0.5, 1, 1.5, 2)

x[0..2] = -1
x
# num4(-1, -1, 10, 12)
```

# Reference

## sum

`|Num4| -> Float`

Returns the result of adding the Num4's elements together.

### Example

```koto
x = num4(10, 20, 30, 40)
x.sum()
# 100
```
