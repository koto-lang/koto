# range

## contains

```kototype
|Range, Number| -> Bool
```

Returns true if the provided number is within the range, and false otherwise.

```kototype
|Range, Range| -> Bool
```

Returns true if the provided range is entirely contained within the range,
and false otherwise.

### Example

```koto
print! (10..20).contains 15
check! true

print! (200..=100).contains 100
check! true

x = 1..10
print! x.contains -1
check! false

print! (10..20).contains 14..18
check! true

print! (100..200).contains 50..250
check! false
```

## end

```kototype
|Range| -> Number
```

Returns the `end` value of the range.

### Example

```koto
print! (50..100).end()
check! 100

print! (10..0).end()
check! 0
```

### See also

- [start](#start)

## expanded

```kototype
|Range, amount: Number| -> Range
```

Returns a copy of the input range which has been 'expanded' in both directions
by the provided `amount`. 

For an ascending range this will mean that `start` will decrease by the provided
amount, while `end` will increase.

Negative amounts will cause the range to shrink rather than grow.

### Example

```koto
print! (10..20).expanded 5
check! 5..25

print! (10..20).expanded -2
check! 12..18

print! (5..-5).expanded 5
check! 10..-10

print! (5..-5).expanded -5
check! 0..0

print! (5..-5).expanded -10
check! -5..5
```

## intersection

```kototype
|Range, Range| -> Range
```

Returns a range representing the intersecting region of the two input ranges.

If there is no intersecting region then `null` is returned.

### Example

```koto
print! (10..20).intersection 5..15
check! 10..15

print! (100..200).intersection 250..=150
check! 150..200

print! (0..10).intersection 90..99
check! null
```

## is_inclusive

```kototype
|Range| -> Bool
```

Returns true if the range has a defined end which is inclusive.

### Example

```koto
print! (10..20).is_inclusive()
check! false

print! (1..=10).is_inclusive()
check! true

print! (100..).is_inclusive()
check! false
```

## start

```kototype
|Range| -> Number
```

Returns the `start` value of the range.

### Example

```koto
print! (50..100).start()
check! 50

print! (10..0).start()
check! 10
```

### See also

- [end](#end)

## union

```kototype
|Range, Number| -> Range
```

Returns the union of the range and a provided number.

If the number falls outside of the range then the resulting range will be
expanded to include the number.

```kototype
|Range, Range| -> Range
```

Returns the union of two ranges.

The resulting range will encompass all values that are contained in the two
ranges, and any values that lie between them.

### Example

```koto
print! (0..10).union 5
check! 0..10

print! (0..10).union 99
check! 0..100

a = 10..20
b = 40..50
print! a.union b
check! 10..50
```
