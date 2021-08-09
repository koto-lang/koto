use {
    koto_bytecode::Chunk,
    koto_runtime::{KotoLogger, Loader, Vm, VmSettings},
    parking_lot::Mutex,
    std::sync::Arc,
};

struct TestLogger {
    output: Arc<Mutex<Vec<String>>>,
}

impl KotoLogger for TestLogger {
    fn writeln(&self, s: &str) {
        self.output.lock().push(s.to_string());
    }
}

mod vm {
    use super::*;

    fn check_logged_output(script: &str, expected_output: &[String]) {
        let output = Arc::new(Mutex::new(Vec::new()));

        let mut vm = Vm::with_settings(VmSettings {
            logger: Arc::new(TestLogger {
                output: output.clone(),
            }),
        });

        let print_chunk = |script: &str, chunk: Arc<Chunk>| {
            println!("{}\n", script);
            let script_lines = script.lines().collect::<Vec<_>>();

            println!("Constants\n---------\n{}\n", chunk.constants.to_string());
            println!(
                "Instructions\n------------\n{}",
                Chunk::instructions_as_string(chunk, &script_lines)
            );
        };

        let mut loader = Loader::default();
        let chunk = match loader.compile_script(script, &None) {
            Ok(chunk) => chunk,
            Err(error) => {
                print_chunk(script, vm.chunk());
                panic!("Error while compiling script: {}", error);
            }
        };

        match vm.run(chunk) {
            Ok(_) => {
                assert_eq!(output.lock().as_slice(), expected_output);
            }
            Err(e) => {
                print_chunk(script, vm.chunk());
                panic!("Error while running script: {}", e.to_string());
            }
        }
    }

    #[test]
    fn print_loop() {
        let script = r#"
for i in 0..5
  "foo {}".print i
"#;
        check_logged_output(
            script,
            &[
                "foo 0".to_string(),
                "foo 1".to_string(),
                "foo 2".to_string(),
                "foo 3".to_string(),
                "foo 4".to_string(),
            ],
        );
    }

    #[test]
    fn debug() {
        let script = "debug 2 + 2";

        check_logged_output(script, &["[1] 2 + 2: 4".to_string()]);
    }
}
