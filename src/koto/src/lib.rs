pub use koto_parser::{Ast, AstNode, KotoParser as Parser, LookupOrId};
use koto_runtime::Runtime;
pub use koto_runtime::{Error, RuntimeResult, Value, ValueList, ValueMap};
use std::{path::Path, rc::Rc};

#[derive(Default)]
pub struct Koto<'a> {
    script: String,
    parser: Parser,
    ast: Vec<AstNode>,
    runtime: Runtime<'a>,
}

impl<'a> Koto<'a> {
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

    pub fn run_script_with_args(
        &mut self,
        script: &str,
        args: Vec<String>,
    ) -> Result<Value<'a>, String> {
        self.parse(script)?;
        self.set_args(args);
        self.run()?;
        if self.has_function("main") {
            self.call_function("main")
        } else {
            Ok(Value::Empty)
        }
    }

    pub fn parse(&mut self, script: &str) -> Result<(), String> {
        match self.parser.parse(&script) {
            Ok(ast) => {
                self.script = script.to_string();
                self.ast = ast;
                Ok(())
            }
            Err(e) => Err(format!("Error while parsing script: {}", e)),
        }
    }

    pub fn set_args(&mut self, args: Vec<String>) {
        use Value::{Map, Str};

        let koto_args = args
            .iter()
            .map(|arg| Str(Rc::new(arg.to_string())))
            .collect::<Vec<_>>();

        match self
            .runtime
            .global_mut()
            .0
            .get_mut(&Rc::new("env".to_string()))
            .unwrap()
        {
            Map(map) => Rc::make_mut(map).add_list("args", ValueList::with_data(koto_args)),
            _ => unreachable!(),
        }
    }

    pub fn set_script_path(&mut self, path: Option<String>) {
        use Value::{Empty, Map, Str};

        let (script_dir, script_path) = match path {
            Some(path) => (
                Path::new(&path)
                    .parent()
                    .map(|p| {
                        Str(Rc::new(
                            p.to_str().expect("invalid script path").to_string(),
                        ))
                    })
                    .or(Some(Empty))
                    .unwrap(),
                Str(Rc::new(path.to_string())),
            ),
            None => (Empty, Empty),
        };

        match self
            .runtime
            .global_mut()
            .0
            .get_mut(&Rc::new("env".to_string())) // TODO no rc
            .unwrap()
        {
            Map(map) => {
                let map = Rc::make_mut(map);
                map.add_value("script_dir", script_dir);
                map.add_value("script_path", script_path);
            }
            _ => unreachable!(),
        }
    }

    pub fn run(&mut self) -> Result<Value<'a>, String> {
        match self.runtime.evaluate_block(&self.ast) {
            Ok(result) => Ok(result),
            Err(e) => Err(match e {
                Error::BuiltinError { message } => format!("Builtin error: {}\n", message,),
                Error::RuntimeError {
                    message,
                    start_pos,
                    end_pos,
                } => {
                    let excerpt = self
                        .script
                        .lines()
                        .skip(start_pos.line - 1)
                        .take(end_pos.line - start_pos.line + 1)
                        .map(|line| format!("  | {}\n", line))
                        .collect::<String>();
                    format!(
                        "Runtime error: {}\n  --> {}:{}\n  |\n{}  |",
                        message, start_pos.line, start_pos.column, excerpt
                    )
                }
            }),
        }
    }

    pub fn has_function(&self, function_name: &str) -> bool {
        // TODO no rc
        matches!(
            self.runtime.get_value(&Rc::new(function_name.to_string())),
            Some((Value::Function(_), _))
        )
    }

    pub fn call_function(&mut self, function_name: &str) -> Result<Value<'a>, String> {
        match self.runtime.lookup_and_call_function(
            &LookupOrId::Id(Rc::new(function_name.to_string())),
            &vec![],
            &AstNode::dummy(),
        ) {
            Ok(result) => Ok(result),
            Err(e) => Err(match e {
                Error::BuiltinError { message } => format!("Builtin error: {}\n", message,),
                Error::RuntimeError {
                    message,
                    start_pos,
                    end_pos,
                } => {
                    let excerpt = self
                        .script
                        .lines()
                        .skip(start_pos.line - 1)
                        .take(end_pos.line - start_pos.line + 1)
                        .map(|line| format!("  | {}\n", line))
                        .collect::<String>();
                    format!(
                        "Runtime error: {}\n  --> {}:{}\n  |\n{}  |",
                        message, start_pos.line, start_pos.column, excerpt
                    )
                }
            }),
        }
    }
}
