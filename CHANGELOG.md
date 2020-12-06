# Changelog

## Unreleased

### Added
- map.update


## [0.3.0] - 2020.12.06

### Added
- iterator.all
- iterator.any
- iterator.max
- iterator.min
- iterator.position
- iterator.skip
- string.size
- strings can now be used with the ordered comparison operators.

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
