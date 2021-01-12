//! # Koto
//!
//! Pulls together the compiler and runtime for the Koto programming language.
//!
//! Programs can be compiled and executed with the [Koto] struct.
//!
//! ## Example
//!
//! ```
//! use koto::{Koto, runtime::Value};
//!
//! let mut koto = Koto::default();
//! match koto.compile("1 + 2") {
//!     Ok(_) => match koto.run() {
//!         Ok(result) => match result {
//!             Value::Number(n) => println!("{}", n), // 3.0
//!             other => panic!("Unexpected result: {}", other),
//!         },
//!         Err(runtime_error) => {
//!             panic!("Runtime error: {}", runtime_error);
//!         }
//!     },
//!     Err(compiler_error) => {
//!         panic!("Compiler error: {}", compiler_error);
//!     }
//! }
//! ```

pub use {koto_bytecode as bytecode, koto_parser as parser, koto_runtime as runtime};

use {
    koto_bytecode::{
        chunk_to_string, chunk_to_string_annotated, Chunk, CompilerError, LoaderError,
    },
    koto_parser::{ParserError, Position},
    koto_runtime::{
        type_as_string, Loader, RuntimeError, Value, ValueList, ValueMap, ValueVec, Vm,
    },
    std::{path::PathBuf, sync::Arc},
};

/// Settings used to control the behaviour of the [Koto] runtime
#[derive(Copy, Clone, Debug, Default)]
pub struct KotoSettings {
    pub run_tests: bool,
    pub show_annotated: bool,
    pub show_bytecode: bool,
    pub repl_mode: bool,
}

/// The main interface for the Koto language.
///
/// Example
#[derive(Default)]
pub struct Koto {
    script_path: Option<PathBuf>,
    runtime: Vm,
    pub settings: KotoSettings,
    loader: Loader,
    chunk: Option<Arc<Chunk>>,
}

impl Koto {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_settings(settings: KotoSettings) -> Self {
        let mut result = Self::new();
        result.settings = settings;
        result
    }

    pub fn compile(&mut self, script: &str) -> Result<Arc<Chunk>, LoaderError> {
        let compile_result = if self.settings.repl_mode {
            self.loader.compile_repl(script)
        } else {
            self.loader.compile_script(script, &self.script_path)
        };

        match compile_result {
            Ok(chunk) => {
                self.chunk = Some(chunk.clone());
                if self.settings.show_annotated {
                    println!("Constants\n---------\n{}\n", chunk.constants.to_string());

                    let script_lines = script.lines().collect::<Vec<_>>();
                    println!(
                        "Instructions\n------------\n{}",
                        chunk_to_string_annotated(chunk.clone(), &script_lines)
                    );
                } else if self.settings.show_bytecode {
                    println!("{}", chunk_to_string(chunk.clone()));
                }
                Ok(chunk)
            }
            Err(error) => Err(error),
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
        let result = self.runtime.run(chunk).map_err(|e| self.format_error(e))?;

        if self.settings.repl_mode {
            Ok(result)
        } else {
            if self.settings.run_tests {
                let _test_result = match self.runtime.get_global_value("tests") {
                    Some(Value::Map(tests)) => {
                        if let Err(error) = self.runtime.run_tests(tests) {
                            return Err(self.format_error(error));
                        }
                    }
                    Some(other) => {
                        return Err(format!(
                            "Expected a Map for the exported 'tests', found '{}'",
                            type_as_string(&other)
                        ))
                    }
                    None => {}
                };
            }

            if let Some(main) = self.runtime.get_global_function("main") {
                self.runtime
                    .run_function(main, &[])
                    .map_err(|e| self.format_error(e))
            } else {
                Ok(result)
            }
        }
    }

    pub fn prelude(&self) -> ValueMap {
        self.runtime.prelude()
    }

    pub fn set_args(&mut self, args: &[String]) {
        use Value::{Map, Str};

        let koto_args = args
            .iter()
            .map(|arg| Str(arg.as_str().into()))
            .collect::<ValueVec>();

        match self
            .runtime
            .prelude()
            .data_mut()
            .get_with_string_mut("koto")
            .unwrap()
        {
            Map(map) => map
                .data_mut()
                .add_list("args", ValueList::with_data(koto_args)),
            _ => unreachable!(),
        }
    }

    pub fn set_script_path(&mut self, path: Option<PathBuf>) {
        use Value::{Empty, Map, Str};

        let (script_dir, script_path) = match &path {
            Some(path) => {
                let path = path.canonicalize().expect("Invalid script path");

                let script_dir = path
                    .parent()
                    .map(|p| {
                        let s = p.to_string_lossy() + "/";
                        Str(s.into_owned().into())
                    })
                    .or(Some(Empty))
                    .unwrap();
                let script_path = Str(path.display().to_string().into());

                (script_dir, script_path)
            }
            None => (Empty, Empty),
        };

        self.script_path = path;

        match self
            .runtime
            .prelude()
            .data_mut()
            .get_with_string_mut("koto")
            .unwrap()
        {
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
            Some(f) => self.call_function(f, args),
            None => Err(format!(
                "Runtime error: function '{}' not found",
                function_name
            )),
        }
    }

    pub fn call_function(&mut self, function: Value, args: &[Value]) -> Result<Value, String> {
        self.runtime
            .run_function(function, args)
            .map_err(|e| self.format_error(e))
    }

    pub fn format_error(&self, error: RuntimeError) -> String {
        use RuntimeError::*;

        match error {
            VmError {
                message,
                chunk,
                instruction,
            } => self.format_vm_error(&message, chunk, instruction),
            ExternalError { message } => format!("Error: {}\n", message,),
        }
    }

    fn format_vm_error(&self, message: &str, chunk: Arc<Chunk>, instruction: usize) -> String {
        match chunk.debug_info.get_source_span(instruction) {
            Some(span) => self.format_error_with_excerpt(
                message,
                &chunk.source_path,
                &chunk.debug_info.source,
                span.start,
                span.end,
            ),
            None => format!(
                "Runtime error at instruction {}: {}\n",
                instruction, message
            ),
        }
    }

    pub fn format_loader_error(&self, error: LoaderError, source: &str) -> String {
        match error {
            LoaderError::ParserError(ParserError { error, span }) => self
                .format_error_with_excerpt(
                    &error.to_string(),
                    &self.script_path,
                    source,
                    span.start,
                    span.end,
                ),
            LoaderError::CompilerError(CompilerError { message, span }) => self
                .format_error_with_excerpt(
                    &message,
                    &self.script_path,
                    source,
                    span.start,
                    span.end,
                ),
            LoaderError::IoError(message) => message,
        }
    }

    fn format_error_with_excerpt(
        &self,
        message: &str,
        source_path: &Option<PathBuf>,
        source: &str,
        start_pos: Position,
        end_pos: Position,
    ) -> String {
        if self.settings.repl_mode {
            // Don't show source excerpt in the repl
            return message.to_string();
        }

        let (excerpt, padding) = {
            let excerpt_lines = source
                .lines()
                .skip((start_pos.line - 1) as usize)
                .take((end_pos.line - start_pos.line + 1) as usize)
                .collect::<Vec<_>>();

            let line_numbers = (start_pos.line..=end_pos.line)
                .map(|n| n.to_string())
                .collect::<Vec<_>>();

            let number_width = line_numbers.iter().max_by_key(|n| n.len()).unwrap().len();

            let padding = " ".repeat(number_width + 2);

            if start_pos.line == end_pos.line {
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

        let position_info = if let Some(path) = source_path {
            format!(
                "{} - {}:{}",
                path.display(),
                start_pos.line,
                start_pos.column
            )
        } else {
            format!("{}:{}", start_pos.line, start_pos.column)
        };

        format!(
            "{message}\n --> {}\n{padding}|\n{excerpt}",
            position_info,
            padding = padding,
            excerpt = excerpt,
            message = message
        )
    }
}
