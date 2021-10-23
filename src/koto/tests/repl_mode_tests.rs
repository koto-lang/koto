use {
    koto::{
        runtime::{KotoFile, KotoRead, KotoWrite, Mutex, RuntimeError},
        Koto, KotoSettings,
    },
    std::{fmt, sync::Arc},
};

fn run_repl_mode_test(inputs_and_expected_outputs: &[(&str, &str)]) {
    let output = Arc::new(Mutex::new(String::new()));

    let mut koto = Koto::with_settings(KotoSettings {
        repl_mode: true,
        stdout: Arc::new(OutputCapture {
            output: output.clone(),
        }),
        stderr: Arc::new(OutputCapture {
            output: output.clone(),
        }),
        ..Default::default()
    });

    for (input, expected_output) in inputs_and_expected_outputs {
        koto.compile(input).unwrap();
        koto.run().unwrap();

        assert_eq!(&output.lock().trim(), &expected_output.trim());

        output.lock().clear();
    }
}

// Captures output from Koto in a String
#[derive(Debug)]
struct OutputCapture {
    output: Arc<Mutex<String>>,
}

impl KotoFile for OutputCapture {}
impl KotoRead for OutputCapture {}

impl KotoWrite for OutputCapture {
    fn write(&self, bytes: &[u8]) -> Result<(), RuntimeError> {
        let bytes_str = match std::str::from_utf8(bytes) {
            Ok(s) => s,
            Err(e) => return Err(e.to_string().into()),
        };
        self.output.lock().push_str(bytes_str);
        Ok(())
    }

    fn write_line(&self, output: &str) -> Result<(), RuntimeError> {
        let mut unlocked = self.output.lock();
        unlocked.push_str(output);
        unlocked.push('\n');
        Ok(())
    }

    fn flush(&self) -> Result<(), RuntimeError> {
        Ok(())
    }
}

impl fmt::Display for OutputCapture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("_stdout_")
    }
}

mod repl_mode {
    use super::*;

    #[test]
    fn basic_arithmetic() {
        run_repl_mode_test(&[("a = 2", ""), ("io.print a + a", "4")]);
    }

    #[test]
    fn import_print() {
        run_repl_mode_test(&[("import io.print", ""), ("print 'hello!'", "hello!")]);
    }

    #[test]
    fn for_loop() {
        run_repl_mode_test(&[
            ("min = 1", ""),
            ("max = 3", ""),
            (
                "
for x in min..=max
  io.print 'x: $x'
",
                "
x: 1
x: 2
x: 3
",
            ),
        ]);
    }
}
