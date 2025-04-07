# random

Utilities for generating random values in Koto.

At the core of the module is the `Rng` type, which is a seedable random
number generator. Each thread has access to a generator with a randomly
selected seed, or unique generators can be created with [`random.generator`](#generator).

The [xoshiro256++][xoshiro] algorithm is used to generate random values,
which is fast and portable, but _not_ cryptographically secure.

## bool

```kototype
|| -> Bool
```

Generates a random boolean using the current thread's generator.

### Example

```koto
# Seed the thread Rng so that we get predictable results
random.seed 99

print! random.bool()
check! false
print! random.bool()
check! true
```

## generator

```kototype
|| -> Rng
```

Creates an [`Rng`](#rng) with a randomly generated seed.

```kototype
|Number| -> Rng
```

Creates an [`Rng`](#rng) with a specified seed.



### Example

```koto
rng = random.generator 99
print! rng.pick (1, 2, 3)
check! 1
print! rng.bool()
check! true
```


## number

```kototype
|| -> Number
```

Generates a random number using the current thread's generator.

The number will be a floating point value in the range from 0 up to but not
including 1.

### Example

```koto
# Seed the thread Rng so that we get predictable results
random.seed 123

# Print random floats up to 3 decimal places
print '{random.number():.3}'
check! 0.646
print '{random.number():.3}'
check! 0.838
```

## pick

```kototype
|Indexable| -> Any?
```

Selects a random value from the input using the current thread's generator.

- If the input is empty, then `null` will be returned.
- If the input is a map, then a tuple containing the key and value of a
  randomly selected entry will be returned.
- If the input is a range, then the result will be an integer within the given
  range.
- If the input is some other indexable type (like a list or tuple),
  then a randomly selected element from the input will be returned.

### Example

```koto
# Seed the thread Rng so that we get predictable results
random.seed -1

print! random.pick (123, -1, 99)
check! -1
print! random.pick 10..20
check! 19
print! random.pick {foo: 42, bar: 99, baz: 123}
check! ('baz', 123)
print! random.pick []
check! null
```

## seed

```kototype
|Number| -> Null
```

Seeds the current thread's generator so that it produces predictable results.

### Example

```koto
from iterator import generate
from random import pick, seed

# Returns a tuple containing three numbers from 1 to 10
pick_3 = || generate((|| pick 1..=10), 3).to_tuple()

seed 1
print! pick_3()
check! (9, 8, 2)

seed 2
print! pick_3()
check! (8, 6, 7)

seed 1
print! pick_3()
check! (9, 8, 2)
```

## shuffle

```kototype
|Indexable| -> Any
```

Reorders the entries in a container so that they have a new randomly shuffled order,
and returns the container.

```koto
from random import seed, shuffle

x = [1, 2, 3, 4, 5]

seed 2
print! shuffle x
check! [1, 5, 4, 3, 2]
print! shuffle x
check! [3, 1, 4, 2, 5]

y = {a: 1, b: 2, c: 3}
print! shuffle y
check! {c: 3, a: 1, b: 2}
print! shuffle y
check! {c: 3, b: 2, a: 1}
```

## Rng

`Rng` is the `random` module's core random generator.

The [xoshiro256++][xoshiro] algorithm is used to generate random values,
which is fast and portable, but _not cryptographically secure_.

## Rng.bool

See [random.bool](#bool).

## Rng.number

See [random.number](#number).

## Rng.pick

See [random.pick](#pick).

## Rng.shuffle

See [random.shuffle](#shuffle).

## Rng.seed

See [random.seed](#seed).

[xoshiro]: https://prng.di.unimi.it
