use koto::{prelude::*, runtime::Result, PtrMut};
use std::{
    ops::Deref,
    path::{Path, PathBuf},
};

struct ExampleTestRunner {
    koto: Koto,
    output: PtrMut<String>,
}

impl ExampleTestRunner {
    fn new() -> Self {
        let output = PtrMut::from(String::new());

        Self {
            output: output.clone(),
            koto: Koto::with_settings(
                KotoSettings::default()
                    .with_stdout(OutputCapture {
                        output: output.clone(),
                    })
                    .with_stderr(OutputCapture { output }),
            ),
        }
    }

    fn compile_script(&mut self, script: &str) {
        if let Err(error) = self.koto.compile(script) {
            panic!("{}", error);
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

fn load_markdown_and_run_examples(path: &Path) {
    if !path.exists() {
        panic!("Path doesn't exist: {path:?}");
    }

    let markdown = std::fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("Unable to load path '{:?}'", &path));

    use pulldown_cmark::{CodeBlockKind, Event::*, Parser, Tag::*};

    let mut in_koto_code = false;

    let mut runner = ExampleTestRunner::new();
    let mut code_block = String::with_capacity(128);
    let mut script = String::with_capacity(128);
    let mut expected_output = String::with_capacity(128);
    let mut skip_check = false;
    let mut skip_run = false;

    for event in Parser::new(&markdown) {
        match event {
            Start(CodeBlock(CodeBlockKind::Fenced(lang))) => {
                let mut lang_info = lang.deref().split(',');
                // Coffeescript highlighting is used for the code example in the readme
                if matches!(lang_info.next(), Some("koto" | "coffee")) {
                    in_koto_code = true;
                    code_block.clear();
                    let modifier = lang_info.next();
                    skip_check = matches!(modifier, Some("skip_check"));
                    skip_run = matches!(modifier, Some("skip_run"));
                }
            }
            Text(text) if in_koto_code => code_block.push_str(&text),
            End(CodeBlock(_)) => {
                if in_koto_code {
                    in_koto_code = false;

                    script.clear();
                    expected_output.clear();

                    for line in code_block.lines() {
                        if line.starts_with("print! ") {
                            script.push_str(&line.replacen("print! ", "print ", 1));
                            script.push('\n');
                        } else if line.starts_with("check! ") {
                            expected_output.push_str(line.trim_start_matches("check! "));
                            expected_output.push('\n');
                        } else {
                            script.push_str(line);
                            script.push('\n')
                        }
                    }

                    if skip_run {
                        runner.compile_script(&script);
                    } else {
                        runner.run_script(&script, &expected_output, skip_check);
                    }
                }
            }
            _ => {}
        }
    }
}

fn run_doc_examples(subfolder: &[&str], name: &str) {
    let mut path = PathBuf::new();
    path.extend([env!("CARGO_MANIFEST_DIR"), "..", "..", "docs"]);
    path.extend(subfolder);
    path.push(format!("{name}.md"));
    path = path.canonicalize().unwrap();
    load_markdown_and_run_examples(&path);
}

mod core_lib {
    macro_rules! test_doc_examples {
        ($path: expr, $name:ident) => {
            #[test]
            #[allow(non_snake_case)]
            fn $name() {
                super::run_doc_examples($path, stringify!($name))
            }
        };
    }

    macro_rules! test_top_level_examples {
        ($name:ident) => {
            test_doc_examples!(&["."], $name);
        };
    }

    macro_rules! test_core_lib_examples {
        ($name:ident) => {
            test_doc_examples!(&["core_lib"], $name);
        };
    }

    test_top_level_examples!(about);
    test_top_level_examples!(language_guide);
    test_top_level_examples!(README);

    test_core_lib_examples!(iterator);
    test_core_lib_examples!(koto);
    test_core_lib_examples!(list);
    test_core_lib_examples!(map);
    test_core_lib_examples!(number);
    test_core_lib_examples!(os);
    test_core_lib_examples!(range);
    test_core_lib_examples!(string);
    test_core_lib_examples!(test);
    test_core_lib_examples!(tuple);
}
