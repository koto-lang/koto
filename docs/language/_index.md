# The Koto Language Guide

This guide contains a tour of the Koto language, providing a detailed overview 
of its features.

The guide is structured linearly, with each section building upon the last.
If you're new to the language then it's recommended to read through the sections 
in order.

## Reading Order

- [Getting Started](getting_started.md)
- [Language Basics](basics.md)
- [Strings](strings.md)
- [Functions](functions.md)
- [Lists](lists.md)
- [Tuples](tuples.md)
- [Maps](maps.md)
- [Core Library](core_library.md)
- [Conditional Expressions](conditional_expressions.md)
- [Loops](loops.md)
- [Iterators](iterators.md)
- [Ranges](ranges.md)
- [Value Unpacking](value_unpacking.md)
- [Advanced Functions](functions_advanced.md)
- [Generators](generators.md)
- [Meta Maps](meta_maps.md)
- [Errors](errors.md)
- [Testing](testing.md)
- [Modules](modules.md)

## About the Guide

The markdown files here serve as the source material for hte
[Koto website's language guide](https://koto.dev/docs/next/language),
and are also included in the `help` feature of the CLI. 

The code examples make use of `print!` and `check!` placeholders which are
used by tools to validate that the examples work correctly
(see the ['docs examples' tests](/core/koto/tests/docs_examples.rs)).
The placeholders are then stripped out by the CLI's preprocessor.
