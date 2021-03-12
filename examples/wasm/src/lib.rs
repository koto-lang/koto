use {
    koto::{runtime::KotoLogger, Koto, KotoSettings},
    std::sync::{Arc, Mutex},
    wasm_bindgen::prelude::*,
};

// A logger that captures output from Koto in a Vec of Strings
struct CaptureLogger {
    output: Arc<Mutex<Vec<String>>>,
}

impl KotoLogger for CaptureLogger {
    fn writeln(&self, output: &str) {
        self.output.lock().unwrap().push(output.to_string());
    }
}

// Runs an input program and returns the output as a String
#[wasm_bindgen]
pub fn compile_and_run(input: &str) -> String {
    let output = Arc::new(Mutex::new(Vec::new()));

    let mut koto = Koto::with_settings(KotoSettings {
        logger: Arc::new(CaptureLogger {
            output: output.clone(),
        }),
        ..Default::default()
    });

    match koto.compile(input) {
        Ok(_) => match koto.run() {
            Ok(_) => output.lock().unwrap().join("\n"),
            Err(e) => format!("Runtime error: {}", e),
        },
        Err(e) => format!("Compilation error: {}", e),
    }
}
