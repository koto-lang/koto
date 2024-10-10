# color

Utilities for working with color in Koto.

At the core of the library is the [`Color`](#color-1) type, with various initializers
available. 

For convenience, the color module itself is callable:

```kototype
|name: String| -> Color
```

Equivalent to calling [`color.named`](#named).

```kototype
|r: Number, g: Number, b: Number| -> Color
```

Equivalent to calling [`color.rgb`](#rgb).

```kototype
|r: Number, g: Number, b: Number, a: Number| -> Color
```

Equivalent to calling [`color.rgba`](#rgba).

**Example:**
```koto
print! color 'red'
check! Color {r: 1, g: 0, b: 0, a: 1}
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
check! Color {r: 0, g: 0.5, b: 0.5, a: 1}
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
check! Color {r: 0.75, g: 1, b: 0.5, a: 1}
```

## named

```kototype
|name: String| -> Color
```

Returns a color corresponding to one of the named colors listed in the
[SVG color keywords][svg-colors] specification.

### Example

```koto
print! color.named 'yellow'
check! Color {r: 1, g: 1, b: 0, a: 1}
```

## okhsl

```kototype
|h: Number, s: Number, l: Number| -> Color
```
```kototype
|h: Number, s: Number, l: Number, a: Number| -> Color
```

Returns a color produced from hue, saturation, lightness,
and optional alpha components, using the [Oklab][oklab] color space.

The hue component is specified in degrees. 

The saturation, lightness, and alpha components are specified as numbers between `0` and `1`.

### Example

```koto
print! color.okhsl 180, 1, 0.25
check! Color {r: 0, g: 0.5, b: 0.5, a: 1}
```

## okhsv

```kototype
|h: Number, s: Number, v: Number| -> Color
```
```kototype
|h: Number, s: Number, v: Number, a: Number| -> Color
```

Returns a color produced from hue, saturation, value,
and optional alpha components, using the [Oklab][oklab] color space.

The hue component is specified in degrees. 

The saturation, value, and alpha components are specified as numbers between `0` and `1`.

### Example

```koto
print! color.okhsv 90, 0.5, 1
check! Color {r: 0.75, g: 1, b: 0.5, a: 1}
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
print! color.oklch 0.5, 0.1, 180
check! Color {r: 0.75, g: 1, b: 0.5, a: 1}
```

## rgb

```kototype
|r: Number, g: Number, b: Number| -> Color
```

Returns a color produced from [red, green, blue][rgb], 
and optional alpha components.

The color components are specified as numbers between `0` and `1`.

### Example

```koto
print! color.rgb 0.5, 0.1, 0.9
check! Color {r: 0.5, g: 0.1, b: 0.9, a: 1}

print! color.rgb 0.2, 0.4, 0.3, 0.5
check! Color {r: 0.2, g: 0.4, b: 0.3, a: 0.5}
```

## Color

The `color` module's core color type, represented by linear RGBA components.

All arithmetic operations are implemented, accepting colors or numbers as
input.

The color's RGBA components are iterable so can be unpacked directly.

### Example

```koto
print! color('red') + color('lime')
check! Color {r: 1, g: 1, b: 0, a: 1}

r, g, b = color('yellow')
print! r, g, b
check! (1.0, 1.0, 0.0)
```

## Color `.r` | `.red`

```kototype
|Color| -> Number
```

Returns the color's red component.

### Example

```koto
print! color('black').r()
check! 0.0

print! color('yellow').red()
check! 1.0
```

## Color .g | .green

```kototype
|Color| -> Number
```

Returns the color's green component.

### Example

```koto
print! color('black').g()
check! 0.0

print! color('yellow').green()
check! 1.0
```

## Color `.b` | `.blue`

```kototype
|Color| -> Number
```

Returns the color's blue component.

### Example

```koto
print! color('black').b()
check! 0.0

print! color('cyan').blue()
check! 1.0
```

## Color `.b` | `.blue`

```kototype
|Color| -> Number
```

Returns the color's blue component.

### Example

```koto
print! color('black').b()
check! 0.0

print! color('cyan').blue()
check! 1.0
```

## Color `.a` | `.alpha`

```kototype
|Color| -> Number
```

Returns the color's alpha component.

### Example

```koto
print! color('black').a()
check! 1.0

print! color(1, 1, 1, 0.5).alpha()
check! 0.5
```

## Color `.set_r` | `.set_red`


```kototype
|Color, r: Number| -> Color
```

Sets the color's red component, and returns the color.


### Example

```koto
print! color('black').set_r(1.0)
check! Color {r: 1, g: 0, b: 0, a: 1}

print! color('red').set_red(0.0)
check! Color {r: 0, g: 0, b: 0, a: 1}
```

## Color `.set_g` | `.set_green`


```kototype
|Color, g: Number| -> Color
```

Sets the color's green component, and returns the color.


### Example

```koto
print! color('black').set_g(1.0)
check! Color {r: 0, g: 1, b: 0, a: 1}

print! color('red').set_green(1.0)
check! Color {r: 1, g: 1, b: 0, a: 1}
```

## Color `.set_b` | `.set_blue`


```kototype
|Color, b: Number| -> Color
```

Sets the color's blue component, and returns the color.


### Example

```koto
print! color('black').set_b(1.0)
check! Color {r: 0, g: 0, b: 1, a: 1}

print! color('red').set_blue(1.0)
check! Color {r: 1, g: 0, b: 1, a: 1}
```

## Color `.set_a` | `.set_alpha`


```kototype
|Color, Number| -> Color
```

Sets the color's blue component, and returns the color.


### Example

```koto
print! color('black').set_a(0.5)
check! Color {r: 0, g: 0, b: 0, a: 0.5}

print! color('red').set_alpha(0.2)
check! Color {r: 1, g: 0, b: 0, a: 0.2}
```

## Color `.mix`

```kototype
|
a: Color, b: Color| -> Color
```

Returns a new color representing an even mix of the two input colo r.

```kototype
|a: Color, b: Color, weight: Number| -> Color
```

Returns a new color representing a weighted mix of the two input colors. 

The `weight` argument is a number between `0` and `1`, with values closer to
`0` producing results closer to the first color, and values closer to `1`
producing results closer to the second color.


### Example

```koto
a, b = color('red'), color('blue')
print! a.mix b
check! Color {r: 0.5, g: 0, b: 0.5, a: 1}

print! a.mix b, 0.25
check! Color {r: 0.75, g: 0, b: 0.25, a: 1}
```



[hsl-hsv]: https://en.wikipedia.org/wiki/HSL_and_HSV
[oklab]: https://en.wikipedia.org/wiki/Oklab_color_space
[rgb]: https://en.wikipedia.org/wiki/RGB_color_model
[rgba]: https://en.wikipedia.org/wiki/RGBA_color_model
[svg-colors]: https://www.w3.org/TR/SVG11/types.html#ColorKeywords
