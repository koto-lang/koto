# number

## abs

```kototype
|Number| -> Number
```

Returns the absolute value of the number.

### Example

```koto
print! -1.abs()
check! 1

print! 1.abs()
check! 1
```

## acos

```kototype
|Number| -> Number
```

Returns the arc cosine of the number. `acos` is the inverse function of `cos`.

### Example

```koto
from number import pi

assert_near 0.acos(), pi / 2
assert_eq 1.acos(), 0
```

## acosh

```kototype
|Number| -> Number
```

Returns the inverse hyperbolic cosine of the number.

### Example

```koto
assert 0.acosh().is_nan()
assert_eq 1.acosh(), 0
assert_near 2.acosh(), 1.3169578969248166
```

## and

```kototype
|Number, Number| -> Number
```

Returns the bitwise combination of the binary representations of two numbers, 
where a `1` in both of the inputs produces a `1` in the corresponding output 
position.

### Note

If either input is a float then its integer part will be used.


### Example

```koto
print! 0b1010.and 0b1100
# 0b1000
check! 8
```

## asin

```kototype
|Number| -> Number
```

Returns the arc sine of the number. `asin` is the inverse function of `sin`.

### Example

```koto
from number import pi

assert_eq 0.asin(), 0
assert_near 1.asin(), pi / 2
```

## asinh

```kototype
|Number| -> Number
```

Returns the inverse hyperbolic sine of the number.

### Example

```koto
assert_eq 0.asinh(), 0
assert_near 1.asinh(), 0.8813735870195429
```

## atan

```kototype
|Number| -> Number
```

Returns the arc tangent of the number. `atan` is the inverse function of `tan`.

### Example

```koto
from number import pi

assert_eq 0.atan(), 0
assert_near 1.atan(), pi / 4
```

## atanh

```kototype
|Number| -> Number
```

Returns the inverse hyperbolic tangent of the number.

### Example

```koto
print! -1.atanh()
check! -inf

print! 0.atanh()
check! 0.0

print! 1.atanh()
check! inf
```

## atan2

```kototype
|Number, Number| -> Number
```

Returns the arc tangent of `y` and `x` in radians, using the signs of `y` and
`x` to determine the correct quadrant.

### Example

```koto
from number import pi

x, y = 1, 1

assert_near y.atan2(x), pi / 4
assert_near y.atan2(-x), pi - pi / 4
```

## ceil

```kototype
|Number| -> Number
```

Returns the integer that's greater than or equal to the input.

### Example

```koto
print! 0.5.ceil()
check! 1

print! 2.ceil()
check! 2

print! -0.5.ceil()
check! 0
```

### See Also

- [`number.floor`](#floor)
- [`number.round`](#round)
- [`number.to_int`](#to-int)

## clamp

```kototype
|input: Number, min: Number, max: Number| -> Number
```

Returns the `input` number restricted to the range defined by `min` and `max`.

### Example

```koto
print! 0.clamp 1, 2
check! 1

print! 1.5.clamp 1, 2
check! 1.5

print! 3.0.clamp 1, 2
check! 2
```

## cos

```kototype
|Number| -> Number
```

Returns the cosine of the number.

### Example

```koto
print! 0.cos()
check! 1.0

print! number.pi.cos()
check! -1.0
```

## cosh

```kototype
|Number| -> Number
```

Returns the hyperbolic cosine of the number.

### Example

```koto
assert_eq 0.cosh(), 1
assert_near 1.cosh(), 1.5430806348152437
```

## degrees

```kototype
|Number| -> Number
```

Converts radians into degrees.

### Example

```koto
from number import pi, tau

print! pi.degrees()
check! 180.0

print! tau.degrees()
check! 360.0
```

## e

```kototype
Number
```

Provides the `e` constant.

## exp

```kototype
|Number| -> Number
```

Returns the result of applying the exponential function,
equivalent to calling `e.pow x`.

### Example

```koto
assert_eq 0.exp(), 1
assert_eq 1.exp(), number.e
```

## exp2

```kototype
|Number| -> Number
```

Returns the result of applying the base-2 exponential function,
equivalent to calling `2.pow x`.

### Example

```koto
print! 1.exp2()
check! 2.0

print! 3.exp2()
check! 8.0
```

## flip_bits

```kototype
|Number| -> Number
```

Returns the input with its bits 'flipped', i.e. `1` => `0`, and `0` => `1`.

### Example

```koto
print! 1.flip_bits()
check! -2
```

## floor

```kototype
|Number| -> Number
```

Returns the integer that's less than or equal to the input.

### Example

```koto
print! 0.5.floor()
check! 0

print! 2.floor()
check! 2

print! -0.5.floor()
check! -1
```

### See Also

- [`number.ceil`](#ceil)
- [`number.round`](#round)
- [`number.to_int`](#to-int)

## infinity

```kototype
Number
```

Provides the `∞` constant.

## is_nan

```kototype
|Number| -> Bool
```

Returns true if the number is `NaN`.

### Example

```koto
print! 1.is_nan()
check! false

print! (0 / 0).is_nan()
check! true
```

## lerp

```kototype
|a: Number, b: Number, t: Number| -> Number
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
a, b = 1, 2

print! a.lerp b, 0
check! 1
print! a.lerp b, 0.5
check! 1.5
print! a.lerp b, 1
check! 2

print! a.lerp b, -0.5
check! 0.5
print! a.lerp b, 1.5
check! 2.5
```

## ln

```kototype
|Number| -> Number
```

Returns the natural logarithm of the number.

### Example

```koto
print! 1.ln()
check! 0.0

print! number.e.ln()
check! 1.0
```

## log2

```kototype
|Number| -> Number
```

Returns the base-2 logarithm of the number.

### Example

```koto
print! 2.log2()
check! 1.0

print! 4.log2()
check! 2.0
```

## log10

```kototype
|Number| -> Number
```

Returns the base-10 logarithm of the number.

### Example

```koto
print! 10.log10()
check! 1.0

print! 100.log10()
check! 2.0
```

## max

```kototype
|Number, Number| -> Number
```

Returns the larger of the two numbers.

### Example

```koto
print! 1.max 2
check! 2

print! 4.5.max 3
check! 4.5
```

## min

```kototype
|Number, Number| -> Number
```

Returns the smaller of the two numbers.

### Example

```koto
print! 1.min 2
check! 1

print! 4.5.min 3
check! 3
```

## nan

```kototype
Number
```

Provides the `NaN` (Not a Number) constant.

## negative_infinity

```kototype
Number
```

Provides the `-∞` constant.

## or

```kototype
|Number, Number| -> Number
```

Returns the bitwise combination of the binary representations of two numbers, 
where a `1` in either of the inputs produces a `1` in the corresponding output 
position.

### Note

If either input is a float then its integer part will be used.

### Example

```koto
print! 0b1010.or 0b1100
# 0b1110
check! 14
```

## pi

```kototype
Number
```

Provides the `π` constant.

## pi_2

```kototype
Number
```

Provides the `π` constant divided by `2`.

## pi_4

```kototype
Number
```

Provides the `π` constant divided by `4`.

## pow

```kototype
|Number, Number| -> Number
```

Returns the result of raising the first number to the power of the second.

### Example

```koto
print! 2.pow 3
check! 8
```

## radians

```kototype
|Number| -> Number
```

Converts degrees into radians.

### Example

```koto
from number import pi

assert_near 90.radians(), pi / 2
assert_near 360.radians(), pi * 2
```

## recip

```kototype
|Number| -> Number
```

Returns the reciprocal of the number, i.e. `1 / x`.

### Example

```koto
print! 2.recip()
check! 0.5
```

## round

```kototype
|Number| -> Number
```

Returns the nearest integer to the input number.
Half-way values round away from zero.

### Example

```koto
print! 0.5.round()
check! 1

print! 2.round()
check! 2

print! -0.5.round()
check! -1
```

### See Also

- [`number.ceil`](#ceil)
- [`number.floor`](#floor)
- [`number.to_int`](#to_int)

## shift_left

```kototype
|Number, shift_amount: Number| -> Number
```

Returns the result of shifting the bits of the first number to the left by the
amount specified by the second number.

### Note

If either input is a float then its integer part will be used.

### Note

The shift amount must be greater than or equal to `0`.

### Example

```koto
print! 0b1010.shift_left 2
# 0b101000
check! 40
```

## shift_right

```kototype
|Number, shift_amount: Number| -> Number
```

Returns the result of shifting the bits of the first number to the right by the
amount specified by the second number.

### Note

If either input is a float then its integer part will be used.

### Note

The shift amount must be greater than or equal to `0`.

### Example

```koto
print! 0b1010.shift_right 2
# 0b0010
check! 2
```

## sin

```kototype
|Number| -> Number
```

Returns the sine of the number.

### Example

```koto
from number import pi

print! (pi * 0.5).sin()
check! 1.0

print! (pi * 1.5).sin()
check! -1.0
```

## sinh

```kototype
|Number| -> Number
```

Returns the hyperbolic sine of the number.

### Example

```koto
assert_eq 0.sinh(), 0
assert_near 1.sinh(), 1.1752011936438014
```

## sqrt

```kototype
|Number| -> Number
```

Returns the square root of the number.

### Example

```koto
print! 64.sqrt()
check! 8.0
```

## tan

```kototype
|Number| -> Number
```

Returns the tangent of the number.

### Example

```koto
assert_eq 0.tan(), 0
assert_near 1.tan(), 1.557407724654902
```

## tanh

```kototype
|Number| -> Number
```

Returns the hyperbolic tangent of the number.

### Example

```koto
assert_near 1.tanh(), 1.sinh() / 1.cosh()
```

## tau

```kototype
Number
```

Provides the `τ` constant, equivalent to `2π`.

## to_int

```kototype
|Number| -> Number
```

Returns the integer part of the input number.

This is often called `trunc` in other languages.

### Example

```koto
print! 2.9.to_int()
check! 2

print! 1.5.to_int()
check! 1

print! -0.5.to_int()
check! 0

print! -1.9.to_int()
check! -1
```

### See Also

- [`number.ceil`](#ceil)
- [`number.floor`](#floor)
- [`number.round`](#round)

## xor

```kototype
|Number, Number| -> Number
```

Returns the bitwise combination of the binary representations of two numbers,
where a `1` in one (and only one) of the input positions
produces a `1` in the corresponding output position.

### Note

If either input is a float then its integer part will be used.

### Example

```koto
print! 0b1010.xor 0b1100
# 0b0110
check! 6
```
