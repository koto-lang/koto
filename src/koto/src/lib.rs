pub use koto_bytecode::{
    bytecode_to_string, bytecode_to_string_annotated, Compiler, InstructionReader,
};
pub use koto_parser::{num4::Num4, Ast, Function, Parser2 as Parser, Position};
use koto_runtime::Vm;
pub use koto_runtime::{
    external_error, make_external_value, type_as_string, DebugInfo, Error, ExternalValue,
    RuntimeFunction, RuntimeResult, Value, ValueHashMap, ValueList, ValueMap, ValueVec,
};
pub use koto_std::{get_external_instance, visit_external_value};
use std::{path::Path, sync::Arc};

#[derive(Copy, Clone, Default)]
pub struct Options {
    pub show_annotated: bool,
    pub show_bytecode: bool,
    pub export_all_at_top_level: bool,
}

#[derive(Default)]
pub struct Koto {
    script: String,
    script_path: Option<String>,
    compiler: Compiler,
    ast: Ast,
    runtime: Vm,
    options: Options,
}

impl Koto {
    pub fn new() -> Self {
        let mut result = Self::default();

        koto_std::register(&mut result.runtime);

        let mut env = ValueMap::new();
        env.add_value("script_dir", Value::Empty);
        env.add_value("script_path", Value::Empty);
        env.add_list("args", ValueList::new());
        result.runtime.global_mut().add_map("env", env);

        result
    }

    pub fn with_options(options: Options) -> Self {
        let mut result = Self::new();
        result.options = options;
        result
    }

    pub fn run_script(&mut self, script: &str) -> Result<Value, String> {
        self.run_script_with_args(script, &[])
    }

    pub fn run_script_with_args(&mut self, script: &str, args: &[String]) -> Result<Value, String> {
        self.compile(script)?;
        self.set_args(args);
        self.run()
    }

    pub fn run(&mut self) -> Result<Value, String> {
        let result = match self.runtime.run() {
            Ok(result) => Ok(result),
            Err(e) => Err(match &e {
                Error::RuntimeError {
                    message,
                    start_pos,
                    end_pos,
                } => self.format_error("Runtime", message, &self.script, start_pos, end_pos),
                Error::VmRuntimeError {
                    message,
                    instruction,
                } => self.format_vm_error(message, *instruction),
                Error::ExternalError { message } => format!("Error: {}\n", message),
            }),
        }?;

        if let Some(main) = self.get_global_function("main") {
            self.call_function(&main, &[])
        } else {
            Ok(result)
        }
    }

    pub fn compile(&mut self, script: &str) -> Result<(), String> {
        let options = koto_parser::Options {
            export_all_top_level: self.options.export_all_at_top_level,
        };

        match Parser::parse(&script, self.runtime.constants_mut(), options) {
            Ok(ast) => {
                self.ast = ast;
                self.runtime.constants_mut().shrink_to_fit();
            }
            Err(e) => {
                return Err(self.format_error(
                    "Parser",
                    &e.to_string(),
                    script,
                    &e.span.start,
                    &e.span.end,
                ));
            }
        }

        match self.compiler.compile_ast(&self.ast) {
            Ok((bytecode, debug_info)) => {
                self.runtime.set_bytecode(bytecode);
                self.runtime.set_debug_info(Arc::new(DebugInfo {
                    source_map: debug_info.clone(),
                    script_path: self.script_path.clone(),
                }));

                self.script = script.to_string();

                if self.options.show_annotated {
                    let script_lines = self.script.lines().collect::<Vec<_>>();
                    println!(
                        "{}",
                        bytecode_to_string_annotated(
                            self.runtime.bytecode(),
                            &script_lines,
                            self.compiler.debug_info()
                        )
                    );
                } else if self.options.show_bytecode {
                    println!("{}", bytecode_to_string(self.runtime.bytecode()));
                }

                Ok(())
            }
            Err(e) => Err(format!("Error while compiling script: {}", e)),
        }
    }

    pub fn global_mut(&mut self) -> &mut ValueMap {
        self.runtime.global_mut()
    }

    pub fn set_args(&mut self, args: &[String]) {
        use Value::{Map, Str};

        let koto_args = args
            .iter()
            .map(|arg| Str(Arc::new(arg.to_string())))
            .collect::<ValueVec>();

        match self.runtime.global_mut().data_mut().get_mut("env").unwrap() {
            Map(map) => map
                .data_mut()
                .add_list("args", ValueList::with_data(koto_args)),
            _ => unreachable!(),
        }
    }

    pub fn set_script_path(&mut self, path: Option<String>) {
        use Value::{Empty, Map, Str};

        let (script_dir, script_path) = match &path {
            Some(path) => (
                Path::new(&path)
                    .parent()
                    .map(|p| {
                        Str(Arc::new(
                            p.to_str().expect("invalid script path").to_string(),
                        ))
                    })
                    .or(Some(Empty))
                    .unwrap(),
                Str(Arc::new(path.to_string())),
            ),
            None => (Empty, Empty),
        };

        self.script_path = path;

        match self.runtime.global_mut().data_mut().get_mut("env").unwrap() {
            Map(map) => {
                let mut map = map.data_mut();
                map.add_value("script_dir", script_dir);
                map.add_value("script_path", script_path);
            }
            _ => unreachable!(),
        }
    }

    pub fn get_global_function(&self, id: &str) -> Option<RuntimeFunction> {
        match self.runtime.get_global_value(id) {
            Some(Value::Function(function)) => Some(function),
            _ => None,
        }
    }

    pub fn call_function_by_name(
        &mut self,
        function_name: &str,
        args: &[Value],
    ) -> Result<Value, String> {
        match self.get_global_function(function_name) {
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
            Err(e) => Err(match &e {
                Error::RuntimeError {
                    message,
                    start_pos,
                    end_pos,
                } => self.format_error("Runtime", &message, &self.script, start_pos, end_pos),
                Error::VmRuntimeError {
                    message,
                    instruction,
                } => self.format_vm_error(message, *instruction),
                Error::ExternalError { message } => format!("Error: {}\n", message,),
            }),
        }
    }

    fn format_vm_error(&self, message: &str, instruction: usize) -> String {
        match self.compiler.debug_info().get_source_span(instruction) {
            Some(span) => {
                self.format_error("Runtime", message, &self.script, &span.start, &span.end)
            }
            None => format!(
                "Runtime error at instruction {}: {}\n",
                instruction, message
            ),
        }
    }

    fn format_error(
        &self,
        error_type: &str,
        message: &str,
        script: &str,
        start_pos: &Position,
        end_pos: &Position,
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

            let padding = format!("{}", " ".repeat(number_width + 2));

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
            "{} error: {message}\n --> {}:{}\n{padding}|\n{excerpt}",
            error_type,
            start_pos.line,
            start_pos.column,
            padding = padding,
            excerpt = excerpt,
            message = message
        )
    }
}
