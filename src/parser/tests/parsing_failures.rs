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
        fn indented_main_block() {
            let source = "
  1 + 1
";
            check_parsing_fails(source);
        }

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
}
