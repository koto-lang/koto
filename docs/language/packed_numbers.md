# Packed Numbers

Koto includes two types, `Num2` and `Num4` which combine multiple numbers in a 
single value.

Element-wise arithmetic operations between Num2s and Num4s are available,
while operations with a Number apply the operation to each element.

## Num2

A `Num2` in Koto is a packed pair of 64bit floating-point numbers,
which can be useful when dealing with operations that require pairs of numbers,
like 2D coordinates.

```koto
x = make_num2 1, 2
y = make_num2 3, 4
print! x + y
check! num2(4, 6)

print! x[0] + y[0]
check! 4.0

print! x + 10
check! num2(11, 12)
```

## Num4

A `Num4` in Koto is a packed group of 32bit floating-point numbers,
which can be useful when working with operations that require 3 or 4 values,
like 3D coordinates, or RGB/RGBA colour values.

```koto
x = make_num4 1, 2, 3, 4
y = make_num4 5, 6, 7, 8

print! x[2]
check! 3.0

print! x + y
check! num4(6, 8, 10, 12)

print! x * 0.5
check! num4(0.5, 1, 1.5, 2)
```
