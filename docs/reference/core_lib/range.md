# range

Ranges are represented in Koto by start and end integers.

## Creating a range

Ranges are created using the syntax `start..end` for a non-inclusive range.

`start..=end` is used for an inclusive range, although this is currently just
syntax sugar for creating a non-inclusive range that includes the `end` value.
This simplifies the internal implementation but could be confusing for users,
so this may change in the future.

Descending ranges are allowed, so the `start` value can be smaller than `end`.

### Example

```koto
# Non-inclusive range
x = 10..20
# 10..20
x.start()
# 10
x.end()
# 20

# Inclusive range
x2 = 100..=200
# 100..201
x2.contains 200
# true

# Descending non-inclusive range
x3 = 3..0
# 3..0
x3.start()
# 3
x3.to_tuple()
# (3, 2, 1)

# Descending inclusive range
x4 = 3..=0
# 3..-1
x4.to_list()
# [3, 2, 1, 0]
```

# Reference

- [contains](#contains)
- [end](#end)
- [expanded](#expanded)
- [size](#size)
- [start](#start)
- [union](#union)

## contains

`|Range, Number| -> Bool`

Returns true if the provided number is within the range, and false otherwise.

### Example

```koto
(10..20).contains 15
# true

(200..=100).contains 100
# true

x = 1..10
x.contains -1
# false
```

## end

`|Range| -> Int`

Returns the `end` value of the range.

### Example

```koto
(50..100).end()
# 100

(10..0).end()
# 0
```

### See also

- [start](#start)

## expanded

`|Range, Number| -> Range`

Returns a copy of the input range which has been 'expanded' in both directions
by the provided amount. For an ascending range this will mean that `start` will
decrease by the provided amount, while `end` will increase.

Negative amounts will cause the range to shrink rather than grow.

### Example

```koto
(10..20).expanded 5
# 5..25

(10..20).expanded -2
# 12..18

(5..-5).expanded 5
# 10..-10

(5..-5).expanded -5
# 0..0

(5..-5).expanded -10
# -5..5
```

## size

`|Range| -> Int`

Returns the size of the range.
This is equivalent to `range.end() - range.start()`.

Note that for descending ranges, a negative value will be returned.

### Example

```koto
(10..20).size()
# 10

(100..=200).size()
# 101

(20..0).size()
# -20
```

## start

`|Range| -> Int`

Returns the `start` value of the range.

### Example

```koto
(50..100).start()
# 50

(10..0).start()
# 10
```

### See also

- [end](#end)

## union

`|Range, Number| -> Range`

Returns the union of the range and a provided number.

If the number falls outside of the range then the resulting range will be
expanded to include the number.

`|Range, Range| -> Range`

Returns the union of two ranges.

The resulting range will encompass all values that are contained in the two
ranges, and any values that lie between them.

### Example

```koto
(0..10).union 5
# 0..10

(0..10).union 99
# 0..100

a = 10..20
b = 40..50
a.union b
# 10..50
```
