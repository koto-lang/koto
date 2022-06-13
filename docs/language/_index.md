# The Koto Language Guide

This guide contains a tour of the Koto language, giving an overview of its features.

The markdown files here are the source material for the help topics included in
the `help` feature of the CLI. 

The guide can be read from start to finish, with later sections building on
concepts introduced in earlier sections.

The code examples make use of `print!` and `check!` placeholders which are
used by tools to validate that the examples work correctly
(see the ['docs examples' tests](/src/koto/tests/docs_examples.rs)).
The placeholders are then stripped out by the CLI's preprocessor.

- [Comments](comments.md)
- [Basic Types](basic_types.md)
- [Value Assignments](value_assignments.md)
- [Strings](strings.md)
- [Functions](functions.md)
- [Lists](lists.md)
- [Tuples](tuples.md)
- [Value Unpacking](value_unpacking.md)
- [Maps](maps.md)
- [Core Library](core_library.md)
- [Iterators](iterators.md)
- [Conditional Expressions](conditional_expressions.md)
- [Loops](loops.md)
- [Ranges](ranges.md)
- [Generators](generators.md)
- [Packed Numbers](packed_numbers.md)
- [Errors](errors.md)
- [Meta Maps](meta_maps.md)
- [Testing](testing.md)
- [Modules](modules.md)
