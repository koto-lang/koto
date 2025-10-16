# color

Utilities for working with color in Koto.

At the core of the library is the [`Color`](#color-1) type, with various initializers
available.

For convenience, the color module itself is callable as a shorthand for some standard initializers:

```kototype
|String| -> Color
```

Equivalent to calling [`color.named`](#named), or [`color.hex`](#hex) if no matching name is found.

```kototype
|Number| -> Color
```

Equivalent to calling [`color.hex`](#hex) with a number.

```kototype
|r: Number, g: Number, b: Number| -> Color
```

```kototype
|r: Number, g: Number, b: Number, a: Number| -> Color
```

Equivalent to calling [`color.rgb`](#rgb).

**Example:**

```koto
print! color 'red'
check! Color(RGB, r: 1, g: 0, b: 0, a: 1)

print! color '#00ffff'
check! Color(RGB, r: 0, g: 1, b: 1, a: 1)

print! color 0xff00ff
check! Color(RGB, r: 1, g: 0, b: 1, a: 1)

print! color 0, 0.5, 1, 0.5
check! Color(RGB, r: 0, g: 0.5, b: 1, a: 0.5)
```

## named

```kototype
|name: String| -> Color?
```

Returns a color in the [sRGB][srgb] color space corresponding to one of the named colors
listed in the [SVG color keywords][svg-colors] specification.

If no name is found then `null` will be returned.

### Example

```koto
print! color.named 'yellow'
check! Color(RGB, r: 1, g: 1, b: 0, a: 1)
```

## hex

```kototype
|String| -> Color
```

Creates a color in the [sRGB][srgb] color space from the given [hex triplet][hex-triplet] string, e.g. `'#7fee80'`.

The `#` prefix is optional, and the 3 digit shorthand version (e.g. `'#7e8'`) can also be used.

If the string can't be parsed as a hex triplet then `null` will be returned.

```kototype
|String| -> Color
```

### Example

```koto
print! color.hex '#ff00ff'
check! Color(RGB, r: 1, g: 0, b: 1, a: 1)

print! color.hex 'f0f'
check! Color(RGB, r: 1, g: 0, b: 1, a: 1)

print! color.hex 0x00ff00
check! Color(RGB, r: 0, g: 1, b: 0, a: 1)
```

## hsl

```kototype
|h: Number, s: Number, l: Number| -> Color
```

```kototype
|h: Number, s: Number, l: Number, a: Number| -> Color
```

Returns a color produced from [hue, saturation, lightness][hsl-hsv],
and optional alpha components.

The hue component is specified in degrees.

The saturation, lightness, and alpha components are specified as numbers between `0`
and `1`.

### Example

```koto
print! color.hsl 180, 1, 0.25
check! Color(HSL, h: 180, s: 1, l: 0.25, a: 1)
```

## hsv

```kototype
|h: Number, s: Number, v: Number| -> Color
```
```kototype
|h: Number, s: Number, v: Number, a: Number| -> Color
```

Returns a color produced from [hue, saturation, value][hsl-hsv],
and optional alpha components.

The hue component is specified in degrees.

The saturation, value, and alpha components are specified as numbers between `0` and `1`.

### Example

```koto
print! color.hsv 90, 0.5, 1
check! Color(HSV, h: 90, s: 0.5, v: 1, a: 1)
```

## oklab

```kototype
|l: Number, a: Number, b: Number| -> Color
```
```kototype
|l: Number, a: Number, b: Number, alpha: Number| -> Color
```

Returns a color produced from lightness, `a`, `b`,
and optional alpha components, using the [Oklab][oklab] color space.

The lightness and alpha components are specified as numbers between `0` and `1`.

The `a` (green/red) and `b` (blue/yellow) components are numbers with values typically between `-0.4` and `0.4`.

### Example

```koto
print! color.oklab 0.5, 0.1, -0.2
check! Color(Oklab, l: 0.5, a: 0.1, b: -0.2, alpha: 1)
```

## oklch

```kototype
|l: Number, c: Number, h: Number| -> Color
```
```kototype
|l: Number, c: Number, h: Number, a: Number| -> Color
```

Returns a color produced from lightness, chroma, hue,
and optional alpha components, using the [Oklab][oklab] color space.

The lightness and alpha components are specified as numbers between `0` and `1`.

The hue component is specified in degrees.

The chroma component is a number between `0` and a maximum that depends on the
hue and lightness components.

### Example

```koto
print! color.oklch 0.6, 0.1, 180
check! Color(Oklch, l: 0.6, c: 0.1, h: 180, a: 1)
```

## rgb

```kototype
|r: Number, g: Number, b: Number| -> Color
```

Returns a color produced from [red, green, blue][rgb],
and optional alpha components, using the [sRGB][srgb] color space.

The color components are specified as numbers between `0` and `1`.

### Example

```koto
print! color.rgb 0.5, 0.1, 0.9
check! Color(RGB, r: 0.5, g: 0.1, b: 0.9, a: 1)

print! color.rgb 0.2, 0.4, 0.3, 0.5
check! Color(RGB, r: 0.2, g: 0.4, b: 0.3, a: 0.5)
```

## Color

The `color` module's core color type.

The color may belong to various different color spaces,
with the space's components available via iteration or indexing.

The color's `alpha` value is always present as the color's fourth component.

The color space's components can be modified via index, and the `alpha`
component can also be modified via [`.set_alpha`](#color-set-alpha).


### Example

```koto
r, g, b = color 'yellow'
print! r, g, b
check! (1.0, 1.0, 0.0)

h, s, v, a = color.hsv 90, 0.5, 0.25
print! h, s, v, a
check! (90.0, 0.5, 0.25, 1.0)

red = color 'red'
print! red.r, red.g, red.b
check! (1.0, 0.0, 0.0)

print! c = color.oklch 0.5, 0.1, 180
check! Color(Oklch, l: 0.5, c: 0.1, h: 180, a: 1)
c[0] = 0.25 # Set the lightness component to 0.25
c[1] = 0.1 # Set the chroma component to 0.1
print c
check! Color(Oklch, l: 0.25, c: 0.1, h: 180, a: 1)
```

## Color.red | Color.r

```kototype
Number
```

The color's `red` component.

An error is thrown if the color space doesn't have a `red` component.

### Example

```koto
print! c = color 'red'
check! Color(RGB, r: 1, g: 0, b: 0, a: 1)

print! c.red
check! 1.0

c.red = 0.5
print! c.r
check! 0.5
```

## Color.green | Color.g

```kototype
Number
```

The color's `green` component.

An error is thrown if the color space doesn't have a `green` component.

### Example

```koto
print! c = color 'lime'
check! Color(RGB, r: 0, g: 1, b: 0, a: 1)

print! c.green
check! 1.0

c.green = 0.5
print! c.g
check! 0.5
```

## Color.blue | Color.b

```kototype
Number
```

The color's `blue` component.

An error is thrown if the color space doesn't have a `blue` component.

### Example

```koto
print! c = color 'blue'
check! Color(RGB, r: 0, g: 0, b: 1, a: 1)

print! c.blue
check! 1.0

c.blue = 0.5
print! c.b
check! 0.5
```

## Color.hue | Color.h

```kototype
Number
```

The color's `hue` component.

An error is thrown if the color space doesn't have a `hue` component.

### Example

```koto
print! c = color.hsv 90, 0.5, 1.0
check! Color(HSV, h: 90, s: 0.5, v: 1, a: 1)

print! c.hue
check! 90.0

c.hue = 45.0
print! c.h
check! 45.0
```

## Color.saturation | Color.s

```kototype
Number
```

The color's `saturation` component.

An error is thrown if the color space doesn't have a `saturation` component.

### Example

```koto
print! c = color.hsv 90, 0.5, 1.0
check! Color(HSV, h: 90, s: 0.5, v: 1, a: 1)

print! c.saturation
check! 0.5

c.saturation = 0.25
print! c.s
check! 0.25
```

## Color.value | Color.v

```kototype
Number
```

The color's `value` component.

An error is thrown if the color space doesn't have a `value` component.

### Example

```koto
print! c = color.hsv 90, 0.5, 1.0
check! Color(HSV, h: 90, s: 0.5, v: 1, a: 1)

print! c.value
check! 1.0

c.value = 0.5
print! c.v
check! 0.5
```

## Color.lightness | Color.l

```kototype
Number
```

The color's `lightness` component.

An error is thrown if the color space doesn't have a `lightness` component.

### Example

```koto
print! c = color.oklab 0.5, 0.2, -0.1
check! Color(Oklab, l: 0.5, a: 0.2, b: -0.1, alpha: 1)

print! c.lightness
check! 0.5

c.lightness = 0.25
print! c.l
check! 0.25
```

## Color.a

```kototype
Number
```

The color's `a` component.

An error is thrown if the color space doesn't have an `a` component.

### Example

```koto
print! c = color.oklab 0.5, 0.25, -0.1
check! Color(Oklab, l: 0.5, a: 0.25, b: -0.1, alpha: 1)

print! c.a
check! 0.25

c.a = 0.5
print! c.a
check! 0.5
```

## Color.b

```kototype
Number
```

The color's `b` component.

An error is thrown if the color space doesn't have a `b` component.

### Example

```koto
print! c = color.oklab 0.5, 0.25, -0.25
check! Color(Oklab, l: 0.5, a: 0.25, b: -0.25, alpha: 1)

print! c.b
check! -0.25

c.b = 0.25
print! c.b
check! 0.25
```


## Color.chroma | Color.c

```kototype
Number
```

The color's `chroma` component.

An error is thrown if the color space doesn't have a `chroma` component.

### Example

```koto
print! c = color.oklch 0.6, 0.25, 180
check! Color(Oklch, l: 0.6, c: 0.25, h: 180, a: 1)

print! c.chroma
check! 0.25

c.chroma = 0.5
print! c.c
check! 0.5
```

## Color.alpha

```kototype
Number
```

The color's alpha component.

### Example

```koto
c = color 'red'

print! c.alpha
check! 1.0

c.alpha = 0.25
print! c.alpha
check! 0.25
```

## Color.mix

```kototype
|a: Color, b: Color| -> Color
```

Returns a new color representing an even mix of the two input colors.

An error is thrown if the colors do not belong to the same color space.

```kototype
|a: Color, b: Color, weight: Number| -> Color
```

Returns a new color representing a weighted mix of the two input colors.

The `weight` argument is a number between `0` and `1`, with values closer to
`0` producing results closer to the first color, and values closer to `1`
producing results closer to the second color.

An error is thrown if the colors do not belong to the same color space.

### Example

```koto
a, b = color('red'), color('blue')
print! a.mix b
check! Color(RGB, r: 0.5, g: 0, b: 0.5, a: 1)

print! a.mix b, 0.25
check! Color(RGB, r: 0.75, g: 0, b: 0.25, a: 1)
```

## Color.to_hsl

```kototype
|Color| -> Color
```

Returns a new color with the input converted into the [HSL][hsl-hsv] color space.

### Example

```koto
print! color('blue').to_hsl()
check! Color(HSL, h: 240, s: 1, l: 0.5, a: 1)
```

## Color.to_hsv

```kototype
|Color| -> Color
```

Returns a new color with the input converted into the [HSV][hsl-hsv] color space.

### Example

```koto
print! color('blue').to_hsv()
check! Color(HSV, h: 240, s: 1, v: 1, a: 1)
```

## Color.to_oklab

```kototype
|Color| -> Color
```

Returns a new color with the input converted into the [Oklab][oklab] color space.

### Example

```koto
l, a, b = color('blue').to_oklab()
allowed_error = 1e-3
assert_near l, 0.452, allowed_error
assert_near a, -0.033, allowed_error
assert_near b, -0.312, allowed_error
```

## Color.to_oklch

```kototype
|Color| -> Color
```

Returns a new color with the input converted into the [Oklch][oklab] color space.

### Example

```koto
l, c, h = color('blue').to_oklch()
allowed_error = 1e-3
assert_near l, 0.452, allowed_error
assert_near c, 0.313, allowed_error
assert_near h, 264.052, allowed_error
```

## Color.to_rgb

```kototype
|Color| -> Color
```

Returns a new color with the input converted into the [sRGB][oklab] color space.

### Example

```koto
l, c, h = color('blue').to_oklch()
allowed_error = 1e-3
assert_near l, 0.452, allowed_error
assert_near c, 0.313, allowed_error
assert_near h, 264.052, allowed_error
```


[hex-triplet]: https://en.wikipedia.org/wiki/Web_colors#Hex_triplet
[hsl-hsv]: https://en.wikipedia.org/wiki/HSL_and_HSV
[oklab]: https://en.wikipedia.org/wiki/Oklab_color_space
[rgb]: https://en.wikipedia.org/wiki/RGB_color_model
[rgba]: https://en.wikipedia.org/wiki/RGBA_color_model
[srgb]: https://en.wikipedia.org/wiki/SRGB
[svg-colors]: https://www.w3.org/TR/SVG11/types.html#ColorKeywords
