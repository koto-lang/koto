# num2

A `Num2` in Koto is a packed pair of 64bit floating-point numbers,
which can be useful when dealing with operations that require pairs of numbers,
like 2D coordinates.

Element-wise arithmetic operations between Num2s are available,
while operations with Numbers apply the number to each element.

## Example

```koto
x = make_num2 1, 2
y = make_num2 3, 4
x + y
# num2(4, 6)

x[0] + y[0]
# 4

x + 10
# num2(11, 12)
```

# Reference

- [angle](#angle)
- [length](#length)
- [make_num2](#make_num2)
- [max](#max)
- [min](#min)
- [normalize](#normalize)
- [product](#product)
- [sum](#sum)
- [with](#with)
- [x](#x)
- [y](#y)

## angle

`|Num2| -> Float`

Returns the angle in radians of the vector represented by the Num2's elements.

### Note

`make_num2(x, y).angle()` is equivalent to `y.atan2 x`

### Example

```koto
x = make_num2 1, 1
x.angle()
# π/4
```

## length

`|Num2| -> Float`

Returns the length of the vector represented by the Num2's elements.

### Example

```koto
x = make_num2 3, 4
x.length()
# 5
```

## make_num2

`|Number| -> Num2`
`|Number, Number| -> Num2`
`|Num2| -> Num2`
`|Iterable| -> Num2`

Makes a Num2 from the provided values.

### Example

```koto
make_num2 1
# num2(1, 1)

make_num2 3, 4
# num2(3, 4)

make_num2 [11, 12]
# num2(11, 12)
```

## max

`|Num2| -> Float`

Returns the value of the largest element in the Num2.

### Example

```koto
x = make_num2(10, 20)
x.max()
# 20
```

## min

`|Num2| -> Float`

Returns the value of the smallest element in the Num2.

### Example

```koto
x = make_num2(10, 20)
x.min()
# 10
```

## normalize

`|Num2| -> Num2`

Returns a Num2 with the same direction as the input,
with its length normalized to 1.

### Example

```koto
x = make_num2(3, 4)
x.normalize()
# num2(0.6, 0.8)
```

## product

`|Num2| -> Float`

Returns the result of multiplying the Num2's elements together.

### Example

```koto
x = make_num2(10, 20)
x.product()
# 300
```

## sum

`|Num2| -> Float`

Returns the result of adding the Num2's elements together.

### Example

```koto
x = make_num2(10, 20)
x.sum()
# 30
```

## with

`|Num2, index: Number, value: Number| -> Num2`

Returns a Num2 with the element at `index` replaced with `value`.

### Example

```koto
x = make_num2(10, 20)
x.with 0, 99
# num2(99, 20)
x.with 1, -1
# num2(10, -1)
```

## x

`|Num2| -> Float`

Returns the first element of the Num2.

### Example

```koto
n = make_num2 10, 20
n.x()
# 10
```

## y

`|Num2| -> Float`

Returns the second element of the Num2.

### Example

```koto
n = make_num2 10, 20
n.y()
# 20
```
