use itertools::join;
use koto_bytecode::{Chunk, CompilerSettings, Loader};
use koto_runtime::{prelude::*, Error, Ptr, Result};
use std::ops::Deref;

use crate::OutputCapture;

struct ExampleTestRunner {
    loader: Loader,
    vm: KotoVm,
    output: OutputCapture,
}

impl ExampleTestRunner {
    fn new(mut prelude_entries: ValueMap) -> Self {
        let (vm, output) = OutputCapture::make_vm_with_output_capture();
        vm.prelude().data_mut().extend(prelude_entries.drain(..));

        Self {
            loader: Loader::default(),
            vm,
            output,
        }
    }

    fn compile_example(&mut self, script: &str, sections: &[String]) -> Result<Ptr<Chunk>> {
        self.loader
            .compile_script(script, None, CompilerSettings::default())
            .map_err(|error| {
                Error::from(format!(
                    "
An example in '{}' failed to compile: {error}",
                    join(sections.iter(), ".")
                ))
            })
    }

    fn run_example(
        &mut self,
        script: &str,
        sections: &[String],
        expected_output: &str,
        skip_check: bool,
    ) -> Result<()> {
        self.output.clear();

        let chunk = self.compile_example(script, sections)?;

        if let Err(error) = self.vm.run(chunk.clone()) {
            println!("\n--------\n{script}\n--------\n");
            println!("Constants\n---------\n{}\n", chunk.constants);
            let script_lines = script.lines().collect::<Vec<_>>();
            println!(
                "Instructions\n------------\n{}",
                Chunk::instructions_as_string(chunk, &script_lines)
            );
            println!("{error}");

            return Err(error);
        }

        if !skip_check {
            let output = self.output.captured_output();
            if expected_output != output.deref() {
                return runtime_error!(
                    "
Example output mismatch in '{}':

--------

{script}
--------

Expected:
{expected_output}

Actual:
{}
",
                    join(sections.iter(), " / "),
                    output.deref()
                );
            }
        }

        Ok(())
    }
}

/// Runs Koto code examples found in a markdown document
///
/// Code blocks tagged with `koto` will be compiled and run.
///
/// Any lines prefixed with `print!` in the Koto example will be replaced with a call to `print`.
/// Any lines prefixed with `check!` will be added to the expected output.
///
/// The expected output will then be compared against the output as captured by the example runner.
///
/// The following additional code block tags can be used to control how an example is tested.
/// - `skip_check`: the example will be compiled and run, but the output won't be checked.
/// - `skip_run`: the example will be compiled but not run.
pub fn run_koto_examples_in_markdown(markdown: &str, prelude_entries: ValueMap) -> Result<()> {
    use pulldown_cmark::{CodeBlockKind, Event::*, Parser, Tag::*};

    let mut in_heading = false;
    let mut in_koto_code = false;

    let mut runner = ExampleTestRunner::new(prelude_entries);
    let mut code_block = String::with_capacity(128);
    let mut script = String::with_capacity(128);
    let mut expected_output = String::with_capacity(128);
    let mut skip_check = false;
    let mut skip_run = false;
    let mut headings: Vec<String> = Vec::with_capacity(4);
    let mut current_level = None;

    for event in Parser::new(markdown) {
        match event {
            Text(text) if in_koto_code => code_block.push_str(&text),
            Text(text) if in_heading => headings.last_mut().unwrap().push_str(&text),
            Code(inline_code) if in_heading => headings.last_mut().unwrap().push_str(&inline_code),
            Start(Heading(new_level, _, _)) => {
                if let Some(current_level) = current_level {
                    if new_level <= current_level {
                        headings.truncate(new_level as usize - 1);
                    }
                }
                headings.push(String::new());
                current_level = Some(new_level);
                in_heading = true;
            }
            End(Heading(_, _, _)) => {
                in_heading = false;
            }
            Start(CodeBlock(CodeBlockKind::Fenced(lang))) => {
                let mut lang_info = lang.deref().split(',');
                if matches!(lang_info.next(), Some("koto")) {
                    in_koto_code = true;
                    code_block.clear();
                    let modifier = lang_info.next();
                    skip_check = matches!(modifier, Some("skip_check"));
                    skip_run = matches!(modifier, Some("skip_run"));
                }
            }
            End(CodeBlock(_)) if in_koto_code => {
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
                    runner.compile_example(&script, &headings)?;
                } else {
                    runner.run_example(&script, &headings, &expected_output, skip_check)?;
                }
            }
            _ => {}
        }
    }

    Ok(())
}
