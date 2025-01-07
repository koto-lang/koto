mod bytecode {
    use koto_bytecode::{Compiler, CompilerSettings};

    fn check_compilation_fails(source: &str) {
        if Compiler::compile(source, None, CompilerSettings::default()).is_ok() {
            panic!("\nUnexpected success while compiling: {source}");
        }
    }

    mod should_fail {
        use super::*;

        #[test]
        fn wildcard_access() {
            let source = "
f _x
";
            check_compilation_fails(source);
        }

        #[test]
        fn wildcard_as_rhs() {
            let source = "
x = 1 + _
";
            check_compilation_fails(source);
        }

        #[test]
        fn break_outside_of_loop() {
            let source = "
break
";
            check_compilation_fails(source);
        }

        #[test]
        fn continue_outside_of_loop() {
            let source = "
continue
";
            check_compilation_fails(source);
        }

        mod try_catch {
            use super::*;

            #[test]
            fn missing_type_hint_on_first_catch_block() {
                let source = "
try
  f()
catch x
  x
catch y
  y
                    ";
                check_compilation_fails(source);
            }

            #[test]
            fn missing_type_hint_on_first_catch_block_with_wildcard_arg() {
                let source = "
try
  f()
catch _x
  0
catch y
  y
                    ";
                check_compilation_fails(source);
            }

            #[test]
            fn type_hint_on_last_catch_block() {
                let source = "
try
  f()
catch x: String
  x
catch x: Bool
  x
                    ";
                check_compilation_fails(source);
            }

            #[test]
            fn type_hint_on_last_catch_block_with_wildcard_arg() {
                let source = "
try
  f()
catch x: String
  x
catch _x: Bool
  0
                    ";
                check_compilation_fails(source);
            }
        }

        mod match_failures {
            use super::*;

            #[test]
            fn match_insufficient_patterns() {
                let source = "
match 0, 1
  x then x
";
                check_compilation_fails(source);
            }

            #[test]
            fn match_too_many_patterns() {
                let source = "
match 0
  x, y then x + y
";
                check_compilation_fails(source);
            }

            #[test]
            fn match_ellipsis_out_of_position() {
                let source = "
match [1, 2, 3]
  (x, ..., y) then 0
";
                check_compilation_fails(source);
            }
        }

        mod export {
            use super::*;

            #[test]
            fn id_without_assignment() {
                let source = "
export x
";
                check_compilation_fails(source);
            }

            #[test]
            fn list() {
                let source = "
export [1, 2, 3]
";
                check_compilation_fails(source);
            }
        }
    }
}
