# num4

A Num4 in Koto is a packed group of 32bit floating-point numbers,
which can be useful when working with operations that require 3D coordinates,
or RGBA colour values.

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

- [length](#length)
- [max](#max)
- [min](#min)
- [normalize](#normalize)
- [product](#product)
- [sum](#sum)

## length

`|Num4| -> Float`

Returns the length of the vector represented by the Num4's elements.

### Example

```koto
x = num4(2, -2, 2, -2)
x.length()
# 4
```

## max

`|Num4| -> Float`

Returns the value of the largest element in the Num4.

### Example

```koto
x = num4(10, 20, -50, -10)
x.max()
# 20
```

## min

`|Num4| -> Float`

Returns the value of the smallest element in the Num4.

### Example

```koto
x = num4(10, 20, -50, -10)
x.min()
# -50
```

## normalize

`|Num4| -> Num4`

Returns a Num4 with the same direction as the input,
with its length normalized to 1.

### Example

```koto
x = num4(2, -2, 2, -2)
x.normalize()
# num4(0.5, -0.5, 0.5, 0.5)
```

## product

`|Num4| -> Float`

Returns the result of multiplying the Num4's elements together.

### Example

```koto
x = num4(10, 20, -50, -10)
x.product()
# 100000
```

## sum

`|Num4| -> Float`

Returns the result of adding the Num4's elements together.

### Example

```koto
x = num4(10, 20, 30, 40)
x.sum()
# 100
```
