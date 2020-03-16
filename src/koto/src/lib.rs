pub use koto_parser::{Ast, AstNode, KotoParser as Parser};
use koto_runtime::{runtime_trace, Runtime};
pub use koto_runtime::{Error, RuntimeResult, Value, ValueList, ValueMap};
use std::{path::Path, rc::Rc};

#[derive(Default)]
pub struct Environment {
    pub script_path: Option<String>,
    pub args: Vec<String>,
}

#[derive(Default)]
pub struct Koto<'a> {
    environment: Environment,
    runtime: Runtime<'a>,
}

impl<'a> Koto<'a> {
    pub fn new() -> Self {
        let mut result = Self::default();

        koto_std::register(&mut result.runtime);

        result
    }

    pub fn environment_mut(&mut self) -> &mut Environment {
        &mut self.environment
    }

    pub fn setup_environment(&mut self) {
        use Value::{Empty, Str};

        let (script_dir, script_path) = match &self.environment.script_path {
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

        let args =
            self.environment.args.iter()
            .map(|arg| Str(Rc::new(arg.to_string()))).collect::<Vec<_>>();

        let mut env = ValueMap::new();

        env.add_value("script_dir", script_dir);
        env.add_value("script_path", script_path);
        env.add_list("args", ValueList::with_data(args));

        self.runtime.global_mut().add_map("env", env);
    }

    /// Run a script and capture the final value
    pub fn run(&mut self, ast: &[AstNode]) -> RuntimeResult<'a> {
        runtime_trace!(self, "run");

        self.runtime.evaluate_block(ast)
    }
}
