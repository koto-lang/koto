mod runtime {
    use {
        koto_bytecode::chunk_to_string_annotated,
        koto_runtime::{Loader, Vm},
    };

    fn check_script_fails(script: &str) {
        let mut vm = Vm::default();

        let print_chunk = |script: &str, chunk| {
            println!("{}\n", script);
            let script_lines = script.lines().collect::<Vec<_>>();

            println!("{}", chunk_to_string_annotated(chunk, &script_lines));
        };

        let mut loader = Loader::default();
        let chunk = match loader.compile_script(script, &None) {
            Ok(chunk) => chunk,
            Err(error) => {
                print_chunk(script, vm.chunk());
                panic!("Error while compiling script: {}", error);
            }
        };

        if let Ok(result) = vm.run(chunk) {
            print_chunk(script, vm.chunk());
            panic!("Script didn't fail as expected, result: {}", result)
        }
    }

    mod should_fail {
        use super::*;

        #[test]
        fn iterator_consume_should_propagate_error() {
            let script = "
import test.assert
(1..5)
  .each |_| assert false
  .consume()
";
            check_script_fails(script);
        }

        #[test]
        fn iterator_count_should_propagate_error() {
            let script = "
import test.assert
(1..5)
  .each |_| assert false
  .count()
";
            check_script_fails(script);
        }
    }
}
