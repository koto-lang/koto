# num4

## length

```kototype
|Num4| -> Float
```

Returns the length of the vector represented by the Num4's elements.

### Example

```koto
x = make_num4(2, -2, 2, -2)
print! x.length()
check! 4.0
```

## lerp

```kototype
|a: Num4, b: Num4, t: Number| -> Num4
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
a = make_num4 0, 10, -10, 0
b = make_num4 10, 50, 10, 0

print! a.lerp b, 0
check! num4(0, 10, -10, 0)
print! a.lerp b, 0.5
check! num4(5, 30, 0, 0)
print! a.lerp b, 1
check! num4(10, 50, 10, 0)

print! a.lerp b, -0.5
check! num4(-5, -10, -20, 0)
print! a.lerp b, 1.5
check! num4(15, 70, 20, 0)
```

## make_num4

```kototype
|Number| -> Num4
```
```kototype
|Number, Number| -> Num4
```
```kototype
|Number, Number, Number| -> Num4
```
```kototype
|Number, Number, Number, Number| -> Num4
```
```kototype
|Num2| -> Num4
```
```kototype
|Num4| -> Num4
```
```kototype
|Iterable| -> Num4
```

Makes a Num4 from the provided values.

### Example

```koto
print! make_num4 1
check! num4(1, 1, 1, 1)

print! make_num4 3, 4
check! num4(3, 4, 0, 0)

print! make_num4 5, 6, 7, 8
check! num4(5, 6, 7, 8)

print! make_num4 [11, 12, 13, 14]
check! num4(11, 12, 13, 14)
```

## max

```kototype
|Num4| -> Float
```

Returns the value of the largest element in the Num4.

### Example

```koto
x = make_num4(10, 20, -50, -10)
print! x.max()
check! 20.0
```

## min

```kototype
|Num4| -> Float
```

Returns the value of the smallest element in the Num4.

### Example

```koto
x = make_num4(10, 20, -50, -10)
print! x.min()
check! -50.0
```

## normalize

```kototype
|Num4| -> Num4
```

Returns a Num4 with the same direction as the input,
with its length normalized to 1.

### Example

```koto
x = make_num4(2, -2, 2, -2)
print! x.normalize()
check! num4(0.5, -0.5, 0.5, -0.5)
```

## product

```kototype
|Num4| -> Float
```

Returns the result of multiplying the Num4's elements together.

### Example

```koto
x = make_num4(10, 20, -50, -10)
print! x.product()
check! 100000.0
```

## sum

```kototype
|Num4| -> Float
```

Returns the result of adding the Num4's elements together.

### Example

```koto
x = make_num4(10, 20, 30, 40)
print! x.sum()
check! 100.0
```

## with

```kototype
|Num4, index: Number, value: Number| -> Num4
```

Returns a Num4 with the element at `index` replaced with `value`.

### Example

```koto
x = make_num4 10, 20, 30, 40
print! x.with 0, 99
check! num4(99, 20, 30, 40)
print! x.with 3, -1
check! num4(10, 20, 30, -1)
```

## r

```kototype
|Num4| -> Float
```

Returns the first element of the Num4.

This can be useful when using a Num4 as a colour value, and want to access its
'red' component.

### Example

```koto
n = make_num4 10, 20, 30, 40
print! n.r()
check! 10.0
```

## g

```kototype
|Num4| -> Float
```

Returns the second element of the Num4.

This can be useful when using a Num4 as a colour value, and want to access its
'green' component.

### Example

```koto
n = make_num4 10, 20, 30, 40
print! n.g()
check! 20.0
```

## b

```kototype
|Num4| -> Float
```

Returns the third element of the Num4.

This can be useful when using a Num4 as a colour value, and want to access its
'blue' component.

### Example

```koto
n = make_num4 10, 20, 30, 40
print! n.b()
check! 30.0
```

## a

```kototype
|Num4| -> Float
```

Returns the fourth element of the Num4.

This can be useful when using a Num4 as a colour value, and want to access its
'alpha' component.

### Example

```koto
n = make_num4 10, 20, 30, 40
print! n.w()
check! 40.0
```

## x

```kototype
|Num4| -> Float
```

Returns the first element of the Num4.

This can be useful when using a Num4 as a 3D or 4D vector, and want to access
its `x` component.

### Example

```koto
n = make_num4 10, 20, 30, 40
print! n.x()
check! 10.0
```

## y

```kototype
|Num4| -> Float
```

Returns the second element of the Num4.

### Example

```koto
n = make_num4 10, 20, 30, 40
print! n.y()
check! 20.0
```

This can be useful when using a Num4 as a 3D or 4D vector, and want to access
its `y` component.

## z

```kototype
|Num4| -> Float
```

Returns the third element of the Num4.

This can be useful when using a Num4 as a 3D or 4D vector, and want to access
its `z` component.

### Example

```koto
n = make_num4 10, 20, 30, 40
print! n.z()
check! 30.0
```

## w

```kototype
|Num4| -> Float
```

Returns the fourth element of the Num4.

This can be useful when using a Num4 as a 4D vector, and want to access its `w`
component.

### Example

```koto
n = make_num4 10, 20, 30, 40
print! n.w()
check! 40.0
```
