//! A collection of tests that validate REPL-like usage of Koto
//!
//! The main difference between the normal mode is that top-level IDs get automatically exported so
//! that each REPL entry can be compiled and executed as a separate chunk.
//! The exports map gets populated with any top-level assigned IDs, and is then made available to
//! each subsequent chunk.

use {koto::prelude::*, std::rc::Rc};

fn run_repl_mode_test(inputs_and_expected_outputs: &[(&str, &str)]) {
    let output = PtrMut::from(String::new());

    let mut koto = Koto::with_settings(KotoSettings {
        export_top_level_ids: true,
        stdout: Rc::new(OutputCapture {
            output: output.clone(),
        }),
        stderr: Rc::new(OutputCapture {
            output: output.clone(),
        }),
        ..Default::default()
    });

    let mut chunks = Vec::with_capacity(inputs_and_expected_outputs.len());

    for (input, expected_output) in inputs_and_expected_outputs {
        match koto.compile(input) {
            Ok(chunk) => chunks.push((input, chunk)),
            Err(error) => panic!("{}", error),
        }

        if let Err(error) = koto.run() {
            for (input, chunk) in chunks.iter() {
                println!("\n--------\n{input}\n--------\n");
                println!("Constants\n---------\n{}\n", chunk.constants);
                println!(
                    "Instructions\n------------\n{}",
                    Chunk::instructions_as_string(chunk.clone(), &[input])
                );
            }

            panic!("{error}");
        }

        assert_eq!(&output.borrow().trim(), &expected_output.trim());

        output.borrow_mut().clear();
    }
}

// Captures output from Koto in a String
#[derive(Debug)]
struct OutputCapture {
    output: PtrMut<String>,
}

impl KotoFile for OutputCapture {
    fn id(&self) -> ValueString {
        "_stdout_".into()
    }
}

impl KotoRead for OutputCapture {}
impl KotoWrite for OutputCapture {
    fn write(&self, bytes: &[u8]) -> Result<(), RuntimeError> {
        let bytes_str = match std::str::from_utf8(bytes) {
            Ok(s) => s,
            Err(e) => return Err(e.to_string().into()),
        };
        self.output.borrow_mut().push_str(bytes_str);
        Ok(())
    }

    fn write_line(&self, output: &str) -> Result<(), RuntimeError> {
        let mut unlocked = self.output.borrow_mut();
        unlocked.push_str(output);
        unlocked.push('\n');
        Ok(())
    }

    fn flush(&self) -> Result<(), RuntimeError> {
        Ok(())
    }
}

mod repl_mode {
    use super::*;

    #[test]
    fn basic_arithmetic() {
        run_repl_mode_test(&[("a = 2", ""), ("print a + a", "4")]);
    }

    #[test]
    fn import_print() {
        run_repl_mode_test(&[
            ("print2 = import io.print", ""),
            ("print2 'hello!'", "hello!"),
        ]);
    }

    #[test]
    fn for_loop() {
        run_repl_mode_test(&[
            ("min = 1", ""),
            ("max = 3", ""),
            (
                "
for x in min..=max
  print 'x: $x'
",
                "
x: 1
x: 2
x: 3
",
            ),
        ]);
    }

    #[test]
    fn negated_id() {
        run_repl_mode_test(&[("a = 2", ""), ("b = -a", ""), ("print a + b", "0")]);
    }

    #[test]
    fn multi_assign() {
        run_repl_mode_test(&[("x, y = 1, 2", ""), ("print x + y", " 3")]);
    }
}
