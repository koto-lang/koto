mod parser {
    use koto_parser::Parser;

    #[cfg(feature = "panic_on_parser_error")]
    fn check_parsing_fails(source: &str) {
        let catch_result = std::panic::catch_unwind(|| Parser::parse(source));

        if let Ok(Ok(ast)) = catch_result {
            panic!(
                "Unexpected success while parsing:\n{}\n{:#?}",
                source,
                ast.0.nodes()
            );
        }
    }

    #[cfg(not(feature = "panic_on_parser_error"))]
    fn check_parsing_fails(source: &str) {
        if let Ok(ast) = Parser::parse(source) {
            panic!(
                "Unexpected success while parsing:\n{}\n{:#?}",
                source,
                ast.0.nodes()
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
            fn extra_indentation_in_arithmetic() {
                let source = "
x = 1
    + 2
      + 3
";
                check_parsing_fails(source);
            }

            #[test]
            fn list_end_with_incorrect_indentation() {
                let source = "
x = [
  1,
  2,
    ]
";
                check_parsing_fails(source);
            }

            #[test]
            fn map_end_with_incorrect_indentation() {
                let source = "
x = {
  foo: 42,
  bar: 99
    }
";
                check_parsing_fails(source);
            }

            #[test]
            fn function_end_with_incorrect_indentation() {
                let source = "
x = |
  x
  y
    | x + y
";
                check_parsing_fails(source);
            }

            #[test]
            fn else_at_same_indentation_as_if_body() {
                let source = "
if f x
  0
  else
    1
";
                check_parsing_fails(source);
            }

            #[test]
            fn else_if_at_same_indentation_as_if_body() {
                let source = "
if f x
  0
  else if g x
    1
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
            fn self_not_in_first_position() {
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
            fn missing_terminator_for_tuple_arg() {
                check_parsing_fails("f = |a, (b, c, d| a");
            }

            #[test]
            fn missing_terminator_for_list_arg() {
                check_parsing_fails("f = |a, [b, c, d| a");
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
            fn missing_commas_in_lookup_call() {
                check_parsing_fails("f.bar 1 2 3");
            }
        }

        mod lookups {
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

        mod maps {
            use super::*;

            #[test]
            fn block_without_indentation() {
                let source = "
foo: 42
bar: 99
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
            fn block_without_indentation_in_function_after_first_line() {
                let source = "
f = ||
  a = 1
  foo: 42
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
            fn pattern_used_with_no_match_value() {
                let source = "
match
  0 if true then 1
  else 2
";
                check_parsing_fails(source);
            }
        }
    }
}
