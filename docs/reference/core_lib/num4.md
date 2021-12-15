# num4

A Num4 in Koto is a packed group of 32bit floating-point numbers,
which can be useful when working with operations that require 3D coordinates,
or RGBA colour values.

Element-wise arithmetic operations between Num4s are available,
while operations with Numbers apply the number to each element.

## Example

```koto
x = make_num4 1, 2, 3, 4
y = make_num4 5, 6, 7, 8
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
- [make_num4](#make_num4)
- [max](#max)
- [min](#min)
- [normalize](#normalize)
- [product](#product)
- [sum](#sum)
- [r](#r)
- [g](#g)
- [b](#b)
- [a](#a)
- [x](#x)
- [y](#y)
- [z](#z)
- [w](#w)

## length

`|Num4| -> Float`

Returns the length of the vector represented by the Num4's elements.

### Example

```koto
x = make_num4(2, -2, 2, -2)
x.length()
# 4
```

## make_num4

`|Number| -> Num4`
`|Number, Number| -> Num4`
`|Number, Number, Number| -> Num4`
`|Number, Number, Number, Number| -> Num4`
`|Num2| -> Num4`
`|Num4| -> Num4`
`|Iterable| -> Num4`

Makes a Num4 from the provided values.

### Example

```koto
make_num4 1
# num4(1, 1, 1, 1)

make_num4 3, 4
# num4(3, 4, 0, 0)

make_num4 5, 6, 7, 8
# num4(5, 6, 7, 8)

make_num4 [11, 12, 13, 14]
# num4(11, 12, 13, 14)
```

## max

`|Num4| -> Float`

Returns the value of the largest element in the Num4.

### Example

```koto
x = make_num4(10, 20, -50, -10)
x.max()
# 20
```

## min

`|Num4| -> Float`

Returns the value of the smallest element in the Num4.

### Example

```koto
x = make_num4(10, 20, -50, -10)
x.min()
# -50
```

## normalize

`|Num4| -> Num4`

Returns a Num4 with the same direction as the input,
with its length normalized to 1.

### Example

```koto
x = make_num4(2, -2, 2, -2)
x.normalize()
# num4(0.5, -0.5, 0.5, 0.5)
```

## product

`|Num4| -> Float`

Returns the result of multiplying the Num4's elements together.

### Example

```koto
x = make_num4(10, 20, -50, -10)
x.product()
# 100000
```

## sum

`|Num4| -> Float`

Returns the result of adding the Num4's elements together.

### Example

```koto
x = make_num4(10, 20, 30, 40)
x.sum()
# 100
```

## r

`|Num4| -> Float`

Returns the first element of the Num4.

This can be useful when using a Num4 as a colour value, and want to access its
'red' component.

### Example

```koto
n = make_num4 10, 20, 30, 40
n.r()
# 10
```

## g

`|Num4| -> Float`

Returns the second element of the Num4.

This can be useful when using a Num4 as a colour value, and want to access its
'green' component.

### Example

```koto
n = make_num4 10, 20, 30, 40
n.g()
# 20
```

## b

`|Num4| -> Float`

Returns the third element of the Num4.

This can be useful when using a Num4 as a colour value, and want to access its
'blue' component.

### Example

```koto
n = make_num4 10, 20, 30, 40
n.b()
# 30
```

## a

`|Num4| -> Float`

Returns the fourth element of the Num4.

This can be useful when using a Num4 as a colour value, and want to access its
'alpha' component.

### Example

```koto
n = make_num4 10, 20, 30, 40
n.w()
# 40
```

## x

`|Num4| -> Float`

Returns the first element of the Num4.

This can be useful when using a Num4 as a 3D or 4D vector, and want to access
its `x` component.

### Example

```koto
n = make_num4 10, 20, 30, 40
n.x()
# 10
```

## y

`|Num4| -> Float`

Returns the second element of the Num4.

### Example

```koto
n = make_num4 10, 20, 30, 40
n.y()
# 20
```

This can be useful when using a Num4 as a 3D or 4D vector, and want to access
its `y` component.

## z

`|Num4| -> Float`

Returns the third element of the Num4.

This can be useful when using a Num4 as a 3D or 4D vector, and want to access
its `z` component.

### Example

```koto
n = make_num4 10, 20, 30, 40
n.z()
# 30
```

## w

`|Num4| -> Float`

Returns the fourth element of the Num4.

This can be useful when using a Num4 as a 4D vector, and want to access its `w`
component.

### Example

```koto
n = make_num4 10, 20, 30, 40
n.w()
# 40
```
