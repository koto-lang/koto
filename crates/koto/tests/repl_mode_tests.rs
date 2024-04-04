//! A collection of tests that validate REPL-like usage of Koto
//!
//! The main difference between the normal mode is that top-level IDs get automatically exported so
//! that each REPL entry can be compiled and executed as a separate chunk.
//! The exports map gets populated with any top-level assigned IDs, and is then made available to
//! each subsequent chunk.

use koto::{prelude::*, runtime::Result, PtrMut};

fn run_repl_mode_test(inputs_and_expected_outputs: &[(&str, &str)]) {
    let output = PtrMut::from(String::new());

    let mut koto = Koto::with_settings(
        KotoSettings {
            export_top_level_ids: true,
            ..Default::default()
        }
        .with_stdout(OutputCapture {
            output: output.clone(),
        })
        .with_stderr(OutputCapture {
            output: output.clone(),
        }),
    );

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
    fn id(&self) -> KString {
        "_stdout_".into()
    }
}

impl KotoRead for OutputCapture {}
impl KotoWrite for OutputCapture {
    fn write(&self, bytes: &[u8]) -> Result<()> {
        let bytes_str = match std::str::from_utf8(bytes) {
            Ok(s) => s,
            Err(e) => return Err(e.to_string().into()),
        };
        self.output.borrow_mut().push_str(bytes_str);
        Ok(())
    }

    fn write_line(&self, output: &str) -> Result<()> {
        let mut unlocked = self.output.borrow_mut();
        unlocked.push_str(output);
        unlocked.push('\n');
        Ok(())
    }

    fn flush(&self) -> Result<()> {
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
    fn import_single_item() {
        run_repl_mode_test(&[
            ("from string import to_number", ""),
            ("print to_number '0x7f'", "127"),
        ]);
    }

    #[test]
    fn import_multiple_items() {
        run_repl_mode_test(&[
            ("from string import to_lowercase, to_uppercase", ""),
            ("print to_lowercase 'HEY'", "hey"),
            ("print to_uppercase 'hey'", "HEY"),
        ]);
    }

    #[test]
    fn import_with_assignment() {
        run_repl_mode_test(&[
            ("print2 = from io import print", ""),
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
  print 'x: {x}'
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

    #[test]
    fn add_assign_number() {
        run_repl_mode_test(&[
            ("print x = 1", "1"),
            ("print x += 1", "2"),
            ("print x += 1", "3"),
        ]);
    }

    #[test]
    fn subtract_assign_number() {
        run_repl_mode_test(&[
            ("print x = 1", "1"),
            ("print x -= 1", "0"),
            ("print x -= 1", "-1"),
        ]);
    }
}
