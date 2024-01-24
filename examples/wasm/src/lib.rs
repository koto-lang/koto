use koto::{prelude::*, runtime::Result, PtrMut};
use wasm_bindgen::prelude::*;

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

struct BlockedInput {}

impl KotoFile for BlockedInput {
    fn id(&self) -> KString {
        "_stdin_".into()
    }
}

impl KotoWrite for BlockedInput {}
impl KotoRead for BlockedInput {
    fn read_line(&self) -> Result<Option<String>> {
        runtime_error!("Unsupported in the browser")
    }

    fn read_to_string(&self) -> Result<String> {
        runtime_error!("Unsupported in the browser")
    }
}

// Runs an input program and returns the output as a String
#[wasm_bindgen]
pub fn compile_and_run(input: &str) -> String {
    let output = PtrMut::from(String::new());

    let mut koto = Koto::with_settings(
        KotoSettings::default()
            .with_stdin(BlockedInput {})
            .with_stdout(OutputCapture {
                output: output.clone(),
            })
            .with_stderr(OutputCapture {
                output: output.clone(),
            }),
    );

    match koto.compile(input) {
        Ok(_) => match koto.run() {
            Ok(_) => std::mem::take(&mut output.borrow_mut()),
            Err(error) => format!("Runtime error: {error}"),
        },
        Err(error) => format!("Compilation error: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn one_plus_one() {
        assert_eq!(compile_and_run("print 1 + 1"), "2\n");
    }

    #[wasm_bindgen_test]
    fn tuple_to_list() {
        assert_eq!(compile_and_run("print (1, 2, 3).to_list()"), "[1, 2, 3]\n");
    }
}
