//! A collection of Koto scripts that should throw an error at runtime

mod runtime {
    use koto_bytecode::{CompilerSettings, ModuleLoader};
    use koto_lexer::{Position, Span};
    use koto_runtime::{InstructionFrame, KotoVm};
    use koto_test_utils::script_instructions;

    fn check_script_fails(script: &str) {
        check_that_script_fails(script, None, None);
    }

    fn check_script_fails_with_span(script: &str, span: Span) {
        check_that_script_fails(script, None, Some(span))
    }

    fn check_script_fails_with_error_span(script: &str, error: impl Into<String>, span: Span) {
        check_that_script_fails(script, Some(error.into()), Some(span))
    }

    fn check_script_fails_with_error(script: &str, error: impl Into<String>) {
        check_that_script_fails(script, Some(error.into()), None)
    }

    fn check_that_script_fails(script: &str, message: Option<String>, span: Option<Span>) {
        let mut vm = KotoVm::default();

        let mut loader = ModuleLoader::default();
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
                if let Some(expected_message) = message {
                    assert_eq!(expected_message, e.error.to_string());
                }

                if let Some(expected_span) = span {
                    let InstructionFrame { chunk, instruction } = e.trace.first().unwrap();
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
            fn expected_callable() {
                let script = "\
let foo: Callable = 99
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
let foo: Iterable? = true
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
            fn ignored_id_expected_bool() {
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
            fn ignored_id_expected_string_in_multi_assignment() {
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
f = |(foo: Number?)| foo
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
            fn ignored_arg_with_type() {
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
            fn nested_ignored_arg_with_type() {
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
f = |x: Number| -> String?
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
            fn function_with_map_arg() {
                let script = "\
f = |{x, y}: Foo|
#    ^^^^^^
  x + y

f {x: 1, y: 2, @type: 'Bar'}
";
                check_script_fails_with_error_span(
                    script,
                    "expected Foo, found Bar",
                    Span {
                        start: Position { line: 0, column: 5 },
                        end: Position {
                            line: 0,
                            column: 11,
                        },
                    },
                );
            }

            #[test]
            fn function_with_map_arg_entry() {
                let script = "\
f = |{number: Number}|
#     ^^^^^^
  number

f {number: 'not a number'}
";
                check_script_fails_with_error_span(
                    script,
                    "expected Number, found String",
                    Span {
                        start: Position { line: 0, column: 6 },
                        end: Position {
                            line: 0,
                            column: 12,
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
            fn for_loop_with_typed_ignored_arg() {
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
            fn for_loop_with_typed_unpacked_ignored_arg() {
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

            #[test]
            fn let_map() {
                let script = "\
let {abc}: Foo = {@type: 'Bar', abc: 123}
#   ^^^^^
";
                check_script_fails_with_error_span(
                    script,
                    "expected Foo, found Bar",
                    Span {
                        start: Position { line: 0, column: 4 },
                        end: Position { line: 0, column: 9 },
                    },
                );
            }

            #[test]
            fn let_map_key() {
                let script = "\
let {abc: String} = {abc: 123}
#    ^^^
";
                check_script_fails_with_error_span(
                    script,
                    "expected String, found Number",
                    Span {
                        start: Position { line: 0, column: 5 },
                        end: Position { line: 0, column: 8 },
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

            #[test]
            fn missing_value_on_rhs() {
                let script = "\
1 + foo
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
        }

        mod iterators {
            use super::*;

            #[test]
            fn advance_should_propagate_error() {
                let script = "\
g = ||
  yield 1
  assert false
# ^^^^^^^^^^^^

g().advance 2
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position { line: 2, column: 2 },
                        end: Position {
                            line: 2,
                            column: 14,
                        },
                    },
                );
            }

            #[test]
            fn consume_should_propagate_error() {
                let script = "\
(1..5)
  .each |_| assert false
#           ^^^^^^^^^^^^
  .consume()
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position {
                            line: 1,
                            column: 12,
                        },
                        end: Position {
                            line: 1,
                            column: 24,
                        },
                    },
                );
            }

            #[test]
            fn count_should_propagate_error() {
                let script = "\
(1..5)
  .each |_| assert false
#           ^^^^^^^^^^^^
  .count()
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position {
                            line: 1,
                            column: 12,
                        },
                        end: Position {
                            line: 1,
                            column: 24,
                        },
                    },
                );
            }

            #[test]
            fn generate_used_as_instance_function() {
                let script = "
[].generate(|| true).take(3).to_list()
";
                check_script_fails(script);
            }

            #[test]
            fn keep_function_missing_argument() {
                let script = "\
(1..10).keep(|| false).to_tuple()
#       ^^^^
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position { line: 0, column: 8 },
                        end: Position {
                            line: 0,
                            column: 12,
                        },
                    },
                );
            }

            #[test]
            fn keep_function_returns_non_bool() {
                let script = "\
(1..10).keep(|x| null).to_tuple()
#       ^^^^
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position { line: 0, column: 8 },
                        end: Position {
                            line: 0,
                            column: 12,
                        },
                    },
                );
            }

            #[test]
            fn repeat_used_as_instance_function() {
                let script = "
[1, 2, 3].repeat(3).to_list()
";
                check_script_fails(script);
            }

            #[test]
            fn string_split_function_missing_argument() {
                let script = "\
'abc'.split(|| false).to_tuple()
#     ^^^^^
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position { line: 0, column: 6 },
                        end: Position {
                            line: 0,
                            column: 11,
                        },
                    },
                );
            }

            #[test]
            fn string_split_function_returns_non_bool() {
                let script = "\
'abc'.split(|c| null).to_tuple()
#     ^^^^^
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position { line: 0, column: 6 },
                        end: Position {
                            line: 0,
                            column: 11,
                        },
                    },
                );
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
            fn insufficient_arguments_for_call() {
                let script = r#"
f = |a, b, c| a + b + c
f 1, 2
"#;
                check_script_fails(script);
            }

            #[test]
            fn insufficient_arguments_with_default_args() {
                let script = r#"
f = |a, b = -1, c = -2| a + b + c
f()
"#;
                check_script_fails(script);
            }

            #[test]
            fn insufficient_arguments_for_generator() {
                let script = r#"
f = |a, b, c| yield a + b + c
f 1, 2
"#;
                check_script_fails(script);
            }

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
# The functor is temporary and it shouldn't be possible to capture
# the identifier that's reserved as the assignment target.
x = (1..10).find |n| n == x
";
                check_script_fails(script);
            }

            #[test]
            fn too_many_arguments_after_unpacking() {
                let script = r#"
f = |a, b| a + b
x = 1, 2, 3 
f x...
"#;
                check_script_fails(script);
            }

            #[test]
            fn too_many_arguments_during_unpacking() {
                let script = r#"
f = |args...| args
x = 1..1000 
f x...
"#;
                check_script_fails(script);
            }
        }

        mod indexing {
            use super::*;

            #[test]
            fn index_out_of_bounds_list() {
                let script = "
x = [0, 1, 2]
x[3] = 3
";
                check_script_fails(script);
            }

            #[test]
            fn index_out_of_bounds_range() {
                let script = "
x = 10..20
x[100]
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

            #[test]
            fn missing_entry() {
                let script = "\
foo = {}
foo.missing_entry()
#   ^^^^^^^^^^^^^
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position { line: 1, column: 4 },
                        end: Position {
                            line: 1,
                            column: 17,
                        },
                    },
                )
            }

            #[test]
            fn missing_entry_on_new_line() {
                let script = "\
foo = {bar: {}}
foo
  .bar
  .missing_entry()
#  ^^^^^^^^^^^^^
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position { line: 3, column: 3 },
                        end: Position {
                            line: 3,
                            column: 16,
                        },
                    },
                )
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

        mod export {
            use super::*;

            #[test]
            fn export_single_value() {
                let script = "
export 99
";
                check_script_fails(script);
            }

            #[test]
            fn export_list() {
                let script = "
x = [1, 2, 3]
export x
";
                check_script_fails(script);
            }

            #[test]
            fn export_iterator_with_non_key_pair_output() {
                let script = "
export (1..=3).each |i| i, i, i
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

            #[test]
            fn invalid_string_op_on_new_line() {
                // Check that the span is correct for invalid string ops

                let script = "\
'testing'
  .xxxx()
#  ^^^^
";
                check_script_fails_with_span(
                    script,
                    Span {
                        start: Position { line: 1, column: 3 },
                        end: Position { line: 1, column: 7 },
                    },
                )
            }
        }

        mod import {
            use super::*;

            #[test]
            fn import_unknown_module() {
                let script = "
import abcxyz
";
                check_script_fails(script);
            }

            #[test]
            fn wildcard_import_after_function() {
                let script = "
f = |x| abs x
from number import *
f -1
";
                check_script_fails(script);
            }

            #[test]
            fn wildcard_import_after_nested_function() {
                let script = "
f = |x|
  g = |x| abs x
  from number import *
  g x

f -1
";
                check_script_fails(script);
            }
        }

        mod stdio {
            use super::*;

            #[test]
            fn unavailable_by_default() {
                // We're just calling `flush`, but whatever File method you call,
                // the error is the same.
                check_script_fails_with_error("io.stdin.flush()", "stdin is unavailable");
                check_script_fails_with_error("io.stdout.flush()", "stdout is unavailable");
                check_script_fails_with_error("io.stderr.flush()", "stderr is unavailable");

                // `print` uses stdout
                check_script_fails_with_error("print 'test'", "stdout is unavailable");
            }
        }
    }
}
