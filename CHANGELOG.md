# Changelog

## Unreleased

### Added
- Core Ops
  - iterator.chain
  - list.clear
  - list.swap
  - map.clear
  - map.get_index
  - os.cpu_count
  - os.physical_cpu_count
  - tuple.first
  - tuple.last
  - tuple.sort_copy
- `match` improvements
  - `else` can be now used as the fallback arm in a match expression.
  - The value to match against is now optional, and when it's ommitted then
    so are match patterns.
    - e.g.
      ```
      n = 0
      match
        n == 0 then "zero"
        n == 1 then "one"
        else "???"
      ```
- Tuples may now be added to lists with the `+` and `+=` operators.
  - e.g.
    ```
    x = [1, 2] + (3, 4)
    assert_eq x [1, 2, 3, 4]
    ```

### Changed
- thread.join now returns the result of the thread's function

### Fixed
- else and else if blocks with the incorrect indentation will now trigger a
  parser error.


## [0.4.0] 2020.12.10

### Added
- Core Ops
  - iterator.min_max
  - list.copy
  - list.deep_copy
  - map.copy
  - map.deep_copy
  - map.update
  - tuple.deep_copy
- Strings are now iterable by default
- Tuples or lists in function arguments can be unpacked automatically.
  - e.g. `f = |a, (b, [c, d])| a + b + c + d`
- Num2 and num4 values can now used in unpacking expressions
  - e.g.
    ```
    x = num2 1 2
    a, b = x
    assert_eq b 2
    ```

### Fixed
- iterator.consume and iterator.count now propagate errors correctly.
- Wildcard function args that weren't in last position would cause arguments to
  be assigned to the wrong IDs.

### Removed
- The copy expression has been removed in favour of copy / deep_copy operations
  on container types.


## [0.3.0] - 2020.12.06

### Added
- Core Ops
  - iterator.all
  - iterator.any
  - iterator.max
  - iterator.min
  - iterator.position
  - iterator.skip
  - string.size
- Strings can now be used with the ordered comparison operators.

### Changed
- Map blocks can now be used in return and yield expressions.
- iterator.each and iterator.keep now collect iterator pairs into tuples.
- Space-separated function calls are allowed in function args when the arg is on
  a new line.
- Unparenthesized expressions can now be used for range boundaries.
  - e.g. `(1 + 1)..(2 + 2)` can now be written as `1 + 1..2 + 2`.

### Removed
- Vim support has been moved to [its own repo][vim].

### Fixed
- iterator.fold, list.retain, and list.transform could cause runtime errors or
  stack overflows when being called after other functions.
  - [Bug report](https://github.com/koto-lang/koto/issues/6)

[vim]: https://github.com/koto-lang/koto.vim


## [0.2.0] - 2020.12.02

### Added
- iterator.count
- string.chars
- tuple.contains

### Changed
- koto.script_dir is now canonicalized and includes a trailing slash.
- koto.script_path is now canonicalized.

### Fixed
- Multiline strings broke following spans.


## [0.1.0] - 2020.12.01
- Initial release
