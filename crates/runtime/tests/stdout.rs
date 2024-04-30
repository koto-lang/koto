use koto_bytecode::{Chunk, CompilerSettings, Loader};
use koto_runtime::Ptr;
use koto_test_utils::OutputCapture;

mod vm {
    use super::*;

    fn check_logged_output(script: &str, expected_output: &str) {
        let (mut vm, output) = OutputCapture::make_vm_with_output_capture();

        let print_chunk = |script: &str, chunk: Ptr<Chunk>| {
            println!("{script}\n");
            let script_lines = script.lines().collect::<Vec<_>>();

            println!("Constants\n---------\n{}\n", chunk.constants);
            println!(
                "Instructions\n------------\n{}",
                Chunk::instructions_as_string(chunk, &script_lines)
            );
        };

        let mut loader = Loader::default();
        let chunk = match loader.compile_script(script, None, CompilerSettings::default()) {
            Ok(chunk) => chunk,
            Err(error) => {
                print_chunk(script, vm.chunk());
                panic!("Error while compiling script: {error}");
            }
        };

        match vm.run(chunk) {
            Ok(_) => {
                assert_eq!(output.captured_output().as_str(), expected_output);
            }
            Err(e) => {
                print_chunk(script, vm.chunk());
                panic!("Error while running script: {e}");
            }
        }
    }

    #[test]
    fn print_loop() {
        let script = "
for i in 0..5
  print 'foo {i}'
";
        check_logged_output(
            script,
            "\
foo 0
foo 1
foo 2
foo 3
foo 4
",
        );
    }

    #[test]
    fn print_value_with_overridden_display() {
        let script = "
foo =
  @display: || 'Hello from @display'
  @type: 'Foo'
print foo
";
        check_logged_output(
            script,
            "\
Hello from @display
",
        );
    }

    #[test]
    fn debug() {
        let script = "debug 2 + 2";

        check_logged_output(script, "[1] 2 + 2: 4\n");
    }

    #[test]
    fn write_via_stdout() {
        let script = "
stdout = io.stdout()
stdout.write 'abc'
stdout.write 'def'
stdout.write_line 'ghi'
";

        check_logged_output(script, "abcdefghi\n");
    }

    #[test]
    fn write_via_stderr() {
        let script = "
stderr = io.stderr()
stderr.write '123'
stderr.write '456'
stderr.write_line '789'
";

        check_logged_output(script, "123456789\n");
    }
}
