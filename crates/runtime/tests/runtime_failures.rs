mod runtime {
    use koto_bytecode::{CompilerSettings, Loader};
    use koto_lexer::Span;
    use koto_runtime::{ErrorFrame, KotoVm};
    use koto_test_utils::script_instructions;

    fn check_script_fails(script: &str) {
        check_that_script_fails(script, None);
    }

    fn check_script_fails_with_span(script: &str, span: Span) {
        check_that_script_fails(script, Some(span))
    }

    fn check_that_script_fails(script: &str, span: Option<Span>) {
        let mut vm = KotoVm::default();

        let mut loader = Loader::default();
        let chunk = match loader.compile_script(script, None, CompilerSettings::default()) {
            Ok(chunk) => chunk,
            Err(error) => {
                println!("{}", script_instructions(script, vm.chunk()));
                panic!("Error while compiling script: {error}");
            }
        };

        match vm.run(chunk) {
            Ok(result) => {
                println!("{}", script_instructions(script, vm.chunk()));
                panic!(
                    "Script didn't fail as expected, result: {}",
                    vm.value_to_string(&result).unwrap()
                )
            }
            Err(e) => {
                if let Some(expected_span) = span {
                    let ErrorFrame { chunk, instruction } = e.trace.first().unwrap();
                    let error_span = chunk.debug_info.get_source_span(*instruction).unwrap();
                    if error_span != expected_span {
                        println!("{}", script_instructions(script, vm.chunk()));
                        assert_eq!(expected_span, error_span);
                    }
                }
            }
        }
    }

    mod should_fail {
        use super::*;

        mod assertions {
            use super::*;

            #[test]
            fn check_assert() {
                check_script_fails("assert false");
            }

            #[test]
            fn check_assert_eq() {
                check_script_fails("assert_eq 0, 1");
            }

            #[test]
            fn check_assert_ne() {
                check_script_fails("assert_ne 1, 1");
            }

            #[test]
            fn check_assert_near() {
                check_script_fails("assert_near 1, 2, 0.1");
            }
        }

        mod type_checks {
            use koto_lexer::Position;

            use super::*;

            #[test]
            fn expected_string() {
                let script = "\
let foo: String = 123
#   ^^^
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position { line: 0, column: 4 },
                        end: Position { line: 0, column: 7 },
                    },
                );
            }

            #[test]
            fn expected_bool_in_multi_assignment() {
                let script = "\
let x: String, y: Bool = 'abc', 123
#              ^
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position {
                            line: 0,
                            column: 15,
                        },
                        end: Position {
                            line: 0,
                            column: 16,
                        },
                    },
                );
            }

            #[test]
            fn expected_indexable() {
                let script = "\
let foo: Indexable = null
#   ^^^
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position { line: 0, column: 4 },
                        end: Position { line: 0, column: 7 },
                    },
                );
            }

            #[test]
            fn expected_iterable() {
                let script = "\
let foo: Iterable = true
#   ^^^
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position { line: 0, column: 4 },
                        end: Position { line: 0, column: 7 },
                    },
                );
            }

            #[test]
            fn wildcard_expected_bool() {
                let script = "\
let _foo: Bool = 'abc'
#   ^^^^
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position { line: 0, column: 4 },
                        end: Position { line: 0, column: 8 },
                    },
                );
            }

            #[test]
            fn wildcard_expected_string_in_multi_assignment() {
                let script = "\
let _x: String, y: Bool = 99, true
#   ^^
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position { line: 0, column: 4 },
                        end: Position { line: 0, column: 6 },
                    },
                );
            }

            #[test]
            fn function_arg_with_type() {
                let script = "\
f = |x: Number| x
#    ^
f 'hello'
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position { line: 0, column: 5 },
                        end: Position { line: 0, column: 6 },
                    },
                );
            }

            #[test]
            fn nested_arg_with_type() {
                let script = "\
f = |(foo: Number)| foo
#     ^^^
f ('hello',)
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position { line: 0, column: 6 },
                        end: Position { line: 0, column: 9 },
                    },
                );
            }

            #[test]
            fn wildcard_arg_with_type() {
                let script = "\
f = |_x: List| true
#    ^^
f 'hello'
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position { line: 0, column: 5 },
                        end: Position { line: 0, column: 7 },
                    },
                );
            }

            #[test]
            fn nested_wildcard_arg_with_type() {
                let script = "\
f = |(_x: Bool)| true
#     ^^
f ('hello',)
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position { line: 0, column: 6 },
                        end: Position { line: 0, column: 8 },
                    },
                );
            }

            #[test]
            fn function_with_output_type_and_implicit_return() {
                let script = "\
f = |x: Number| -> String
  x + x
# ^^^^^
f 42
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position { line: 1, column: 2 },
                        end: Position { line: 1, column: 7 },
                    },
                );
            }

            #[test]
            fn function_with_output_type_and_explicit_return() {
                let script = "\
f = |x: Number| -> String
  return x + x
# ^^^^^^^^^^^^
f 42
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position { line: 1, column: 2 },
                        end: Position {
                            line: 1,
                            column: 14,
                        },
                    },
                );
            }

            #[test]
            fn for_loop_with_typed_arg() {
                let script = "\
for foo: Number in (1, true, 2)
#   ^^^
  foo
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position { line: 0, column: 4 },
                        end: Position { line: 0, column: 7 },
                    },
                );
            }

            #[test]
            fn for_loop_with_typed_wildcard_arg() {
                let script = "\
for _foo: Number in (1, true, 2)
#   ^^^^
  null
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position { line: 0, column: 4 },
                        end: Position { line: 0, column: 8 },
                    },
                );
            }

            #[test]
            fn for_loop_with_typed_unpacked_arg() {
                let script = "\
for i: Number, x: Bool in 'abc'.enumerate()
#              ^
  null
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position {
                            line: 0,
                            column: 15,
                        },
                        end: Position {
                            line: 0,
                            column: 16,
                        },
                    },
                );
            }

            #[test]
            fn for_loop_with_typed_unpacked_wildcard_arg() {
                let script = "\
for i: Number, _x: Bool in 'abc'.enumerate()
#              ^^
  null
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position {
                            line: 0,
                            column: 15,
                        },
                        end: Position {
                            line: 0,
                            column: 17,
                        },
                    },
                );
            }

            #[test]
            fn generator_with_type_hint() {
                let script = "\
g = || -> Number
  yield 1
  yield 'abc'
# ^^^^^^^^^^^
  42
g().consume()
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position { line: 2, column: 2 },
                        end: Position {
                            line: 2,
                            column: 13,
                        },
                    },
                );
            }
        }

        mod missing_values {
            use super::*;

            #[test]
            fn missing_identifier_before_last_expression() {
                let script = "
x = 123
y # y hasn't been declared, so should throw an error on access
x
";
                check_script_fails(script);
            }
        }

        mod iterators {
            use super::*;

            #[test]
            fn iterator_consume_should_propagate_error() {
                let script = "
(1..5)
  .each |_| assert false
  .consume()
";
                check_script_fails(script);
            }

            #[test]
            fn iterator_count_should_propagate_error() {
                let script = "
(1..5)
  .each |_| assert false
  .count()
";
                check_script_fails(script);
            }

            #[test]
            fn unbounded_range_used_as_iterator() {
                let script = "
(1..).count()
";
                check_script_fails(script);
            }

            #[test]
            fn unbounded_range_used_in_for_loop() {
                let script = "
for i in 0..
  print i
";
                check_script_fails(script);
            }
        }

        mod function_calls {
            use super::*;

            #[test]
            fn tuple_unpacking_of_non_tuple() {
                let script = r#"
f = |(a, b)| a + b
f "O_o"
"#;
                check_script_fails(script);
            }

            #[test]
            fn tuple_unpacking_of_tuple_with_wrong_size() {
                let script = r#"
f = |(a, b)| a + b
f (1, 2, 3)
"#;
                check_script_fails(script);
            }

            #[test]
            fn capturing_a_reserved_value_in_a_temporary_function() {
                let script = "
x = (1..10).find |n| n == x
";
                check_script_fails(script);
            }
        }

        mod indexing {
            use super::*;

            #[test]
            fn index_out_of_bounds() {
                let script = "
x = [0, 1, 2]
x[3] = 3
";
                check_script_fails(script);
            }
        }

        mod maps {
            use super::*;

            #[test]
            fn list_as_key() {
                let script = "
x = {}
x.insert [1, 2], 'hello'
";
                check_script_fails(script);
            }

            #[test]
            fn map_as_key() {
                let script = "
x = {}
x.insert {foo: 42}, 'hello'
";
                check_script_fails(script);
            }

            #[test]
            fn tuple_as_key_with_contained_list() {
                let script = "
x = {}
x.insert (1, [2, 3]), 'hello'
";
                check_script_fails(script);
            }
        }

        mod meta_maps {
            use super::*;

            #[test]
            fn next_with_generator() {
                let script = "
x =
  @next: || yield 42
a, b = x
";
                check_script_fails(script);
            }

            #[test]
            fn next_with_non_function() {
                let script = "
x =
  @next: 42
a, b = x
";
                check_script_fails(script);
            }

            #[test]
            fn next_back_without_next() {
                let script = "
x =
  @next_back: || 42
x.reversed().next()
";
                check_script_fails(script);
            }
        }

        mod strings {
            use super::*;

            #[test]
            fn missing_interpolated_id() {
                let script = "
x = '{foo}'
";
                check_script_fails(script);
            }

            #[test]
            fn invalid_raw_string_delimiter() {
                // 256 #s in the delimiter is over the limit
                let script = "
x = r################################################################################################################################################################################################################################################################'foo'################################################################################################################################################################################################################################################################
";
                check_script_fails(script);
            }
        }
    }
}
