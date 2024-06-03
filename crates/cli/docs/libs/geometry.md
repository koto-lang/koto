# geometry

Utilities for working with geometry in Koto.

The module contains the [`Vec2`](#vec2-1), [`Vec3`](#vec3-1), and
[`Rect`](#rect-1) types.

## rect

```kototype
|| -> Rect
```

Initializes a default `Rect` with each component set to `0`.

```kototype
|x: Number, y: Number, width: Number, height: Number| -> Rect
|xy: Vec2, size: Vec2| -> Rect
```

Initializes a `Rect` with corresponding position and size.


### Example

```koto
from geometry import rect, vec2

print! rect()
check! Rect{x: 0, y: 0, width: 0, height: 0}

print! rect 10, 20, 30, 40
check! Rect{x: 10, y: 20, width: 30, height: 40}

print! rect (vec2 -1, 2), (vec2 99, 100)
check! Rect{x: -1, y: 2, width: 99, height: 100}
```

## vec2

```kototype
|| -> Vec2
```

Initializes a default `Vec2` with each component set to `0`.

```kototype
|x: Number| -> Vec2
```

Initializes a `Vec2` with `x` specified, and `y` set to `0`.

```kototype
|x: Number, y: Number| -> Vec2
|xy: Vec2| -> Vec2
```

Initializes a `Vec2` with corresponding `x` and `y` components.


### Example

```koto
from geometry import vec2

print! vec2()
check! Vec2{x: 0, y: 0}

print! vec2 99, 100
check! Vec2{x: 99, y: 100}
```

## vec3

```kototype
|| -> Vec3
```

Initializes a default `Vec3` with each component set to `0`.

```kototype
|x: Number| -> Vec3
```

Initializes a `Vec3` with `x` specified, and all other components set to `0`.

```kototype
|x: Number, y: Number| -> Vec3
|xy: Vec2| -> Vec3
```

Initializes a `Vec3` with `x` and `y` specified, and `z` set to `0`.

```kototype
|x: Number, y: Number, z: Number| -> Vec3
|xy: Vec2, z: Number| -> Vec3
|xyz: Vec3| -> Vec3
```

Initializes a `Vec3` with specified `x`, `y`, and `z` components.


### Example

```koto
from geometry import vec2, vec3

print! vec3()
check! Vec3{x: 0, y: 0, z: 0}

print! vec3 -1, 3
check! Vec3{x: -1, y: 3, z: 0}

print! vec3 10, 20, 30
check! Vec3{x: 10, y: 20, z: 30}

print! vec3 (vec2 -1, -2), 5
check! Vec3{x: -1, y: -2, z: 5}
```

## Rect

The `Rect` type represents a 2-dimensional rectangle, 
with a defined position and size. 

The position is interpreted as being at the center of the rectangle. 

Comparison operations are available, and the rect's components are iterable.

### Example

```koto
r = geometry.rect 10, 20, 30, 40
x, y, w, h = r
print! x, y, w, h
check! (10.0, 20.0, 30.0, 40.0)
```

## Rect.left

```kototype
|Rect| -> Number
```

Returns the position of rectangle's left edge.

### Example

```koto
# Create a rectangle centered at 0, 0
r = geometry.rect 0, 0, 200, 100
print! r.left()
check! -100.0
```

## Rect.right

```kototype
|Rect| -> Number
```

Returns the position of rectangle's right edge.

### Example

```koto
# Create a rectangle centered at 0, 0
r = geometry.rect 0, 0, 200, 100
print! r.right()
check! 100.0
```

## Rect.top

```kototype
|Rect| -> Number
```

Returns the position of rectangle's top edge.

### Example

```koto
# Create a rectangle centered at 0, 0
r = geometry.rect 0, 0, 200, 100
print! r.top()
check! 50.0
```

## Rect.bottom

```kototype
|Rect| -> Number
```

Returns the position of rectangle's bottom edge.

### Example

```koto
# Create a rectangle centered at 0, 0
r = geometry.rect 0, 0, 200, 100
print! r.bottom()
check! -50.0
```

## Rect.width

```kototype
|Rect| -> Number
```

Returns the width of the rectangle.

### Example

```koto
r = geometry.rect 0, 0, 200, 100
print! r.width()
check! 200.0
```

## Rect.height

```kototype
|Rect| -> Number
```

Returns the width of the rectangle.

### Example

```koto
r = geometry.rect 0, 0, 200, 100
print! r.height()
check! 100.0
```

## Rect.center

```kototype
|Rect| -> Vec2
```

Returns the center point of the rectangle.

### Example

```koto
r = geometry.rect -100, 42, 200, 100
print! r.center()
check! Vec2{x: -100, y: 42}
```

## Rect.x

```kototype
|Rect| -> Vec2
```

Returns the `x` component of the rectangle's center point.

### Example

```koto
r = geometry.rect -100, 42, 200, 100
print! r.x()
check! -100.0
```

## Rect.y

```kototype
|Rect| -> Vec2
```

Returns the `y` component of the rectangle's center point.

### Example

```koto
r = geometry.rect -100, 42, 200, 100
print! r.y()
check! 42.0
```

## Rect.contains

```kototype
|Rect, xy: Vec2| -> Vec2
```

Returns true if the given `Vec2` is located within the rectangle's
bounds.

### Example

```koto
from geometry import rect, vec2

r = rect 0, 0, 200, 200

print! r.contains vec2 50, 50
check! true
print! r.contains vec2 500, 500
check! false
```

## Rect.set_center

```kototype
|Rect, x: Number y: Number| -> Rect
|Rect, xy: Vec2| -> Rect
```

Sets the rect's center position to the given `x` and `y` coordinates, and
returns the rect.

### Example

```koto
from geometry import rect, vec2

r = rect 0, 0, 200, 200

print! r.set_center 10, 10
check! Rect{x: 10, y: 10, width: 200, height: 200}
print! r.set_center vec2()
check! Rect{x: 0, y: 0, width: 200, height: 200}
```

## Vec2

The `Vec2` type represents a 2-dimensional vector, with `x` and `y` coordinates.

All operators are implemented, and the vector's coordinates are iterable.

### Example

```koto
from geometry import vec2

print! (vec2 10, 20) + (vec2 30, 40)
check! Vec2{x: 40, y: 60}

v = vec2 50, 100
v *= vec2 0.5, 2
x, y = v
print! x, y
check! (25.0, 200.0)
print! v -= 50
check! Vec2{x: -25, y: 150}
```

## Vec2.angle

```kototype
|Vec2| -> Number
```

Returns the angle of the vector, expressed in radians.

### Example

```koto
from geometry import vec2

print! (vec2 1, 0).angle()
check! 0.0
print '{(vec2 0, 1).angle():.3}'
check! 1.571
print '{(vec2 -1, 0).angle():.3}'
check! 3.142
print '{(vec2 0, -1).angle():.3}'
check! -1.571
```

## Vec2.length

```kototype
|Vec2| -> Number
```

Returns the length of the vector.

### Example

```koto
from geometry import vec2

print! (vec2 0, 0).length()
check! 0.0
print! (vec2 3, 4).length()
check! 5.0
print! (vec2 -4, -3).length()
check! 5.0
```

## Vec2.x

```kototype
|Vec2| -> Number
```

Returns the `x` coordinate of the vector.

### Example

```koto
from geometry import vec2

print! (vec2 -1, 0).x()
check! -1.0
print! (vec2 3, 4).x()
check! 3.0
```

## Vec2.y

```kototype
|Vec2| -> Number
```

Returns the `y` coordinate of the vector.

### Example

```koto
from geometry import vec2

print! (vec2 0, -2).y()
check! -2.0
print! (vec2 3, 4).y()
check! 4.0
```

## Vec3

The `Vec3` type represents a 3-dimensional vector, with `x`, `y`, and `z` coordinates.

All operators are implemented, and the vector's coordinates are iterable.

### Example

```koto
from geometry import vec3

print! (vec3 10, 20, 30) + (vec3 40, 50, 60)
check! Vec3{x: 50, y: 70, z: 90}

v = vec3 50, 100, 150
v *= vec3 0.5, 2, -1
x, y, z = v
print! x, y, z
check! (25.0, 200.0, -150.0)
```

## Vec3.x

```kototype
|Vec3| -> Number
```

Returns the `x` coordinate of the vector.

### Example

```koto
from geometry import vec3

print! (vec3 -1, 0, 1).x()
check! -1.0
```


## Vec3.y

```kototype
|Vec3| -> Number
```

Returns the `y` coordinate of the vector.

### Example

```koto
from geometry import vec3

print! (vec3 -1, -2, -3).y()
check! -2.0
```

## Vec3.z

```kototype
|Vec3| -> Number
```

Returns the `z` coordinate of the vector.

### Example

```koto
from geometry import vec3

print! (vec3 10, 20, 30).z()
check! 30.0
```
