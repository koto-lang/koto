# num2

A `Num2` in Koto is a packed pair of 64bit floating-point numbers,
which can be useful when dealing with operations that require pairs of numbers,
like 2D coordinates.

Element-wise arithmetic operations between Num2s are available,
while operations with Numbers apply the number to each element.

## Example

```koto
x = num2 1, 2
y = num2 3, 4
x + y
# num2(4, 6)

x[0] + y[0]
# 4

x + 10
# num2(11, 12)

x[0] = -1
x
# num2(-1, 2)
```

# Reference

- [iter](#iter)
- [max](#max)
- [min](#min)
- [product](#product)
- [sum](#sum)

## iter

`|Num2| -> Iterator`

Returns an iterator that iterates over the list's values.

Num2 values are iterable, so it's not necessary to call `.iter()` to get access
to iterator operations, but it can be useful sometimes to make a standalone
iterator for manual iteration.

### Example

```koto
x = (num2 3, 4).iter()
x.skip(1)
x.next()
# 4
```

## max

`|Num2| -> Float`

Returns the value of the largest element in the Num2.

### Example

```koto
x = num2(10, 20)
x.max()
# 20
```

## min

`|Num2| -> Float`

Returns the value of the smallest element in the Num2.

### Example

```koto
x = num2(10, 20)
x.min()
# 10
```

## product

`|Num2| -> Float`

Returns the result of multiplying the Num2's elements together.

### Example

```koto
x = num2(10, 20)
x.product()
# 300
```

## sum

`|Num2| -> Float`

Returns the result of adding the Num2's elements together.

### Example

```koto
x = num2(10, 20)
x.sum()
# 30
```
