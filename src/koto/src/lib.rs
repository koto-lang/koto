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
    koto_bytecode::{Chunk, LoaderError},
    koto_runtime::{
        DefaultLogger, KotoLogger, Loader, RuntimeError, Value, ValueList, ValueMap, ValueVec, Vm,
        VmSettings,
    },
    std::{error::Error, fmt, path::PathBuf, sync::Arc},
};

#[derive(Debug)]
pub enum KotoError {
    RuntimeError(RuntimeError),
    NothingToRun,
    InvalidTestsType(String),
    FunctionNotFound(String),
}

impl fmt::Display for KotoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use KotoError::*;

        match &self {
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
    pub repl_mode: bool,
    pub logger: Arc<dyn KotoLogger>,
}

impl Default for KotoSettings {
    fn default() -> Self {
        Self {
            run_tests: true,
            repl_mode: false,
            logger: Arc::new(DefaultLogger {}),
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
    chunk: Option<Arc<Chunk>>,
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
                logger: settings.logger,
            }),
            loader: Loader::default(),
            chunk: None,
            script_path: None,
        }
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
                Ok(chunk)
            }
            Err(error) => Err(error),
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

    pub fn run_chunk(&mut self, chunk: Arc<Chunk>) -> KotoResult {
        let result = self.runtime.run(chunk)?;

        if self.settings.repl_mode {
            Ok(result)
        } else {
            if self.settings.run_tests {
                let _test_result = match self.runtime.get_global_value("tests") {
                    Some(Value::Map(tests)) => {
                        self.runtime.run_tests(tests)?;
                    }
                    Some(other) => return Err(KotoError::InvalidTestsType(other.type_as_string())),
                    None => {}
                };
            }

            if let Some(main) = self.runtime.get_global_function("main") {
                self.runtime.run_function(main, &[]).map_err(|e| e.into())
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
            .contents_mut()
            .data
            .get_with_string_mut("koto")
            .unwrap()
        {
            Map(map) => map
                .contents_mut()
                .data
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
            .contents_mut()
            .data
            .get_with_string_mut("koto")
            .unwrap()
        {
            Map(map) => {
                let map = &mut map.contents_mut().data;
                map.add_value("script_dir", script_dir);
                map.add_value("script_path", script_path);
            }
            _ => unreachable!(),
        }
    }

    pub fn call_function_by_name(&mut self, function_name: &str, args: &[Value]) -> KotoResult {
        match self.runtime.get_global_function(function_name) {
            Some(f) => self.call_function(f, args),
            None => Err(KotoError::FunctionNotFound(function_name.into())),
        }
    }

    pub fn call_function(&mut self, function: Value, args: &[Value]) -> KotoResult {
        self.runtime
            .run_function(function, args)
            .map_err(|e| e.into())
    }
}
