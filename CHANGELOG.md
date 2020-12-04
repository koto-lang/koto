# Changelog

## Unreleased

### Added
- iterator.all
- iterator.any
- string.size

### Changed
- Map blocks can now be used in return and yield expressions
- iterator.each and iterator.keep now collect iterator pairs into tuples

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
- koto.script_dir is now canonicalized and includes a trailing slash
- koto.script_path is now canonicalized

### Fixed
- Multiline strings broke following spans


## [0.1.0] - 2020.12.01
- Initial release
