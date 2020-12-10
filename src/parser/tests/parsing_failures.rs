mod parser {
    use koto_parser::Parser;

    #[cfg(feature = "panic_on_parser_error")]
    fn check_parsing_fails(source: &str) {
        let catch_result = std::panic::catch_unwind(|| Parser::parse(source));

        if let Ok(result) = catch_result {
            if let Ok(ast) = result {
                panic!(
                    "Unexpected success while parsing:\n{}\n{:#?}",
                    source,
                    ast.0.nodes()
                );
            }
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
        }

        mod lookups {
            use super::*;

            #[test]
            fn detached_index() {
                let source = "
x.foo
  [0]
";
                check_parsing_fails(source);
            }

            #[test]
            fn detached_dot_access() {
                let source = "
x. foo
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
            fn else_used_with_condition() {
                let source = "
match x
  0 then 1
  if true else 2
";
                check_parsing_fails(source);
            }
        }
    }
}
