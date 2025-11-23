mod parser {
    use koto_parser::{Ast, ErrorKind, Parser, Position, Result, Span, SyntaxError};

    fn check_parsing_result(
        result: Result<Ast>,
        source: &str,
        error_kind: Option<ErrorKind>,
        span: Option<Span>,
    ) {
        match result {
            Ok(ast) => {
                panic!(
                    "Unexpected success while parsing:\n{source}\n{:#?}",
                    ast.nodes()
                );
            }
            Err(error) => {
                if let Some(expected_error_kind) = error_kind {
                    assert_eq!(expected_error_kind.to_string(), error.error.to_string());
                }

                if let Some(expected_span) = span {
                    assert_eq!(expected_span, error.span);
                }
            }
        }
    }

    fn check_that_parsing_fails_with_error_span(
        source: &str,
        error: Option<ErrorKind>,
        span: Option<Span>,
    ) {
        #[cfg(feature = "panic_on_parser_error")]
        check_parsing_result(
            std::panic::catch_unwind(|| Parser::parse(source)),
            source,
            error,
            span,
        );

        #[cfg(not(feature = "panic_on_parser_error"))]
        check_parsing_result(Parser::parse(source), source, error, span);
    }

    fn check_parsing_fails(source: &str) {
        check_that_parsing_fails_with_error_span(source, None, None);
    }

    fn check_parsing_fails_with_span(source: &str, span: Span) {
        check_that_parsing_fails_with_error_span(source, None, Some(span))
    }

    fn check_parsing_fails_with_error_span(source: &str, error: impl Into<ErrorKind>, span: Span) {
        check_that_parsing_fails_with_error_span(source, Some(error.into()), Some(span))
    }

    mod should_fail {
        use super::*;

        mod arithmetic {
            use super::*;

            #[test]
            fn missing_term_in_arithmetic() {
                check_parsing_fails("1 + * 2");
            }
        }

        mod assignment {
            use super::*;

            #[test]
            fn missing_assignment_rhs() {
                let source = "\
x =
# ^
";
                check_parsing_fails_with_span(
                    source,
                    Span {
                        start: Position { line: 0, column: 2 },
                        end: Position { line: 0, column: 3 },
                    },
                )
            }

            #[test]
            fn expected_map_rebind_assignment_target() {
                let source = "\
{ x as [] } = { x: [] }
#      ^
";
                check_parsing_fails_with_error_span(
                    source,
                    SyntaxError::ExpectedUnpackedMapKeyRebindId,
                    Span {
                        start: Position { line: 0, column: 7 },
                        end: Position { line: 0, column: 8 },
                    },
                );
            }

            #[test]
            fn unexpected_map_assignment_value() {
                let source = "\
{ x: 'foo' } = { x: 'foo' }
#    ^^^^^
";
                check_parsing_fails_with_error_span(
                    source,
                    SyntaxError::UnexpectedMapKeyTypeHint,
                    Span {
                        start: Position { line: 0, column: 5 },
                        end: Position {
                            line: 0,
                            column: 10,
                        },
                    },
                );
            }

            #[test]
            fn unexpected_map_assignment_value_with_meta_key() {
                let source = "\
{ @type: 'Foo' } = { @type: 'Foo' }
#        ^^^^^
";
                check_parsing_fails_with_error_span(
                    source,
                    SyntaxError::UnexpectedMapKeyTypeHint,
                    Span {
                        start: Position { line: 0, column: 9 },
                        end: Position {
                            line: 0,
                            column: 14,
                        },
                    },
                );
            }

            #[test]
            fn expected_type_for_map_assignment() {
                let source = "\
let { x: 'foo' } = { x: 'foo' }
#        ^
";
                check_parsing_fails_with_error_span(
                    source,
                    SyntaxError::ExpectedType,
                    Span {
                        start: Position { line: 0, column: 9 },
                        end: Position {
                            line: 0,
                            column: 10,
                        },
                    },
                );
            }
        }

        mod indentation {
            use super::*;

            #[test]
            fn indented_main_block() {
                let source = "
  1 + 1
";
                check_parsing_fails(source);
            }

            #[test]
            fn decreased_indentation_in_arithmetic() {
                let source = "
x =
  1 + 2
+ 3
";
                check_parsing_fails(source);
            }

            #[test]
            fn else_at_greater_indentation_than_else_if() {
                let source = "
z = if f x
        0
    else if g x
        1
      else
        2
";
                check_parsing_fails(source);
            }
        }

        mod semicolons {
            use super::*;

            #[test]
            fn without_expression() {
                let source = "
;
";
                check_parsing_fails(source);
            }

            #[test]
            fn in_map_block() {
                let source = "
foo = 
  bar: x = 1; x
";
                check_parsing_fails(source);
            }

            #[test]
            fn in_for_condition() {
                let source = "
for x in y = 1; y
  x
";
                check_parsing_fails(source);
            }
        }

        mod loops {
            use super::*;

            #[test]
            fn if_following_for() {
                let source = "
for x in y if f x
  debug x
";
                check_parsing_fails(source);
            }
        }

        mod functions {
            use super::*;

            #[test]
            fn self_as_first_arg() {
                check_parsing_fails("f = |self, x| x");
            }

            #[test]
            fn self_as_last_arg() {
                check_parsing_fails("f = |x, self| x");
            }

            #[test]
            fn varargs_not_in_last_position() {
                check_parsing_fails("f = |x..., y| x");
            }

            #[test]
            fn varargs_on_ignored_arg() {
                check_parsing_fails("f = |x, _...| x");
            }

            #[test]
            fn missing_terminator_for_unpacked_arg() {
                check_parsing_fails("f = |a, (b, c, d| a");
            }

            #[test]
            fn square_brackets_used_for_unpacked_arg() {
                check_parsing_fails("f = |a, [b, c]| a");
            }

            #[test]
            fn missing_default_value() {
                check_parsing_fails("f = |a = 42, b| a");
            }

            #[test]
            fn missing_commas_in_call() {
                check_parsing_fails("f 1 2 3");
            }

            #[test]
            fn missing_commas_in_call_in_indented_block() {
                let source = "
f = ||
  f 1 2 3
";
                check_parsing_fails(source);
            }

            #[test]
            fn missing_commas_in_chained_call() {
                check_parsing_fails("f.bar 1 2 3");
            }

            #[test]
            fn unexpected_token_as_body() {
                let source = "\
f = || ?
#      ^
";
                check_parsing_fails_with_span(
                    source,
                    Span {
                        start: Position { line: 0, column: 7 },
                        end: Position { line: 0, column: 8 },
                    },
                )
            }
        }

        mod chains {
            use super::*;

            #[test]
            fn detached_dot_access() {
                let source = "
x. foo
";
                check_parsing_fails(source);
            }

            #[test]
            fn detached_dot_access_2() {
                let source = "
x .foo
";
                check_parsing_fails(source);
            }

            #[test]
            fn detached_null_check() {
                let source = "
x.foo ?.bar
";
                check_parsing_fails(source);
            }

            #[test]
            fn null_check_at_root() {
                let source = "
?x.foo
";
                check_parsing_fails(source);
            }

            #[test]
            fn double_null_check() {
                let source = "
x??.foo
";
                check_parsing_fails(source);
            }
        }

        mod piped_calls {
            use super::*;

            #[test]
            fn pipe_without_indentation_in_function() {
                let source = "
x = ||
  foo 42
  -> bar
";
                check_parsing_fails(source);
            }
        }

        mod maps {
            use super::*;

            #[test]
            fn ignored_as_map_id() {
                check_parsing_fails("{_}");
            }

            #[test]
            fn block_starting_on_same_line_as_assignment_single_entry() {
                let source = "
x = foo: 42
";
                check_parsing_fails(source);
            }

            #[test]
            fn block_starting_on_same_line_as_assignment() {
                let source = "
x = foo: 42
    bar: 99
";
                check_parsing_fails(source);
            }

            #[test]
            fn block_key_without_value() {
                let source = "
x =
  foo: 42
  bar
  baz: -1
";
                check_parsing_fails(source);
            }

            #[test]
            fn inline_map_without_braces() {
                let source = "
x = foo: 42, bar: 99,
  baz: 99
";
                check_parsing_fails(source);
            }

            #[test]
            fn string_used_as_valueless_key() {
                let source = "
x = {'y'}
";
                check_parsing_fails(source);
            }

            #[test]
            fn unexpected_token_inside_braces() {
                let source = "\
x = {foo: 42, ?}
#             ^
";
                check_parsing_fails_with_span(
                    source,
                    Span {
                        start: Position {
                            line: 0,
                            column: 14,
                        },
                        end: Position {
                            line: 0,
                            column: 15,
                        },
                    },
                );
            }

            #[test]
            fn map_pattern_key_rebind_on_rhs() {
                let source = "\
{x as y}
#  ^^
";
                check_parsing_fails_with_error_span(
                    source,
                    SyntaxError::UnexpectedMapKeyRebindOnRhs,
                    Span {
                        start: Position { line: 0, column: 3 },
                        end: Position { line: 0, column: 5 },
                    },
                );
            }
        }

        mod lists {
            use super::*;

            #[test]
            fn unexpected_token_inside_list() {
                let source = "\
x = [1, 2, ?]
#          ^
";
                check_parsing_fails_with_span(
                    source,
                    Span {
                        start: Position {
                            line: 0,
                            column: 11,
                        },
                        end: Position {
                            line: 0,
                            column: 12,
                        },
                    },
                );
            }
        }

        mod tuples {
            use super::*;

            #[test]
            fn unexpected_token_inside_tuple() {
                let source = "\
x = (
  1,
  2,
  ?
)
# ^
";
                check_parsing_fails_with_span(
                    source,
                    Span {
                        start: Position { line: 3, column: 2 },
                        end: Position { line: 3, column: 3 },
                    },
                );
            }
        }

        mod match_expressions {
            use super::*;

            #[test]
            fn else_used_with_pattern() {
                let source = "
match x
  0 then 1
  1 else 2
";
                check_parsing_fails(source);
            }

            #[test]
            fn else_not_in_last_arm() {
                let source = "
match x
  else 2
  0 then 1
";
                check_parsing_fails(source);
            }

            #[test]
            fn else_used_with_condition() {
                let source = "
match x
  0 then 1
  if true else 2
";
                check_parsing_fails(source);
            }

            #[test]
            fn indented_block_missing_then() {
                let source = "
match x
  0
    1
";
                check_parsing_fails(source);
            }

            #[test]
            fn pattern_used_with_no_match_value() {
                let source = "
match
  0 if true then 1
  else 2
";
                check_parsing_fails(source);
            }

            #[test]
            fn square_brackets_used_for_unpacking() {
                let source = "
match [1, 2, 3]
  [x, y, z] then x + y + z
  else 2
";
                check_parsing_fails(source);
            }

            #[test]
            fn map_pattern_key() {
                let source = "\
match { @type: 'Foo' }
  { @type: 'Foo' } then 'ok'
#   ^
";
                check_parsing_fails_with_error_span(
                    source,
                    SyntaxError::ExpectedMapPatKey,
                    Span {
                        start: Position { line: 1, column: 4 },
                        end: Position { line: 1, column: 5 },
                    },
                );
            }

            #[test]
            fn map_pattern_string_key_without_as() {
                let source = "\
match { x: 'y' }
  { 'x' } then 'ok'
#   ^^^
";
                check_parsing_fails_with_error_span(
                    source,
                    SyntaxError::ExpectedMapPatAsAfterString,
                    Span {
                        start: Position { line: 1, column: 4 },
                        end: Position { line: 1, column: 7 },
                    },
                );
            }

            #[test]
            fn map_pattern_non_pattern_after_as() {
                let source = "\
match { x: 'y' }
  { 'x' as [] } then 'ok'
#          ^
";
                check_parsing_fails_with_error_span(
                    source,
                    SyntaxError::ExpectedUnpackedMapKeyRebindId,
                    Span {
                        start: Position {
                            line: 1,
                            column: 11,
                        },
                        end: Position {
                            line: 1,
                            column: 12,
                        },
                    },
                );
            }
        }

        mod switch_expressions {
            use super::*;

            #[test]
            fn indented_block_missing_then() {
                let source = "
switch
  true
    1
";
                check_parsing_fails(source);
            }
        }

        mod strings {
            use super::*;

            #[test]
            fn unterminated_string() {
                check_parsing_fails("'hello");
            }

            #[test]
            fn incorrect_terminating_quote() {
                check_parsing_fails("'hello\"");
            }

            #[test]
            fn unterminated_template_expression() {
                check_parsing_fails("'hello, {name'");
            }

            #[test]
            fn incomplete_template_expression() {
                check_parsing_fails("'{1 + }'");
            }

            #[test]
            fn multiline_template_expression() {
                let source = "
'foo: {
42
}'
";
                check_parsing_fails(source);
            }
        }

        mod import {
            use super::*;

            #[test]
            fn missing_comma_in_import() {
                check_parsing_fails("import foo bar");
            }

            #[test]
            fn nested_import() {
                check_parsing_fails("import foo.bar");
            }

            #[test]
            fn multiple_from_items() {
                check_parsing_fails("from bar, baz import foo");
            }

            #[test]
            fn from_after_import() {
                check_parsing_fails("import foo from bar");
            }

            #[test]
            fn missing_id_after_as() {
                check_parsing_fails("from foo import bar as");
            }

            #[test]
            fn wildcard_without_from() {
                check_parsing_fails("import *");
            }

            #[test]
            fn wildcard_after_id() {
                check_parsing_fails("from foo import bar, *");
            }
        }

        mod reserved_keywords {
            use super::*;

            #[test]
            fn r#await() {
                check_parsing_fails("await = 99");
            }

            #[test]
            fn r#const() {
                check_parsing_fails("const = 99");
            }
        }
    }
}
