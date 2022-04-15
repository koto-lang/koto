# number

Numbers in Koto are represented internally as either a signed 64 bit integer or
float, switching between the two representations automatically depending on
usage.

### Example

```koto
x = 1        # x is assigned as an integer
y = x + 0.5  # y is assigned as a float
x += 0.99    # x is now a float
```

# Reference

## abs

`|Number| -> Number`

Returns the absolute value of the number.

### Example

```koto
-1.abs()
# 1

1.abs()
# 1
```

## acos

`|Number| -> Float`

Returns the arc cosine of the number. `acos` is the inverse function of `cos`.

### Example

```koto
0.acos()
# π / 2

1.acos()
# 1
```

## acosh

`|Number| -> Float`

Returns the inverse hyperbolic cosine of the number.

### Example

```koto
0.acosh()
# NaN

1.acosh()
# 0

2.acosh()
# 1.3169578969248166
```

## and

`|Integer, Integer| -> Integer`

Returns the bitwise combination of two integers, where a `1` in both input
positions produces a `1` in corresponding output positions.

### Example

```koto
0b1010.and 0b1100
# 0b1000
```

## asin

`|Number| -> Float`

Returns the arc sine of the number. `asin` is the inverse function of `sin`.

### Example

```koto
0.asin()
# 0

1.asin()
# π / 2
```

## asinh

`|Number| -> Float`

Returns the inverse hyperbolic sine of the number.

### Example

```koto
0.asinh()
# 0

1.asinh()
# 0.8813735870195429
```

## atan

`|Number| -> Float`

Returns the arc tangent of the number. `atan` is the inverse function of `tan`.

### Example

```koto
0.atan()
# 0

1.atan()
# π / 4
```

## atanh

`|Number| -> Float`

Returns the inverse hyperbolic tangent of the number.

### Example

```koto
-1.atanh()
# -inf

0.atanh()
# 0

1.atanh()
# inf
```

## atan2

`|Number, Number| -> Float`

Returns the arc tangent of `y` and `x` in radians, using the signs of `y` and
`x` to determine the correct quadrant.

### Note

`y.atan2 x` is equivalent to `make_num2(x, y).angle()`.

### Example

```koto
x, y = 1, 1

y.atan2 x
# π/4

y.atan2 -x
# π - π/4

-y.atan2 x
# -π/4
```

## ceil

`|Number| -> Integer`

Returns the smallest integer that's greater than or equal to the input.

### Example

```koto
0.5.ceil()
# 1

2.ceil()
# 2

-0.5.ceil()
# 0
```

### See Also

- [`number.floor`](#floor)
- [`number.round`](#round)
- [`number.to_int`](#to-int)

## clamp

`|Number, Number, Number| -> Number`

Returns the first number restricted to the range defined by the second and third
numbers.

### Example

```koto
0.clamp 1, 2
# 1

1.5.clamp 1, 2
# 1.5

3.0.clamp 1, 2
# 2
```

## cos

`|Number| -> Float`

Returns the cosine of the number.

### Example

```koto
0.cos()
# 1.0

import number.pi

pi.cos()
# -1.0
```

## cosh

`|Number| -> Float`

Returns the hyperbolic cosine of the number.

### Example

```koto
3.cosh()
# (e.pow(3) + e.pow(-3)) / 2
```

## degrees

`|Number| -> Float`

Converts radians into degrees.

### Example

```koto
from number import pi, tau

pi.degrees()
# 180.0

tau.degrees()
# 360.0
```

## e

`Float`

Provides the `e` constant.

## exp

`|Number| -> Float`

Returns the result of applying the exponential function,
equivalent to calling `e.pow x`.

### Example

```koto
0.exp()
# 1.0

1.exp()
# number.e
```

## exp2

`|Number| -> Float`

Returns the result of applying the base-2 exponential function,
equivalent to calling `2.pow x`.

### Example

```koto
1.exp2()
# 2.0

3.exp2()
# 8.0
```

## flip_bits

`|Integer| -> Integer`

Returns the input with its bits 'flipped', i.e. `1` => `0`, and `0` => `1`.

### Example

```koto
1.flip_bits()
# -2
```

## floor

`|Number| -> Integer`

Returns the smallest integer that's less than or equal to the input.

### Example

```koto
0.5.floor()
# 0

2.floor()
# 2

-0.5.floor()
# -1
```

### See Also

- [`number.ceil`](#ceil)
- [`number.round`](#round)
- [`number.to_int`](#to-int)

## infinity

`Float`

Provides the `∞` constant.

## is_nan

`|Number| -> Bool`

Returns true if the number is `NaN`.

### Example

```koto
1.is_nan()
# false

(0 / 0).is_nan()
# true
```

## lerp

`|a: Number, b: Number, t: Number| -> Float`

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
a, b = 1, 2

a.lerp b, 0
# 1
a.lerp b, 0.5
# 1.5
a.lerp b, 1
# 2

a.lerp b, -0.5
# 0.5
a.lerp b, 1.5
# 2.5
```

## ln

`|Number| -> Float`

Returns the natural logarithm of the number.

### Example

```koto
1.ln()
# 0.0

from number import e

e.ln()
# 1.0
```

## log2

`|Number| -> Float`

Returns the base-2 logarithm of the number.

### Example

```koto
2.log2()
# 1.0

4.log2()
# 2.0
```

## log10

`|Number| -> Float`

Returns the base-10 logarithm of the number.

### Example

```koto
10.log10()
# 1.0

100.log10()
# 2.0
```

## max

`|Number, Number| -> Number`

Returns the larger of the two numbers.

### Example

```koto
1.max 2
# 2

4.5.max 3
# 4.5
```

## min

`|Number, Number| -> Number`

Returns the smaller of the two numbers.

### Example

```koto
1.min 2
# 1

4.5.min 3
# 3
```

## nan

`Float`

Provides the `NaN` (Not a Number) constant.

## negative_infinity

`Float`

Provides the `-∞` constant.

## or

`|Integer, Integer| -> Integer`

Returns the bitwise combination of two integers, where a `1` in either input
positions produces a `1` in corresponding output positions.

### Example

```koto
0b1010.or 0b1100
# 0b1110
```

## pi

`Float`

Provides the `π` constant.

## pi_2

`Float`

Provides the `π` constant divided by `2`.

## pi_4

`Float`

Provides the `π` constant divided by `4`.

## pow

`|Number, Number| -> Number`

Returns the result of raising the first number to the power of the second.

### Example

```koto
2.pow 3
# 8
```

## radians

`|Number| -> Float`

Converts degrees into radians.

### Example

```koto
90.radians()
# π / 2

360.radians()
# π * 2
```

## recip

`|Number| -> Float`

Returns the reciprocal of the number, i.e. `1 / x`.

### Example

```koto
2.recip()
# 0.5
```

## round

`|Number| -> Integer`

Returns the nearest integer to the input number.
Half-way values round away from zero.

### Example

```koto
0.5.round()
# 1

2.round()
# 2

-0.5.round()
# -1
```

### See Also

- [`number.ceil`](#ceil)
- [`number.floor`](#floor)
- [`number.to_int`](#to-int)

## shift_left

`|Integer, Integer| -> Integer`

Returns the result of shifting the bits of the first number to the left by the
amount specified by the second number.

### Note

The shift amount must be greater than or equal to `0`.

### Example

```koto
0b1010.shift_left 2
# 0b101000
```

## shift_right

`|Integer, Integer| -> Integer`

Returns the result of shifting the bits of the first number to the right by the
amount specified by the second number.

### Note

The shift amount must be greater than or equal to `0`.

### Example

```koto
0b1010.shift_left 2
# 0b101000
```

## sin

`|Number| -> Float`

Returns the sine of the number.

### Example

```koto
import number.pi

(pi * 0.5).sin()
# 1.0

(pi * 1.5).sin()
# -1.0
```

## sinh

`|Number| -> Float`

Returns the hyperbolic sine of the number.

### Example

```koto
3.sinh()
# (e.pow(3) - e.pow(-3)) / 2
```

## sqrt

`|Number| -> Float`

Returns the square root of the number.

### Example

```koto
64.sqrt()
# 8.0
```

## tan

`|Number| -> Float`

Returns the tangent of the number.

### Example

```koto
1.tan()
# 1.sin() / 1.cos()
```

## tanh

`|Number| -> Float`

Returns the hyperbolic tangent of the number.

### Example

```koto
1.tanh()
# 1.sinh() / 1.cosh()
```

## tau

`Float`

Provides the `τ` constant, equivalent to `2π`.

## to_float

`|Number| -> Float`

Returns the number as a `Float`.

### Example

```koto
1.to_float()
# 1.0
```

## to_int

`|Number| -> Integer`

Returns the number as an `Integer`. This is equivalent to calling `ceil`.

### Example

```koto
1.5.to_int()
# 2

-0.5.to_int()
# 0
```

### See Also

- [`number.ceil`](#ceil)
- [`number.floor`](#floor)
- [`number.round`](#round)

## xor

`|Integer, Integer| -> Integer`

Returns the bitwise combination of two integers,
where a `1` in one (and only one) of the input positions
produces a `1` in corresponding output positions.

### Example

```koto
0b1010.xor 0b1100
# 0b0110
```
