# Changelog

The format of this changelog is based on
[Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

The Koto project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.15.0] Unreleased

### Added 

#### Language

- Type hints with runtime type checks have been added ([#298](https://github.com/koto-lang/koto/issues/298)).
  - Thanks to [@Tarbetu](https://github.com/Tarbetu) for the contributions.
- `export` can be used with multi-assignment expressions.
  - e.g. expressions like `export a, b, c = foo()` are now allowed.
- Maps now support `[]` indexing, returning the Nth entry as a tuple.

#### Core Library

- `tuple.sort_copy` now supports sorting with a key function, like `list.sort`.

### Changed

#### Language

- `await`, `const`, and `let` have been reserved as keywords for future use.
- The type of a number is now always `Number`, rather than distinguishing
  between `Int` and `Float`.
- The `>>` pipe operator has been replaced with `->`.
  - This aligns it with the `->` function output type syntax, which avoids 
    having two different special-case operators related to function output.
- Error messages have been improved when calling core library functions with
  incorrect arguments.

#### API

- The line and column numbers referred to in spans are now zero-based. 
- Functions that previously took `Option<PathBuf>` now take `Option<&Path>`.
- `AstIndex` and `ConstantIndex` are now newtypes that wrap `u32`.
- `Node::Lookup` has been renamed to `Node::Chain`, and `LookupNode` is now 
  `ChainNode`.
- `type_error` has been renamed to `unexpected_type`.  
  - `type_error_with_slice` has been replaced by `unexpected_args` and
    `unexpected_args_after_instance`. 

### Removed

#### API

- `koto_parser::MapKey` and `IdOrString` have been removed, map keys and import 
  IDs are now represented as AST nodes.
- `Node::NamedCall` has been removed, with all calls represented by expression
  chains.

### Fixed

#### Language

- Calling `.next()` on an exhausted generator no longer causes a panic.
  - Thanks to [@edenbynever](https://github.com/edenbynever) for the fix.

## [0.14.0] 2024.04.17

### Added 

#### API

- `KMap::get` has been introduced as simpler alternative to 
  `KMap::data().get().cloned()`.

#### Libs

- Markdown docs have been added for the extra libs.
- `random.pick` can now be used with objects and maps that implement `@[]`

### Changed

#### API 

- The use of `CallArgs` has been simplified with the introduction of `From`
  implementations for single values, arrays, and slices. 
  - `CallArgs::None` has been removed, instead you can pass in `&[]`.
- The `run_function`/`run_instance_function` methods in `Koto` and `KotoVm` have
  been renamed to `call_function` and `call_instance_function`.

#### Libs

- `regex.find_all` now returns `null` when no matches are found.
- `regex.captures` now doesn't add extra by-index entries for named capture
  groups. `map.get_index` can be used to explicitly retrieve groups by index.

### Removed

#### Language

- The `@not` metakey has been removed.

#### API

- `Koto::run_exported_function` has been removed. Functions can be accessed via
  `Koto::exports().get()` and then called with `Koto::call_function()`.
- `Koto::run_with_args` has been removed. For equivalent behaviour, 
  `Koto::set_args` can be called before calling `Koto::run`.

### Fixed

#### Language

- Arguments in paren-free function calls no longer require whitespace after
  commas. e.g. `f 1,2,3` would previously be rejected.


## [0.13.0] 2024.04.05

### Added

#### Language

- The `+` operator has been reintroduced for tuples, lists, and maps.
- Raw strings are now supported. Any string prefixed with `r` will skip
  character escaping and string interpolation.
- Formatting options have been added for interpolated strings.
- `import` expressions can now use `as` for more ergonomic item renaming.
- Assignments can now be used in `while`/`until` conditions.
- Unpacked assignments with a single value on the RHS are now accepted, 
  with remaining values being set to `null`.
  - e.g. `a, b, c = 42` will assign `42` to `a`, and `null` to `b` and `c`.
- The `@size` metakey (along with `KObject::size`) has been added to allow 
  custom value types to work with argument unpacking and pattern matching.
  - The general rule is now that matching works with any value that declares a
    size and supports indexing.

#### API

- The `koto_derive` crate has been introduced containing derive macros that make
  it easier to implement `KotoObject`s.
- `Koto::run_instance_function` has been added.
- `Ptr`/`PtrMut` now have an associated `ref_count` function.
- `KMap::clear` has been added.
- A maximum execution duration can now be defined in `KotoVmSettings`, 
  with a timeout error being returned when the deadline is reached.

#### Core Library

- Dynamic compilation and evaluation features (`koto.load` and `koto.run`) have 
  been added, thanks to [@alisomay](https://github.com/alisomay).
- `koto.size` has been added (and added to the prelude), 
  replacing the various type-specific `.size()` functions.
- `string.char_indices` has been added to support the switch to byte-based
  indexing.

#### Libs

- A `regex` module has been added, thanks to [@jasal92](https://github.com/jasal82).
- `iterator.once` has been added.

### Changed

#### Language

- String interpolation has been updated:
  - The `$` prefix has been removed from interpolated expressions.
    e.g. `'${1 + 1}'` is now `{1 + 1}`
  - Id-only interpolation (without the `{}` braces) has been removed.
    e.g. `'$foo` is now `{foo}`
  - The `\$` escape sequence has been replaced with `\{`.
- Pattern matching and function argument unpacking now use parentheses for all
  container types. 
  - Any uses of `[]` brackets to match against lists can be updated to use
    parentheses instead.
- Indexing operations on strings now access bytes instead of grapheme clusters.
  - This is to avoid the non-linear performance cost of indexing by cluster.
  - To access clusters via indexing, `string.char_indices` can be called first to
    retrieve valid indices. 

#### Core Library

- `io.print` no longer implicitly treats its first argument as a format string. 
  Interpolated strings should be used instead.
- `iterator.next`/`next_back` and `Peekble.peek`/`peek_back` now return
  `IteratorOutput` for output values, and `null` when the iterator is exhausted.
  - `.get()` needs to be called on the output to get the underlying value.
- `map.with_meta_map` has been renamed to `with_meta`, and `get_meta_map` has
  been renamed to `get_meta`.
- `string.to_number` changes:
  - `0x`, `0o`, and `0b` prefixes are understood for parsing hex, octal, or
    binary numbers respectively.
  - An overload has been added that accepts a number base between 2 and 36.
  - If the string doesn't contain a number, `null` is now returned instead of an
    exception being thrown.

#### API

- `Vm` has been renamed to `KotoVm` for the sake of clarity.
- `Value` has been renamed to `KValue` for consistency with the other core
  runtime value types, and to avoid polluting the prelude with a generic name.
- The VM-specific parts of `KotoSettings` are now defined via `KotoVmSettings`.
- The `KotoLookup` trait has been replaced with `KotoEntries`.
- Objects can be compared with `null` on the LHS without having to implement 
  `KotoObject::equal` and/or `not_equal`.
- `KRange` initialization has been revamped to support `From` for
  `RangeBounds<i64>`.

#### Internals

- The Koto runtime is now thread-safe by default, with the previous
  single-threaded behaviour available via the `rc` feature.
  - The `rc` variant has slightly better performance at the cost of thread
    safety.

#### CLI

- The REPL `config.koto` settings have all been moved into a `repl` sub-map.
  - e.g. 
    `export { edit_mode: 'vi' }` is now `export { repl: { edit_mode: 'vi' }}`
- The `--import_tests`/`-T` CLI option will now run tests in the main script
  along with any tests from imported modules.

### Removed 

#### Core Library

- `koto.deep_copy` has been removed from the prelude.
- `string.format` has been removed now that formatting options have been added
  to interpolated strings.
  - If dynamically generating format strings was useful, then this can still be
    achieved by evaluating a generated string with `koto.run`.

#### API

- `ObjectEntryBuilder` has been replaced with macros from `koto_derive`.
- `KMap::add_map` and `KMap::add_value` have been removed, `KMap::insert` now
  accepts any value that implements `Into<Value>` and can be used instead.
- `koto::Error/Result` have been replaced with re-exports from `koto_runtime`.

### Fixed

#### Language

- Chained compound assignment operators are now right-associative and all share 
  the same precedence.


## [0.12.0] 2023.10.18

### Added

#### Language

- Ellipses can now be used when unpacking nested function args.
  - e.g. 
    ```koto
    f = |(a, b, others...)| a * b + others.sum()
    f (10, 100, 1, 2, 3)
    # 1006
    ```
- Meta map improvements
  - Compound assignment operators (`@+=`, `@*=`, etc.) can now be implemented 
    in meta maps and external values.
  - The function call operator (`@||`) can be implemented to values that behave 
    like functions.
  - Values that implement `@[]` can now be used in unpacking assignment
    expressions.
  - `@next` and `@next_back` meta keys have been added to enable custom iterators.
- `export` can now be used with maps as well as single-value assignments.
  - e.g. 
    ```koto
    a, b, c = 1, 2, 3
    export { a, b, c, foo: 42 }
    ```

#### Libs

- New `color` and `geometry` libs have been added, and are available by default 
  in the CLI.
- `koto.hash` has been added to allow value hashes to be accessed.
- The `copy`/`deep_copy` functions have been merged into the `koto` module, 
  and made available in the prelude.
- `range` additions:
  - `range.contains` can now accept a range as an argument.
    - e.g. 
      ```koto
      (10..30).contains 15..25
      # true
      ```
  - `is_inclusive` and `intersection` have been added.
- `iterator` additions:
  - `iterator.next_back`
  - `iterator.peekable`
  - `iterator.step`
  - `iterator.take` has a new overload that takes a predicate.

#### Internals

- The `KotoObject` trait has been introduced to simplify creating custom object 
  types, replacing `ExternalValue`.
- Preludes are now available in the `koto` and `koto_runtime` crates.
- `Ptr<T>` and `PtrMut<T>` wrappers have been introduced as the core memory
  types for the runtime, replacing uses of `Rc<T>` and `Rc<RefCell<T>>`.

#### REPL

- Added support for disabling colored output with the `NO_COLOR` environment
  variable.
- The REPL has been reimplemented with 
  [rustyline](https://github.com/kkawakam/rustyline)
  - History is now maintained between sessions.
  - Emacs / VI key bindings have been added.
- A `config.koto` file can be written to maintain REPL settings.

### Changed

#### Rust MSRV

- The minimum supported Rust version is now the latest stable version.

#### Language

- `self` is now provided implicitly in functions and doesn't need to be declared
    as an argument.
- Multi-assignment now unpacks values via iteration rather than by indexing.
  - e.g.
    ```koto
    a, b, c = (1..10).each |n| n * 10
    # 10, 20, 30
    ```
  - Iteration is also used when unpacking for loop arguments.
    - e.g. 
    ```koto
    for a, b, c in (1..10).windows 3
      debug a, b, c
      # 1, 2, 3
      # 2, 3, 4
      # ...
    ```
- Ranges now preserve whether or not they're inclusive.
- `File`s now implement `@display`, showing their paths.
- `Tuple`s now share data when sub-tuples are made via indexing or unpacking, 
    avoiding unnecessary copies. 
- Import nested items directly is no longer allowed
  - e.g. `import foo.bar` now needs to be written as `from foo import bar`.

### Libs

- The various `.copy`/`.deep_copy` module functions have been merged into 
  `koto.copy`/`koto.deep_copy`, which have also been added to the prelude.
- `iterator.chunks`, `.cycle`, and `.windows` now cache initial iterator output
  rather than relying on copying the adapted iterator.

#### Internals

- `Value` no longer implements `fmt::Display`, instead `value_to_string` can be
  called on `Koto` or the Vm to get a string.
- `Value::ExternalValue` has been replaced by `Value::Object`.
- External functions have been simplified, with a `CallContext` provided that
  provides access to the VM and its arguments.
  - Functions that need access to the `self` instance can access it via
    `CallContext::instance`.
- The core Koto runtime types have been renamed for consistency, 
  and now use a `K` prefix to help disambiguate them in context 
  (e.g. `KIterator` vs. `Iterator`).
- `KTuple::data` has been removed, with a `Deref` impl to `&[Value]` taking
  its place.
- Type strings and strings returned by `KotoFile` implementations are now 
  expected to be `KString`s.
- `unexpected_type_error_with_slice` has been renamed to
  `type_error_with_slice`, and has had the prefix argument removed.
- `DataMap::get_with_string` has been replaced with a simplified `ValueKey`
  implementation that allows for efficient `&str` accesses without the
  intermediate steps.
- Implementing `KotoFile` has been made easier, with the `Display + Debug`
  constraint replaced with a required `id()` function.
- `KotoError` and `KotoResult` are now `koto::Error` and `koto::Result`.
- `Koto::run_function_by_name` is now `Koto::run_exported_function`.

### Removed

#### Packed number removal

- The `Num2` and `Num4` types have been removed.
  - Some of the use cases for these types are covered by the new `color` and 
    `geometry` libs.
  - See https://github.com/koto-lang/koto/issues/201 for removal rationale.

#### Core Library

- `list.with_size` has been removed in favour of using `iterator.to_list`.
  - e.g.
    ```koto
    # Instead of:
    list.with_size 5, 'x'
    # You can use:
    iterator.repeat('x', 5).to_list()
    ```
- `string.slice` has been removed in favour of `[]` indexing. 
  - e.g.
    ```koto
    # Instead of:
    "hello".slice 2, 4
    # You can use:
    "hello"[2..4]
    ```

### Fixed

- Ignored values (i.e. `_` or values with a `_` prefix) will now trigger a 
  compilation error when they're accessed.
  - e.g. 
    ```koto
    _x = 42
    debug _x
    #     ^^ This will now cause a compilation error
    ```

## [0.11.0] 2022.07.14

### Added

#### Language

- The `null` keyword has been introduced, which is a more explicit way of
  declaring a non-value than `()`.
  - A consequence of this addition is that formatted JSON is now accepted as
    valid Koto.
    - e.g.
      ```koto
      data = {
        "empty": null,
        "nested": {
          "number": 123,
          "string": "hello"
        }
      }

      data.nested.number
      # 123
      ```
- Koto values now coerce to Bool in boolean contexts, with `false` and `null`
  evaluating to `false`, with all other values evaluating to `true`.
  - e.g.
    ```koto
    x = null
    y = x or 42
    y
    # 42
    ```
- Maps can now implement `@iterator`, which allows you to define custom
  iteration behaviour.
  - e.g.
    ```koto
    foo = |n|
      n: n
      @iterator: |self| 1..=self.n
    foo(3).to_tuple()
    # (1, 2, 3)
    ```
- Num2 and Num4 values can now be iterated over in a for loop.
- Empty tuples can be declared by including a trailing comma in parentheses,
  e.g. `(,)`.
- Wildcard arguments (declared with `_`) can now optionally have names
  following the underscore.
  - e.g.
    ```koto
    # Before
    x, _, z = 1, 2, 3
    # After
    x, _unused, z = 1, 2, 3
    ```
- Loop improvements
  - The result of loop expressions (`for`, `while`, `until`, and `loop`) can now
    be assigned to a value, with the default result being the final expression 
    in the loop body. 
    - If no loop iterations are performed then the result is `null`.
  - The `break` keyword can now take an expression, which will be returned as 
    the result of the loop.
    - e.g. 
      ```koto
      y = for x in 0..=10
        if x == 5
          break x * x 
      y
      # 25
      ```

#### Core Library

- New additions:
  - `iterator`
    - `chunks`, `find`, `flatten`, `generate`, `repeat`, `reversed`,
      `to_num2`, `to_num4`, `windows`
  - `list`
    - `extend`, `resize_with`
  - `map`
    - `extend`, `get_meta_map`, `with_meta_map`
  - `number`
    - `acosh`, `asinh`, `atanh`, `atan2`, `lerp`
    - `pi_2`, `pi_4`
  - `num2`
    - `lerp`, `make_num2`, `with`
    - `x`, `y`
    - `angle`
  - `num4`
    - `lerp`, `make_num4`, `with`
    - `r`, `g`, `b`, `a`, `x`, `y`, `z`, `w`
  - `os`
    - `time`
      - Provides information about the current date and time.
    - `start_timer`
      - Provides a timer that can be used for measuring the duration between
        moments in time.
  - `string`
    - `from_bytes`, `replace`
- The following items are now imported by default into the top level of the
  prelude:
  - `io.print`, `koto.type`, `num2.make_num2`, `num4.make_num4`,
    `test.assert`, `test.assert_eq`, `test.assert_ne`, `test.assert_near`
- `iterator.consume` now accepts an optional function that will be called
  for each iterator output value.
  - e.g.
    ```koto
    (1, 2, 3).consume |n| print n
    # 1
    # 2
    # 3
    ```
- `test.assert_near`'s margin of error is now optional, defaulting to a very
  small value.

#### Internals

- The minumum supported rust version (MSRV) is now `1.58.1`.
- A 'module imported' callback has been added to `KotoSettings` to aid in
  keeping track of a script's module dependencies.
- `Koto::clear_module_cache()` has been added to allow for reloading scripts
  when one of the script's dependencies has changed.

### Changed

#### Language

- Assigning to a module's meta map has been reworked
  - The `export` keyword is no longer needed to assign to a meta key, as meta
    keys can never be assigned locally.
    - e.g.
      ```koto
      # Before
      export @tests =
        ...
      # After
      @tests =
        ...
      ```
  - `main` functions are now defined using the `@main` meta key.
    - This is so that modules don't have to pollute their public exported API to
      take advantage of having a main function.
    - e.g.
      ```koto
      # Before
      export main = ||
        ...
      # After
      @main = ||
        ...
      ```
- Map equality comparisons now don't rely on maps having keys in the same order.
  - e.g.
    ```koto
    x = {foo: 42, bar: 99}
    y = {bar: 99, foo: 42}
    # Before
    assert x != y
    # After
    assert x == y
    ```
- Functions that access a value that was exported prior to the function being
  created, will capture the value rather than access it from exports.
  - e.g.
    ```koto
    export x = 123
    f = || x
    # Re-exporting x doesn't affect the value of x captured when f was created
    export x = 99
    f() 
    # 123
    ```
- Arms in `match` and `switch` expressions that have indented blocks as their
  bodies need to use `then`.
  - This reverts a change made in `0.9.0`, in practice it's less distracting to
    have `then` required in all arms.
- Parsing of multi-line braced expressions is now more flexible.
  - e.g.
    ```koto
    # The following style of list declaration was previously disallowed
    x = [ 1
        , 2
        , 3
        ]
    ```
- Curly braces are now required when declaring a Map with inline syntax.
  - This reverts a change made in 0.9 which created too many ambiguous parsing 
    situations in practice.

#### Core Library

- The `num2` and `num4` keywords have been removed in favour of the new
  `make_num2`, `make_num4`, `iterator.to_num2`, and `iterator.to_num4`
  functions.
- The value provided to `list.resize` is now optional, with `null` being
  inserted when growing the list.
- `list.get`, `tuple.get`, and `map.get_index` will now return `null` when a
  negative number is used as the index, rather than throwing an error.
- List operations that modify the list but previously returned `null`,
  now return the modified list.
  - e.g.
    ```koto
    x = [1, 2, 3]

    # Before
    x.push 4
    # Null

    # After
    x.push 4
    # [1, 2, 3, 4]
    ```
- `range.contains` now supports descending ranges.
- `io.print` will now print a series of values as a Tuple, assuming the first value isn't a string.

#### Random Library

- The default generator functions can now be used directly.
  Previously they had to be used as instance functions.
  - e.g.
    ```koto
    # Before
    if random.bool() then do_x()
    # After
    rng_bool = import random.bool
    if rng_bool() then do_x()
    ```
- The `number2` and `number4` functions have been renamed to
  `num2` and `num4`.
- The number of rounds used by the generator (ChaCha) has been reduced from
  20 to 8.
- The random module is provided as a `ValueMap` rather than a `Value`,
  meaning that its now added to the prelude via `add_map` like other modules.

#### Internals

- Koto now uses the Rust 2021 edition.
- `Value::Empty` has been renamed to `Null`.
- `ExternalIterator` has been renamed to `KotoIterator`.
- `ValueIterator::make_external` has been renamed to `ValueIterator::new`.
- `Koto::set_script_path` and `set_args` now return `Result`s.
- `ValueMap`'s meta map is now optional, and use of the `meta()` getter will
  need to be adapted. Helpers (`get_meta_value`, `contains_meta_key`,
  `insert_meta`) have been introduced for convenience.
- The 'modulo' operator is now referred to more accurately as the 'remainder'
  operator.

### Removed

- The `num2` and `num4` keywords have been removed, see above.
- Support for setting `Num2`/`Num4` elements by index (added in `0.9.0`) has
  been removed. These value types should be treated as immutable; the `with`
  functions can be used to create new values with modified elements.
- `list.sort_copy` has been removed in favour of `.copy().sort()`.
  - e.g. 
    ```koto
    x = [3, 2, 1]
    y = x.copy().sort()
    # [1, 2, 3]
    x
    # [3, 2, 1]
    ```
- Support for nested multiline comments has been removed.
  - This makes it easier to toggle code blocks, e.g.
    ```koto
    #--
    # Adding a '#' to the start or end of the line above toggles the code below
    print 'hello'
    #--#
    ```
- The `+` operator is no longer implemented for Lists and Maps, 
  `list.extend` and `map.extend` can be used as an alternative.

### Fixed

- Error traces have been made more reliable, with the correct positions being
  displayed more consistently in calling functions.
- `io.print` now correctly prints values that are printed without a format
  string and that override @display.
- Fixed a panic that could occur when skipping past the end of an iterator and
  then calling a 'to X' function.
- Fixed unexpected shaky behaviour when compiling expressions that assign to the
  same name more than once in the expression, e.g. `x = x = 1`.
- Import expressions now work with previously-imported maps.
  - e.g.
    ```koto
    import foo.bar
    import bar.baz # <-- Previously this would cause a runtime error
    debug baz
    ```
- Accessing an ID without side effects would previously be optimized away,
  which led to the confusing situation where a missing ID could be accessed in a
  script without triggering an error.
- Running an integer remainder operation with a divisor of zero (e.g. `1 % 0`)
  no longer causes a panic and instead returns `NaN`.
- Added missing support for escaping `$` in strings.


## [0.10.0] 2021.12.02

### Added

- `iterator.cycle` has been added to the core library.
  - e.g.
    ```koto
    (1, 2, 3).cycle().take(10).to_list()
    # [1, 2, 3, 1, 2, 3, 1, 2, 3, 1]
    ```
- A pipe operator (`>>`) has been added to help with making long
  function call chains more readable.
  - e.g.
    ```koto
    x = then_that and_this 99, do_this 123
    # can now be written as:
    x = do_this 123 >> and_this 99 >> then_that
    # or with indentation:
    x = do_this 123
      >> and_this 99
      >> then_that
    ```
- Maps can now override the behaviour of the `not` operator using the `@not`
  meta key.
- `random.pick` now supports picking values from Tuples and Maps.

### Changed

- Linebreaks are now more flexible.
  - Linebreaks are allowed before assignment operators.
    - e.g.
      ```koto
      foo
        = 123 + 456
      # ^~~ Previously the indented `=` here would be disallowed.
      ```
  - Indentation can increase in arithmetic expressions.
    - e.g.
      ```koto
      x = 123
          + 456
            * 789
            # ^~~ Previously this indentation would have been disallowed.
          + 321
      ```
- Individual `.iter()` functions on containers have been replaced with
  `iterator.iter()`.
  - Existing scripts will continue to work without issue, unless they explicitly
    call or import one of the `iter()` functions from a module.
    `iterator.iter()` should be used instead.
- Internals
  - The Koto runtime is now single-threaded.
    - Reference counted value types are now wrapped in `Rc<...>`
      instead of `Arc<...>`.
    - External value meta maps that are instantiated using `lazy_static` may
      now use `thread_local!` instead.
  - The AST struct returned by the parser now includes its associated constant
    pool as a member.
  - Koto functions are now called from the outside with a `CallArgs` argument,
    which provides more information to the runtime about how the function
    should be called.
    - `CallArgs::AsTuple` will pass the arguments into the function as a tuple,
      using a non-allocating temporary tuple when possible (i.e. when the
      function immediately unpacks the tuple's values).

### Fixed

- Inline control flow expressions no longer incorrectly produce temporary
  results when the bodies are implicit tuples.
  - e.g.
    ```koto
    x = if foo then 1, 2, 3 else 4, 5, 6
    assert_eq x[0], 1 # Previously this would result in an error
    ```
- Functions passed as arguments can now be broken onto a new line.
  - e.g.
    ```koto
    foo
      bar,
      |x| x * x
    # ^~~~ Previously this would have returned a parsing error
    ```

### Removed

- The `thread` core library module has been removed.
- `os.cpu_count` and `os.physical_cpu_count` have been removed.

## [0.9.1] 2021.11.01

### Fixed

- Fixed a couple of REPL bugs, assigning from a negated value or while using
  multi-assignment didn't work correctly.

## [0.9.0] 2021.10.25

### Added

- Koto is now supported on Windows.
- String improvements
  - Support for string interpolation has been added.
    - e.g.
      ```koto
      x = 42

      "The answer is $x"
      # The answer is 42

      "$x divided by 3 is ${x / 3}."
      # 42 divided by 3 is 14.
      ```
  - Indexing a string with a range starting from 'one past the end' is now
    supported.
    - e.g. `"x"[1..]` is allowed, and produces an empty string.
- Iterator improvements
  - `iterator.min`, `iterator.max`, and `iterator.min_max` now all have
    overloads that accept a key function.
  - Added `iterator.copy`.
  - `deep_copy` operations will now make copies of contained iterators (instead
    of the resulting iterators having shared iterator positions).
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
  - Following a parenthesized num2/num4 expression with a lookup is now
    supported.
    - e.g.
      ```koto
      num2(1, 2).sum()
      #         ^-- Previously this would result in an 'unexpected token' error.
      ```
- Core library additions:
  - `io.extend_path`
  - `list.get`, `tuple.get`, `map.get`, and `map.get_index` now accept an
    optional default value that should be returned when an entry isn't found.
  - `num2.iter` / `num4.iter`
  - `num2.iter` / `num4.iter`
  - `num2.length` / `num4.length`
  - `num2.max` / `num4.max`
  - `num2.min` / `num4.min`
  - `num2.normalize` / `num4.normalize`
  - `num2.product` / `num4.product`
  - `os.name`
- `import` improvements
  - Strings can now be used in import expressions, which allows for more
    flexible module naming, and for dynamically importing items.
    - e.g.
      ```koto
      # Dynamically choosing a module path
      from "${module_path()}/my_module" import foo, bar
      # Loading a module with a name that isn't allowed as an identifier
      my_module = import "123"
      ```
- Added an optional library for working with YAML data.
- Throw and debug expressions can now be used more freely, in particular as
  expressions in match and switch arms.
  - e.g.
    ```koto
    match foo()
      0 then true
      1 then false
      x then debug x # debug would previously require an indented block here.
    ```
- Indented function calls are now allowed on lookups.
  - e.g. The following expression was previously disallowed:
    ```koto
    test.assert_eq
      1 + 1,
    # ^~~~ An 'unexpected token' error would previously be generated here
      2
    ```
- CLI
  - An `import_tests` flag has been added that causes a module's tests to be run
    when it's first imported.
- Internals
  - `From` implementations are extended to cover integer and floating point
    number types for `Value`. Also additional `From` implementations for `u16`
    and `i16` are added for both `Value` and `ValueNumber`.
    - e.g.
      ```rust
      let mut number: Value = 42_u16.into();
      number = -42_i16.into();
      ```
  - The Koto struct now has a `Koto::exports()` getter that allows access to a
    script's exported values.
  - A `run_import_tests` setting has been added to the runtime which will cause
    a module's tests to be run when it's imported.

### Changed

- Functions can now be called with missing arguments, with any missing arguments
  set to Empty.
  - e.g.
    ```koto
    foo = |a, b|
      a = if a == () then 100 else a
      b = if b == () then 42 else b
      a + b

    foo()    # 142
    foo 1    # 43
    foo 1, 2 # 3
    ```
- Curly braces are now optional when defining maps using inline syntax.
  - e.g.
    ```koto
    # The following map definition:
    x = {foo: 42, bar: -1}
    # ...can also now be written as:
    x = foo: 42, bar: -1
    ```
  - Curly braces are still useful when creating empty maps, or when using the
    'valueless map entry' feature, e.g.
    ```koto
    # Empty map
    x = {}
    # Valueless map entry
    foo = 42
    x = {foo, bar: -1}
    assert_eq x.foo, 42
    ```
- `$` symbols in string literals now need to be escaped due to the addition of
  string interpolation.
- `then` is no longer allowed in match and switch expression arms that have
  indented bodies. `then` is only to be used for inline arms, similar to inline
  `if` expressions.
- Internals
  - Compilation errors from the top-level Koto struct are now returned as a
    variant of `KotoError`.
  - External iterators must now implement the `ExternalIterator` trait.

### Removed

- Range expansion when making a list is no longer supported due to a reworking
  of list / tuple building. Iterators and `.to_list()` can be used as an
  alternative.
  - e.g.
    ```koto
    # Instead of:
    x = [1..=5] # [1, 2, 3, 4, 5]
    # Use .to_list():
    x = (1..=5).to_list() # [1, 2, 3, 4, 5]
    ```
- The `child_vm` mechanism has been removed.
  - External functions that make use of it should be able to switch to reusing
    the vm passed into the external function.

### Fixed

- Strings that end with an escaped backslash are now parsed correctly.

## [0.8.1] 2021.08.18

### Fixed

- Fixed a regression introduced in `v0.7.0` that prevented Maps from using a
  quoted string for the first entry's key while using block syntax.

## [0.8.0] 2021.08.17

### Added

- CLI improvements
  - The REPL now contains a help system that provides reference documentation
    for the core library.
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
  - The value to match against is now optional, and when it's omitted then
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
- Num2 and num4 values can now be used in unpacking expressions.
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
