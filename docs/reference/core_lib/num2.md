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
print! x + y
check! num2(4, 6)

print! x[0] + y[0]
check! 4.0

print! x + 10
check! num2(11, 12)
```

# Reference

## angle

```kototype
|Num2| -> Float
```

Returns the angle in radians of the vector represented by the Num2's elements.

### Note

`make_num2(x, y).angle()` is equivalent to `y.atan2 x`

### Example

```koto
x = make_num2 1, 1
print! x.angle()
# π/4
check! 0.7853981633974483
```

## length

```kototype
|Num2| -> Float
```

Returns the length of the vector represented by the Num2's elements.

### Example

```koto
x = make_num2 3, 4
print! x.length()
check! 5.0
```

## lerp

```kototype
|a: Num2, b: Num2, t: Number| -> Num2
```

Linearly interpolates between `a` and `b` using the interpolation factor `t`.

The range (`a` -> `b`) corresponds to the value range of (`0` -> `1`) for `t`.

e.g.
- At `t` == `0`, the result is equal to `a`.
- At `t` == `1`, the result is equal to `b`.
- At other values of `t`, the result is a proportional mix of `a` and `b`.
- Values for `t` outside of (`0` -> `1`) will extrapolate from the (`a` -> `b`)
  range.

### Example

```koto
a = make_num2 0, 10
b = make_num2 10, 50

print! a.lerp b, 0
check! num2(0, 10)
print! a.lerp b, 0.5
check! num2(5, 30)
print! a.lerp b, 1
check! num2(10, 50)

print! a.lerp b, -0.5
check! num2(-5, -10)
print! a.lerp b, 1.5
check! num2(15, 70)
```

## make_num2

```kototype
|Number| -> Num2
```

```kototype
|Number, Number| -> Num2
```

```kototype
|Num2| -> Num2
```

```kototype
|Iterable| -> Num2
```

Makes a Num2 from the provided values.

### Example

```koto
print! make_num2 1
check! num2(1, 1)

print! make_num2 3, 4
check! num2(3, 4)

print! make_num2 [11, 12]
check! num2(11, 12)
```

## max

```kototype
|Num2| -> Float
```

Returns the value of the largest element in the Num2.

### Example

```koto
x = make_num2(10, 20)
print! x.max()
check! 20.0
```

## min

```kototype
|Num2| -> Float
```

Returns the value of the smallest element in the Num2.

### Example

```koto
x = make_num2(10, 20)
print! x.min()
check! 10.0
```

## normalize

```kototype
|Num2| -> Num2
```

Returns a Num2 with the same direction as the input,
with its length normalized to 1.

### Example

```koto
x = make_num2(3, 4)
print! x.normalize()
check! num2(0.6, 0.8)
```

## product

```kototype
|Num2| -> Float
```

Returns the result of multiplying the Num2's elements together.

### Example

```koto
x = make_num2(10, 20)
print! x.product()
check! 200.0
```

## sum

```kototype
|Num2| -> Float
```

Returns the result of adding the Num2's elements together.

### Example

```koto
x = make_num2(10, 20)
print! x.sum()
check! 30.0
```

## with

```kototype
|Num2, index: Number, value: Number| -> Num2
```

Returns a Num2 with the element at `index` replaced with `value`.

### Example

```koto
x = make_num2(10, 20)
print! x.with 0, 99
check! num2(99, 20)
print! x.with 1, -1
check! num2(10, -1)
```

## x

```kototype
|Num2| -> Float
```

Returns the first element of the Num2.

### Example

```koto
n = make_num2 10, 20
print! n.x()
check! 10.0
```

## y

```kototype
|Num2| -> Float
```

Returns the second element of the Num2.

### Example

```koto
n = make_num2 10, 20
print! n.y()
check! 20.0
```
