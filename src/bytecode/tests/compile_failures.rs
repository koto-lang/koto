mod bytecode {
    use {koto_bytecode::Compiler, koto_parser::Parser};

    fn check_compilation_fails(source: &str) {
        match Parser::parse(&source) {
            Ok((ast, _constants)) => {
                if let Ok(_) = Compiler::compile(&ast, koto_bytecode::Settings::default()) {
                    panic!("\nUnexpected success while compiling: {}", source,);
                }
            }
            Err(parser_error) => {
                panic!("Failure while parsing:\n{}\n{}", source, parser_error);
            }
        }
    }

    mod should_fail {
        use super::*;

        #[test]
        fn wildcard_as_value() {
            let source = "
x = 1 + _
";
            check_compilation_fails(source);
        }

        #[test]
        fn match_insufficient_patterns() {
            let source = "
match 0, 1
  x then x
";
            check_compilation_fails(source);
        }

    }
}
