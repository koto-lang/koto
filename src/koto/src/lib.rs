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
    dunce::canonicalize,
    koto_bytecode::{Chunk, LoaderError},
    koto_runtime::{
        CallArgs, KotoFile, Loader, MetaKey, RuntimeError, Value, ValueMap, Vm, VmSettings,
    },
    std::{error::Error, fmt, path::PathBuf, rc::Rc},
};

#[derive(Debug)]
pub enum KotoError {
    CompileError(LoaderError),
    RuntimeError(RuntimeError),
    NothingToRun,
    InvalidTestsType(String),
    FunctionNotFound(String),
}

impl KotoError {
    pub fn is_indentation_error(&self) -> bool {
        match &self {
            Self::CompileError(e) => e.is_indentation_error(),
            _ => false,
        }
    }
}

impl fmt::Display for KotoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use KotoError::*;

        match &self {
            CompileError(e) => e.fmt(f),
            RuntimeError(e) => e.fmt(f),
            NothingToRun => {
                f.write_str("Missing compiled chunk, call compile() before calling run()")
            }
            InvalidTestsType(t) => {
                write!(f, "Expected a Map for the exported 'tests', found '{}'", t)
            }
            FunctionNotFound(name) => {
                write!(f, "Function '{}' not found", name)
            }
        }
    }
}

impl Error for KotoError {}

impl From<RuntimeError> for KotoError {
    fn from(error: RuntimeError) -> Self {
        Self::RuntimeError(error)
    }
}

pub type KotoResult = Result<Value, KotoError>;

/// Settings used to control the behaviour of the [Koto] runtime
#[derive(Clone)]
pub struct KotoSettings {
    pub run_tests: bool,
    pub run_import_tests: bool,
    pub repl_mode: bool,
    pub stdin: Rc<dyn KotoFile>,
    pub stdout: Rc<dyn KotoFile>,
    pub stderr: Rc<dyn KotoFile>,
}

impl Default for KotoSettings {
    fn default() -> Self {
        let default_vm_settings = VmSettings::default();
        Self {
            run_tests: true,
            run_import_tests: true,
            repl_mode: false,
            stdin: default_vm_settings.stdin,
            stdout: default_vm_settings.stdout,
            stderr: default_vm_settings.stderr,
        }
    }
}

/// The main interface for the Koto language.
///
/// Example
pub struct Koto {
    runtime: Vm,
    pub settings: KotoSettings, // TODO make private, needs enable / disable tests methods
    script_path: Option<PathBuf>,
    loader: Loader,
    chunk: Option<Rc<Chunk>>,
}

impl Default for Koto {
    fn default() -> Self {
        Self::new()
    }
}

impl Koto {
    pub fn new() -> Self {
        Self::with_settings(KotoSettings::default())
    }

    pub fn with_settings(settings: KotoSettings) -> Self {
        Self {
            settings: settings.clone(),
            runtime: Vm::with_settings(VmSettings {
                stdin: settings.stdin,
                stdout: settings.stdout,
                stderr: settings.stderr,
                run_import_tests: settings.run_import_tests,
            }),
            loader: Loader::default(),
            chunk: None,
            script_path: None,
        }
    }

    pub fn compile(&mut self, script: &str) -> Result<Rc<Chunk>, KotoError> {
        let compile_result = if self.settings.repl_mode {
            self.loader.compile_repl(script)
        } else {
            self.loader.compile_script(script, &self.script_path)
        };

        match compile_result {
            Ok(chunk) => {
                self.chunk = Some(chunk.clone());
                Ok(chunk)
            }
            Err(error) => Err(KotoError::CompileError(error)),
        }
    }

    pub fn run_with_args(&mut self, args: &[String]) -> KotoResult {
        self.set_args(args);
        self.run()
    }

    pub fn run(&mut self) -> KotoResult {
        let chunk = self.chunk.clone();
        match chunk {
            Some(chunk) => self.run_chunk(chunk),
            None => Err(KotoError::NothingToRun),
        }
    }

    pub fn run_chunk(&mut self, chunk: Rc<Chunk>) -> KotoResult {
        let result = self.runtime.run(chunk)?;

        if self.settings.repl_mode {
            Ok(result)
        } else {
            if self.settings.run_tests {
                let maybe_tests = self
                    .runtime
                    .context()
                    .exports
                    .meta()
                    .get(&MetaKey::Tests)
                    .cloned();
                match maybe_tests {
                    Some(Value::Map(tests)) => {
                        self.runtime.run_tests(tests)?;
                    }
                    Some(other) => {
                        return Err(KotoError::InvalidTestsType(other.type_as_string()));
                    }
                    None => {}
                }
            }

            let maybe_main = self
                .runtime
                .context()
                .exports
                .meta()
                .get(&MetaKey::Main)
                .cloned();
            if let Some(main) = maybe_main {
                self.runtime
                    .run_function(main, CallArgs::None)
                    .map_err(|e| e.into())
            } else {
                Ok(result)
            }
        }
    }

    pub fn prelude(&self) -> ValueMap {
        self.runtime.prelude()
    }

    pub fn exports(&self) -> ValueMap {
        self.runtime.context().exports.clone()
    }

    pub fn set_args(&mut self, args: &[String]) {
        use Value::{Map, Str, Tuple};

        let koto_args = args
            .iter()
            .map(|arg| Str(arg.as_str().into()))
            .collect::<Vec<_>>();

        match self
            .runtime
            .prelude()
            .data_mut()
            .get_with_string_mut("koto")
            .unwrap()
        {
            Map(map) => {
                map.data_mut().add_value("args", Tuple(koto_args.into()));
            }
            _ => unreachable!(),
        }
    }

    pub fn set_script_path(&mut self, path: Option<PathBuf>) {
        use Value::{Empty, Map, Str};

        let (script_dir, script_path) = match &path {
            Some(path) => {
                let path = canonicalize(path).expect("Invalid script path");

                let script_dir = path
                    .parent()
                    .map(|p| {
                        let s = p.to_string_lossy();
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
                let map = &mut map.data_mut();
                map.add_value("script_dir", script_dir);
                map.add_value("script_path", script_path);
            }
            _ => unreachable!(),
        }
    }

    pub fn run_function_by_name(&mut self, function_name: &str, args: CallArgs) -> KotoResult {
        match self.runtime.get_exported_function(function_name) {
            Some(f) => self.run_function(f, args),
            None => Err(KotoError::FunctionNotFound(function_name.into())),
        }
    }

    pub fn run_function(&mut self, function: Value, args: CallArgs) -> KotoResult {
        self.runtime
            .run_function(function, args)
            .map_err(|e| e.into())
    }
}
