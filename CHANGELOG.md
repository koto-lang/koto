# Changelog

The format of this changelog is based on
[Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

The Koto project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

- Num2 / Num4 improvements.
  - Elements can now be assigned via indexing.
    - e.g.
      ```koto
      x = num4 1, 2, 3, 4
      x[2..] = 99
      assert_eq x, (num4 1, 2, 99, 99)
      ```
  - Num2 and Num4 are now iterable.
    - e.g.
      ```koto
      x = num4 5, 6, 7, 8
      assert_eq x.keep(|n| n > 6).count(), 2
      ```
  - Added core operations:
    - `num2.iter` / `num4.iter`
    - `num2.length` / `num4.length`
    - `num2.max` / `num4.max`
    - `num2.min` / `num4.min`
    - `num2.normalize` / `num4.normalize`
    - `num2.product` / `num4.product`
- Indexing a string with a range starting from 'one past the end' is now
  supported.
- Throw and debug expressions can now be used more freely, in particular as
  expressions in match and switch arms.
  - e.g.
    ```koto
    match foo()
      0 then true
      1 then false
      x then debug x # debug would previously require an indented block
    ```

## [0.8.1] 2021.08.18

### Fixed

- Fixed a regression introduced in `v0.7.0` that prevented Maps from using a
  quoted string for the first entry's key while using block syntax.

## [0.8.0] 2021.08.17

### Added

- CLI improvements
  - The REPL now contains a help system that provides reference documentation for
    the core library.
  - An `--eval` option has been added to allow for direct evaluation of an
    expression.
- New features for Strings.
  - Strings now support indexing operations.
    - e.g.
      ```koto
      assert_eq "héllö"[1..3], "él"
      assert_eq "👋🥳😆"[1], "🥳"
      ```
  - Single-quotes can now be used to create strings, which can be useful when a
    string contains double quotes that would otherwise need to be escaped.
  - Modifiers can be used in formatting strings.
    - Borrowing from Rust's syntax, minimum and maximum widths can be specified
      for formatted values.
      - e.g.
        ```koto
        assert_eq ('{:6.2}'.format 1 / 3), '  0.33'
        assert_eq ('{:-^8}'.format "ab"), '---ab---'
        ```
  - `\x` and `\u` escape codes are now supported.
    - Borrowing Rust's syntax again, `\x` is followed by 2 hexadecimal digits
      representing an ASCII character in the range `\x00` to `\x7f`.
    - `\u` is followed by up to 6 hexadecimal digits surrounded by `{}` braces,
      representing a unicode character.
    - e.g.
      ```koto
      assert_eq '\x4f\x5f\x6f', 'O_o'
      assert_eq '\u{1f98b}', '🦋'
      ```
  - `string.bytes` has been added to provide access to a string's underlying
    byte sequence.
  - `string.split` can now take a function as its matching argument.
- New features for Iterators.
  - `iterator.intersperse` intersperses adjacent values in the iterator's output
    with copies of a provided value.
    - e.g.
      ```koto
      assert_eq ("a", "b", "c").intersperse("-").to_string(), "a-b-c"
      ```
  - `iterator.last` returns the last value returned by the iterator.
  - `iterator.to_string` produces a string from the iterator's output.
    - e.g.
      ```koto
      assert_eq (1..=5).to_string(), "12345"
      assert_eq ("x", "y". "z").to_string(), "xyz"
      ```
- I/O improvements.
  - Added `io.stdin`, `io.stdout`, and `io.stderr`.
  - New features for `File`:
    - `File.flush`
    - `File.read_line`
  - Reading and writing to files is now buffered.
- Meta maps can now have user-defined entries defined, using the `@meta` tag.
  - e.g.
    ```koto
    make_foo = |x, y|
      x: x
      y: y
      @meta get_x_plus_y: |self| self.x + self.y
    foo = make_foo 1, 2
    assert_eq foo.get_x_plus_y(), 3
    assert_eq foo.keys().to_tuple(), ("x", "y")
    ```

### Changed

- Items from the prelude now don't have to be imported for them to available
  in a script.
  - The core library is made available in the prelude by default, so core
    modules can be accessed in scripts without them being imported first.
    - e.g. `number.pi` is now a valid script, whereas previously
      `import number` would be required for `number` to be available.
- Tests are now defined using the meta map.
  - e.g. instead of `test_check_it_works: ...`,
    you now write `@test check_it_works: ...`.
  - Similarly, `pre_test:` and `post_test` are now defined as
    `@pre_test` and `@post_test`.
  - To define a tests map, export the map as `@tests` rather than `tests`.
  - e.g.
    ```koto
    export @tests =
        @pre_test: |self|
            self.test_data = 1, 2, 3
        @post_test: |self|
            self.test_data = ()
        @test data_size: |self|
            assert_eq self.test_data.size(), 3
    ```
- External value types are now simpler to implement, with a dedicated
  `ExternalValue` value type that consists a blob of `ExternalData` plus a
  `MetaMap` where implementation functions can be defined.
  - An external value's meta map can be shared between external value instances,
    with `lazy_static` used for lazy initialization.
- Changes to the `koto` module:
  - `koto.args` is now a Tuple instead of a List.
  - `koto.current_dir` has been moved to `io.current_dir`.
  - `koto.script_dir` and `koto.script_path` are now empty by default.
- Ranges that have operations starting on an intended following line can now be
  written without parentheses.
  - e.g.
    ```koto
    0..10 # Previously this would have to be written as (1..10)
      .keep |x| x % 2 == 0
      .to_tuple()
    # (0, 2, 4, 6, 8)
    ```
- Omitting the value after a key in a map declaration is now only allowed when
  using inline syntax.
- `string.print` has been moved to `io.print`.
  - `print` was previously in `string` to allow for import-free printing,
    but now that core modules in the prelude `io.print "hello"` can be expressed
    without imports.
  - `io.print` will now accept any value without a formatting string.
- The custom logging interface has been replaced by the `KotoFile` trait,
  with stdin, stdout, and stderr available to be overridden.

### Removed

- `ExternalDataId` has been removed as a `Value` type, see the note on
  `ExternalValue` above.

## [0.7.0] 2021.03.27

### Added

- Direct access to the module's export map is now allowed via `koto.exports()`.
- Logging behaviour via print and debug logging can now be customized.
- Koto can now be compiled to wasm.
- Operator overloading for maps is now supported.
  - e.g.
    ```koto
    foo = |x|
      x: x
      @+: |self, other| foo self.x + other.x
    assert_eq (foo(10) + foo(20)), foo(30)
    ```
- Binary, octal, and hex notation for number literals is now supported.
  - e.g.
    ```koto
    assert_eq 0b1000, 8
    assert_eq 0o1000, 512
    assert_eq 0x1000, 4096
    ```
- Bitwise operations are now available for integers.
  - `number.and`
  - `number.flip_bits`
  - `number.or`
  - `number.shift_left`
  - `number.shift_right`
  - `number.xor`
- `throw` can now be used for throwing errors.
  - Strings can be used as an error message:
    `throw "Was für ein Fehler!"`
  - Maps that implement `@display` can also be thrown:
    ```
    throw
      data: foo
      @display: |self| "Che errore! - {}".format self.data
    ```

### Changed

- Captured values in functions are now immutable.
  - e.g.
    ```koto
    x = 100
    f = |n|
      x = x + n # Assigning to x here now only affects the local copy of x
    debug f 42  # 142
    debug x     # 100 - The value of x in this scope is unchanged
    ```
  - Captured values can now be thought of as 'hidden arguments' for a function
    rather than 'hidden mutable state', which simplifies things quite a bit.
  - If mutable state is required then you can use a list or map, e.g.
    ```koto
    state = {x: 100}
    f = |n|
      state.x = state.x + n # The function has a local copy of the state,
                            # which shares its data with the outer scope's copy.
    debug f 42    # 142
    debug state.x # 142
    ```
- Runtime errors now provide a full backtrace.
- Keywords can now be used as identifiers in lookups, e.g. `foo.and()` was
  previously disallowed.
- Maps are now printed in the REPL with keys only.

## [0.6.0] 2021.01.21

### Added

- Core Ops
  - `range.expanded`
  - `range.union`

### Changed

- Function calls without parentheses now require commas to separate arguments.
  - e.g. `f a b c` now needs to be written as `f a, b, c`.
  - Care needs to be taken when adapting programs to this change.
    - e.g. `f a, b c` was parsed as two separate expressions
      (i.e. `(f a), (b c)`), and it's now parsed as `f(a, (b c))`.
- `match` when used without a value to match against has been renamed to
  `switch`.
- Error messages in core ops that call functors have been made a bit clearer.
- Core ops that accept function arguments can now take external functions.
  - e.g.
    ```koto
    x = [[1, 2, 3], [1], [1, 2]]
    x.sort list.size
    assert_eq x [[1], [1, 2], [1, 2, 3]]
    ```
- The `Koto` struct now returns a concrete error type instead of a `String`.
- It's no longer necessary to call helper functions to get formatted source
  extracts for errors.
- Whitespace is no longer required after operators,
  e.g. `1+1==2` would previously trigger a parsing error.

### Fixed

- Error messages produced in the functor passed to `iterator.fold` were reported
  as coming from `iterator.each`.
- Error messages associated with accessed IDs now have the correct spans.
  - e.g.
    ```
    x = (1..10).fold 42
    ```
    - Previously the error (wrong arguments for `.fold`) would be connected with
      the range rather than the function call.

## [0.5.0] 2020.12.17

### Added

- Core Ops
  - `iterator.chain`
  - `iterator.product`
  - `iterator.sum`
  - `list.clear`
  - `list.swap`
  - `map.clear`
  - `map.get_index`
  - `map.sort`
  - `number.is_nan`
  - `number.to_float`
  - `number.to_int`
  - `os.cpu_count`
  - `os.physical_cpu_count`
  - `string.ends_with`
  - `string.starts_with`
  - `tuple.first`
  - `tuple.last`
  - `tuple.sort_copy`
- Core Constants
  - `number.e`
  - `number.infinity`
  - `number.nan`
  - `number.negative_infinity`
- `match` improvements
  - `else` can be now used as the fallback arm in a match expression.
  - The value to match against is now optional, and when it's ommitted then
    so are match patterns.
    - e.g.
      ```koto
      n = 0
      match
        n == 0 then "zero"
        n == 1 then "one"
        else "???"
      ```
    - _Note_ (20.12.2020): After v0.5.0 this form of expression was renamed to
      `switch`.
  - The results of list/map accesses or function calls can be used as match
    patterns.
    - e.g.
      ```koto
      match x
        f y then "x == f y"
        m.foo then "x == m.foo"
        z[10] then "x == z[10]"
      ```
  - match arms that have indented bodies can now optionally use `then`,
    which can look clearer when the match pattern is short.
    - e.g.
      ```koto
      match x
        0 then # <-- `then` was previously disallowed here
          "zero"
        1 then
          "one"
      ```
- Tuples may now be added to lists with the `+` and `+=` operators.
  - e.g.
    ```koto
    x = [1, 2] + (3, 4)
    assert_eq x [1, 2, 3, 4]
    ```

### Changed

- `thread.join` now returns the result of the thread's function.
- Numbers now can either be integers or floats.
  - The integer representation is `i64`.
  - Arithmetic involving only integers will produce an integer result,
    otherwise the result will be floating point.
- The RWLock implementation used in Koto is now the one from
  [parking_lot](https://crates.io/crates/parking_lot).
  - Performance improvements of up to 13% were seen in testing.
- Accessing the runtime's prelude is now performed via `Koto::prelude()` rather
  than via `Koto::context_mut()`, which has been removed.
  - The prelude was the only reason to expose the context, so it's cleaner to
    make this explicit.
  - Behind this change is a small performance improvement whereby core
    operations have one RWLock fewer to get past.
- `list.sort` and `map.sort` can now take an optional function to customize the
  sorting behaviour.
- The ordering of entries is now preserved when calling `map.remove`.

### Fixed

- `else` and `else if` blocks with unexpected indentation will now trigger a
  parser error.
- Multi-assignment of values where the values are used in the expressions now
  works as expected.
  - e.g.
    ```koto
    a, b = 1, 2
    a, b = b, a
    # Previously this would result in b being re-assigned to itself
    assert_eq b 1
    ```
- Generator functions can now capture non-local values.
- `1.exp()` is now parsed correctly as a number followed by a call to `exp()`,
  rather than `1.e` followed by `xp()`.
- `string.split` now works correctly when used with multi-character patterns.

## [0.4.0] 2020.12.10

### Added

- Core Ops
  - `iterator.min_max`
  - `list.copy`
  - `list.deep_copy`
  - `map.copy`
  - `map.deep_copy`
  - `map.update`
  - `tuple.deep_copy`
- Strings are now iterable by default
- Tuples or lists in function arguments can be unpacked automatically.
  - e.g. `f = |a, (b, [c, d])| a + b + c + d`
- Num2 and num4 values can now used in unpacking expressions
  - e.g.
    ```koto
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
  - `iterator.all`
  - `iterator.any`
  - `iterator.max`
  - `iterator.min`
  - `iterator.position`
  - `iterator.skip`
  - `string.size`
- Strings can now be used with the ordered comparison operators.

### Changed

- Map blocks can now be used in return and yield expressions.
- `iterator.each` and `iterator.keep` now collect iterator pairs into tuples.
- Space-separated function calls are allowed in function args when the arg is on
  a new line.
- Unparenthesized expressions can now be used for range boundaries.
  - e.g. `(1 + 1)..(2 + 2)` can now be written as `1 + 1..2 + 2`.

### Removed

- Vim support has been moved to [its own repo][vim].

### Fixed

- `iterator.fold`, `list.retain`, and `list.transform` could cause runtime
  errors or stack overflows when being called after other functions.
  - [Bug report](https://github.com/koto-lang/koto/issues/6)

[vim]: https://github.com/koto-lang/koto.vim

## [0.2.0] - 2020.12.02

### Added

- `iterator.count`
- `string.chars`
- `tuple.contains`

### Changed

- `koto.script_dir` is now canonicalized and includes a trailing slash.
- `koto.script_path` is now canonicalized.

### Fixed

- Multiline strings broke following spans.

## [0.1.0] - 2020.12.01

- Initial release
