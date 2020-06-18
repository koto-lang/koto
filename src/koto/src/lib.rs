pub use {
    koto_bytecode::{
        chunk_to_string, chunk_to_string_annotated, Chunk, Compiler, DebugInfo, InstructionReader,
    },
    koto_parser::{num4::Num4, Ast, Function, Parser, Position},
    koto_runtime::{
        external_error, make_external_value, type_as_string, Error, ExternalValue, Loader,
        RuntimeFunction, RuntimeResult, Value, ValueHashMap, ValueList, ValueMap, ValueVec,
    },
    koto_std::{get_external_instance, visit_external_value},
};

use {
    koto_runtime::Vm,
    std::{path::PathBuf, sync::Arc},
};

#[derive(Copy, Clone, Default)]
pub struct Options {
    pub show_annotated: bool,
    pub show_bytecode: bool,
    pub repl_mode: bool,
}

#[derive(Default)]
pub struct Koto {
    script: String,
    script_path: Option<PathBuf>,
    runtime: Vm,
    options: Options,
    loader: Loader,
    chunk: Option<Arc<Chunk>>,
}

impl Koto {
    pub fn new() -> Self {
        let mut result = Self::default();

        koto_std::register(&mut result.runtime);

        let mut env = ValueMap::new();
        env.add_value("script_dir", Value::Empty);
        env.add_value("script_path", Value::Empty);
        env.add_list("args", ValueList::default());
        result.runtime.prelude_mut().add_map("env", env);

        result
    }

    pub fn with_options(options: Options) -> Self {
        let mut result = Self::new();
        result.options = options;
        result
    }

    pub fn compile(&mut self, script: &str) -> Result<Arc<Chunk>, String> {
        let compile_result = if self.options.repl_mode {
            self.loader.compile_repl(script)
        } else {
            self.loader.compile_script(script, &self.script_path)
        };

        match compile_result {
            Ok(chunk) => {
                self.chunk = Some(chunk.clone());
                self.script = script.to_string();
                if self.options.show_annotated {
                    let script_lines = script.lines().collect::<Vec<_>>();
                    println!(
                        "{}",
                        chunk_to_string_annotated(chunk.clone(), &script_lines)
                    );
                } else if self.options.show_bytecode {
                    println!("{}", chunk_to_string(chunk.clone()));
                }
                Ok(chunk)
            }
            Err(e) => Err(e),
            // Err(e) => Err(self.format_error(&e.to_string(), &self.script, e.span.start, e.span.end)),
        }
    }

    pub fn run_with_args(&mut self, args: &[String]) -> Result<Value, String> {
        self.set_args(args);
        self.run()
    }

    pub fn run(&mut self) -> Result<Value, String> {
        let chunk = self.chunk.clone();
        match chunk {
            Some(chunk) => self.run_chunk(chunk),
            None => Err("koto.run: missing compiled chunk".to_string()),
        }
    }

    pub fn run_chunk(&mut self, chunk: Arc<Chunk>) -> Result<Value, String> {
        let result = match self.runtime.run(chunk) {
            Ok(result) => result,
            Err(e) => {
                return Err(match &e {
                    Error::VmError {
                        message,
                        instruction,
                    } => self.format_vm_error(message, *instruction),
                    Error::ExternalError { message } => format!("Error: {}\n", message),
                })
            }
        };

        if self.options.repl_mode {
            Ok(result)
        } else if let Some(main) = self.runtime.get_global_function("main") {
            self.call_function(&main, &[])
        } else {
            Ok(result)
        }
    }

    pub fn prelude_mut(&mut self) -> &mut ValueMap {
        self.runtime.prelude_mut()
    }

    pub fn set_args(&mut self, args: &[String]) {
        use Value::{Map, Str};

        let koto_args = args
            .iter()
            .map(|arg| Str(Arc::new(arg.to_string())))
            .collect::<ValueVec>();

        match self.runtime.prelude_mut().data_mut().get_mut("env").unwrap() {
            Map(map) => map
                .data_mut()
                .add_list("args", ValueList::with_data(koto_args)),
            _ => unreachable!(),
        }
    }

    pub fn set_script_path(&mut self, path: Option<PathBuf>) {
        use Value::{Empty, Map, Str};

        let (script_dir, script_path) = match &path {
            Some(path) => (
                path.parent()
                    .map(|p| {
                        Str(Arc::new(
                            p.to_str().expect("invalid script path").to_string(),
                        ))
                    })
                    .or(Some(Empty))
                    .unwrap(),
                Str(Arc::new(path.display().to_string())),
            ),
            None => (Empty, Empty),
        };

        self.script_path = path;

        match self.runtime.prelude_mut().data_mut().get_mut("env").unwrap() {
            Map(map) => {
                let mut map = map.data_mut();
                map.add_value("script_dir", script_dir);
                map.add_value("script_path", script_path);
            }
            _ => unreachable!(),
        }
    }

    pub fn call_function_by_name(
        &mut self,
        function_name: &str,
        args: &[Value],
    ) -> Result<Value, String> {
        match self.runtime.get_global_function(function_name) {
            Some(f) => self.call_function(&f, args),
            None => Err(format!(
                "Runtime error: function '{}' not found",
                function_name
            )),
        }
    }

    pub fn call_function(
        &mut self,
        function: &RuntimeFunction,
        args: &[Value],
    ) -> Result<Value, String> {
        match self.runtime.run_function(function, args) {
            Ok(result) => Ok(result),
            Err(e) => Err(match e {
                Error::VmError {
                    message,
                    instruction,
                } => self.format_vm_error(&message, instruction),
                Error::ExternalError { message } => format!("Error: {}\n", message,),
            }),
        }
    }

    fn format_vm_error(&self, message: &str, instruction: usize) -> String {
        match self.runtime.chunk().debug_info.get_source_span(instruction) {
            Some(span) => self.format_error(message, &self.script, span.start, span.end),
            None => format!(
                "Runtime error at instruction {}: {}\n",
                instruction, message
            ),
        }
    }

    fn format_error(
        &self,
        message: &str,
        script: &str,
        start_pos: Position,
        end_pos: Position,
    ) -> String {
        let (excerpt, padding) = {
            let excerpt_lines = script
                .lines()
                .skip((start_pos.line - 1) as usize)
                .take((end_pos.line - start_pos.line + 1) as usize)
                .collect::<Vec<_>>();

            let line_numbers = (start_pos.line..=end_pos.line)
                .map(|n| n.to_string())
                .collect::<Vec<_>>();

            let number_width = line_numbers.iter().max_by_key(|n| n.len()).unwrap().len();

            let padding = " ".repeat(number_width + 2);

            if excerpt_lines.len() == 1 {
                let mut excerpt = format!(
                    " {:>width$} | {}\n",
                    line_numbers.first().unwrap(),
                    excerpt_lines.first().unwrap(),
                    width = number_width
                );

                excerpt += &format!(
                    "{}|{}",
                    padding,
                    format!(
                        "{}{}",
                        " ".repeat(start_pos.column as usize),
                        "^".repeat((end_pos.column - start_pos.column) as usize)
                    ),
                );

                (excerpt, padding)
            } else {
                let mut excerpt = String::new();

                for (excerpt_line, line_number) in excerpt_lines.iter().zip(line_numbers.iter()) {
                    excerpt += &format!(
                        " {:>width$} | {}\n",
                        line_number,
                        excerpt_line,
                        width = number_width
                    );
                }

                (excerpt, padding)
            }
        };

        format!(
            "{message}\n --> {}:{}\n{padding}|\n{excerpt}",
            start_pos.line,
            start_pos.column,
            padding = padding,
            excerpt = excerpt,
            message = message
        )
    }
}
