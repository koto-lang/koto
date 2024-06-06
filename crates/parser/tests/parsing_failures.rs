mod parser {
    use koto_parser::Parser;

    #[cfg(feature = "panic_on_parser_error")]
    fn check_parsing_fails(source: &str) {
        let catch_result = std::panic::catch_unwind(|| Parser::parse(source));

        if let Ok(Ok(ast)) = catch_result {
            panic!(
                "Unexpected success while parsing:\n{source}\n{:#?}",
                ast.nodes()
            );
        }
    }

    #[cfg(not(feature = "panic_on_parser_error"))]
    fn check_parsing_fails(source: &str) {
        if let Ok(ast) = Parser::parse(source) {
            panic!(
                "Unexpected success while parsing:\n{source}\n{:#?}",
                ast.nodes()
            );
        }
    }

    mod should_fail {
        use super::*;

        #[test]
        fn wildcard_as_map_id() {
            check_parsing_fails("{_}");
        }

        #[test]
        fn missing_term_in_arithmetic() {
            check_parsing_fails("1 + * 2");
        }

        #[test]
        fn missing_comma_in_import() {
            check_parsing_fails("import foo bar");
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
            fn varargs_on_wildcard() {
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
        }

        mod lists {
            use super::*;

            #[test]
            fn double_comma() {
                let source = "x = [1, 2, , 3]";

                check_parsing_fails(source);
            }

            #[test]
            fn space_separated_function_call_in_list() {
                let source = "x = [1, 2, f y, 4]";

                check_parsing_fails(source);
            }
        }

        mod tuples {
            use super::*;

            #[test]
            fn double_comma() {
                let source = "x = (1, 2, , 3)";

                check_parsing_fails(source);
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
            fn missing_template_identifier() {
                check_parsing_fails("'hello, $");
            }

            #[test]
            fn unterminated_template_expression() {
                check_parsing_fails("'hello, ${name'");
            }

            #[test]
            fn incomplete_template_expression() {
                check_parsing_fails("'${1 + }'");
            }

            #[test]
            fn multiline_template_expression() {
                let source = "
'foo: ${
42
}'
";
                check_parsing_fails(source);
            }
        }

        mod import {
            use super::*;

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
