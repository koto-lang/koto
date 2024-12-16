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

## hex

```kototype
|String| -> Color
```

Creates a color from the given [hex triplet][hex-triplet] string, e.g. `'#7fee80'`.

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

## named

```kototype
|name: String| -> Color or Null
```

Returns a color corresponding to one of the named colors listed in the
[SVG color keywords][svg-colors] specification. 

If no name is found then `null` will be returned.

### Example

```koto
print! color.named 'yellow'
check! Color(RGB, r: 1, g: 1, b: 0, a: 1)
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
check! Color(Oklab, l: 0.5, a: 0.1, b: -0.2, a: 1)
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

An `alpha` value is always present as the color's fourth component.

The color may belong to various different color spaces, 
with the space's components available via iteration or indexing.

Components can be modified via index.


### Example

```koto
r, g, b = color 'yellow'
print! r, g, b
check! (1.0, 1.0, 0.0)

h, s, v, a = color.hsv 90, 0.5, 0.25
print! h, s, v, a
check! (90.0, 0.5, 0.25, 1.0)

print! color('red')[0]
check! 1.0

print! c = color.oklch 0.5, 0.1, 180
check! Color(Oklch, l: 0.5, c: 0.1, h: 180, a: 1)
c[0] = 0.25 # Set the lightness component to 0.25
c[3] = 0.1 # Set the alpha component to 0.1
print c
check! Color(Oklch, l: 0.25, c: 0.1, h: 180, a: 0.1)
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
