use {
    koto::{
        bytecode::Chunk,
        runtime::{KotoFile, KotoRead, KotoWrite, RuntimeError},
        Koto, KotoSettings,
    },
    std::{
        cell::RefCell,
        fmt,
        ops::Deref,
        path::{Path, PathBuf},
        rc::Rc,
    },
};

struct ExampleTestRunner {
    koto: Koto,
    output: Rc<RefCell<String>>,
}

impl ExampleTestRunner {
    fn new() -> Self {
        let output = Rc::new(RefCell::new(String::new()));

        Self {
            output: output.clone(),
            koto: Koto::with_settings(KotoSettings {
                repl_mode: false,
                stdout: Rc::new(OutputCapture {
                    output: output.clone(),
                }),
                stderr: Rc::new(OutputCapture { output }),
                ..Default::default()
            }),
        }
    }

    fn run_script(&mut self, script: &str, expected_output: &str, skip_check: bool) {
        self.output.borrow_mut().clear();

        match self.koto.compile(script) {
            Ok(chunk) => {
                if let Err(error) = self.koto.run() {
                    println!("\n--------\n{script}\n--------\n");
                    println!("Constants\n---------\n{}\n", chunk.constants);
                    let script_lines = script.lines().collect::<Vec<_>>();
                    println!(
                        "Instructions\n------------\n{}",
                        Chunk::instructions_as_string(chunk, &script_lines)
                    );

                    panic!("{error}");
                }

                if !skip_check {
                    let output = self.output.borrow();
                    if expected_output != output.deref() {
                        println!("\nError - mismatch in example output");
                        println!("\n--------\n\n{script}\n--------\n");
                        println!(
                            "Expected:\n\n{expected_output}\nActual:\n\n{}",
                            output.deref()
                        );
                        panic!();
                    }
                }
            }
            Err(error) => panic!("{}", error),
        }
    }
}

// Captures output from Koto in a String
#[derive(Debug)]
struct OutputCapture {
    output: Rc<RefCell<String>>,
}

impl KotoFile for OutputCapture {}
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

impl fmt::Display for OutputCapture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("_stdout_")
    }
}

fn load_markdown_and_run_examples(path: &Path) {
    if !path.exists() {
        panic!("Path doesn't exist: {:?}", path);
    }

    let markdown = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Unable to load path '{:?}'", &path));

    use pulldown_cmark::{CodeBlockKind, Event::*, Parser, Tag::*};

    let mut in_koto_code = false;

    let mut runner = ExampleTestRunner::new();
    let mut code_block = String::with_capacity(128);
    let mut script = String::with_capacity(128);
    let mut expected_output = String::with_capacity(128);

    for event in Parser::new(&markdown) {
        match event {
            Start(CodeBlock(CodeBlockKind::Fenced(lang))) if lang.deref() == "koto" => {
                in_koto_code = true;
                code_block.clear();
            }
            Text(text) if in_koto_code => code_block.push_str(&text),
            End(CodeBlock(_)) => {
                if in_koto_code {
                    in_koto_code = false;

                    script.clear();
                    expected_output.clear();
                    let mut skip_check = false;

                    for line in code_block.lines() {
                        if line.starts_with("print! ") {
                            script.push_str(&line.replacen("print! ", "print ", 1));
                            script.push('\n');
                        } else if line.starts_with("check! ") {
                            expected_output.push_str(line.trim_start_matches("check! "));
                            expected_output.push('\n');
                        } else if line.starts_with("skip_check!") {
                            skip_check = true;
                        } else {
                            script.push_str(line);
                            script.push('\n')
                        }
                    }

                    runner.run_script(&script, &expected_output, skip_check);
                }
            }
            _ => {}
        }
    }
}

fn run_core_lib_examples(name: &str) {
    let mut path = PathBuf::new();
    path.push(env!("CARGO_MANIFEST_DIR"));
    path.push("..");
    path.push("..");
    path.push("docs");
    path.push("reference");
    path.push("core_lib");
    path.push(format!("{name}.md"));
    path = path.canonicalize().unwrap();
    load_markdown_and_run_examples(&path);
}

macro_rules! test_core_lib_examples {
    ($name:ident) => {
        #[test]
        fn $name() {
            run_core_lib_examples(stringify!($name));
        }
    };
}

test_core_lib_examples!(iterator);
test_core_lib_examples!(koto);
test_core_lib_examples!(list);
test_core_lib_examples!(map);
test_core_lib_examples!(num2);
test_core_lib_examples!(num4);
test_core_lib_examples!(number);
test_core_lib_examples!(os);
test_core_lib_examples!(range);
test_core_lib_examples!(string);
test_core_lib_examples!(test);
test_core_lib_examples!(tuple);
