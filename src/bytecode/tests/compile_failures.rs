mod bytecode {
    use {koto_bytecode::Compiler, koto_parser::Parser};

    fn check_compilation_fails(source: &str) {
        match Parser::parse(&source) {
            Ok((ast, _constants)) => {
                if let Ok(_) = Compiler::compile(&ast, koto_bytecode::Options::default()) {
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

        // #[test]
        // fn for_loop_insufficient_args() {
        //     let source = "
// for x in a, b
  // x
// ";
        //     check_compilation_fails(source);
        // }
    }
}
