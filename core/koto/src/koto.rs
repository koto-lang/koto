use crate::prelude::*;
use dunce::canonicalize;
use koto_bytecode::CompilerSettings;
use koto_runtime::ModuleImportedCallback;
use std::{error::Error, fmt, path::PathBuf, rc::Rc};

/// The error type returned by [Koto] operations
#[allow(missing_docs)]
#[derive(Debug)]
pub enum KotoError {
    CompileError(LoaderError),
    RuntimeError(RuntimeError),
    NothingToRun,
    InvalidScriptPath(PathBuf),
    MissingKotoModuleInPrelude,
    InvalidTestsType(String),
    FunctionNotFound(String),
}

impl KotoError {
    /// Returns true if the error is a complier 'expected indentation' error
    ///
    /// This is useful in the REPL, where an indentation error signals that the expression should be
    /// continued on an indented line.
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
            InvalidScriptPath(path) => {
                write!(f, "The path '{}' couldn't be found", path.to_string_lossy())
            }
            MissingKotoModuleInPrelude => {
                f.write_str("The koto module wasn't found in the runtime's prelude")
            }
            InvalidTestsType(t) => {
                write!(f, "Expected a Map for the exported 'tests', found '{t}'")
            }
            FunctionNotFound(name) => {
                write!(f, "Function '{name}' not found")
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
pub struct KotoSettings {
    /// Whether or not tests should be run when loading a script
    pub run_tests: bool,
    /// Whether or not tests should be run when importing modules
    pub run_import_tests: bool,
    /// Whether or not top-level identifiers should be automatically exported
    ///
    /// The default behaviour in Koto is that `export` expressions are required to make a value
    /// available outside of the current module.
    ///
    /// This is used by the REPL, allowing for incremental compilation and execution of expressions
    /// that need to share declared values.
    pub export_top_level_ids: bool,
    /// The runtime's stdin
    pub stdin: Rc<dyn KotoFile>,
    /// The runtime's stdout
    pub stdout: Rc<dyn KotoFile>,
    /// The runtime's stderr
    pub stderr: Rc<dyn KotoFile>,
    /// An optional callback that is called whenever a module is imported by the runtime
    ///
    /// This allows you to track the runtime's dependencies, which might be useful if you want to
    /// reload the script when one of its dependencies has changed.
    pub module_imported_callback: Option<Box<dyn ModuleImportedCallback>>,
}

impl KotoSettings {
    /// Helper for conveniently defining a custom stdin implementation
    #[must_use]
    pub fn with_stdin(self, stdin: impl KotoFile + 'static) -> Self {
        Self {
            stdin: Rc::new(stdin),
            ..self
        }
    }

    /// Helper for conveniently defining a custom stdout implementation
    #[must_use]
    pub fn with_stdout(self, stdout: impl KotoFile + 'static) -> Self {
        Self {
            stdout: Rc::new(stdout),
            ..self
        }
    }

    /// Helper for conveniently defining a custom stderr implementation
    #[must_use]
    pub fn with_stderr(self, stderr: impl KotoFile + 'static) -> Self {
        Self {
            stderr: Rc::new(stderr),
            ..self
        }
    }

    /// Convenience function for declaring the 'module imported' callback
    #[must_use]
    pub fn with_module_imported_callback(
        self,
        callback: impl ModuleImportedCallback + 'static,
    ) -> Self {
        Self {
            module_imported_callback: Some(Box::new(callback)),
            ..self
        }
    }
}

impl Default for KotoSettings {
    fn default() -> Self {
        let default_vm_settings = VmSettings::default();
        Self {
            run_tests: true,
            run_import_tests: true,
            export_top_level_ids: false,
            stdin: default_vm_settings.stdin,
            stdout: default_vm_settings.stdout,
            stderr: default_vm_settings.stderr,
            module_imported_callback: None,
        }
    }
}

/// The main interface for the Koto language.
pub struct Koto {
    runtime: Vm,
    run_tests: bool,
    export_top_level_ids: bool,
    script_path: Option<PathBuf>,
    chunk: Option<Ptr<Chunk>>,
}

impl Default for Koto {
    fn default() -> Self {
        Self::new()
    }
}

impl Koto {
    /// Initializes Koto with the default settings
    pub fn new() -> Self {
        Self::with_settings(KotoSettings::default())
    }

    /// Initializes Koto with the provided settings
    pub fn with_settings(settings: KotoSettings) -> Self {
        Self {
            runtime: Vm::with_settings(VmSettings {
                stdin: settings.stdin,
                stdout: settings.stdout,
                stderr: settings.stderr,
                run_import_tests: settings.run_import_tests,
                module_imported_callback: settings.module_imported_callback,
            }),
            run_tests: settings.run_tests,
            export_top_level_ids: settings.export_top_level_ids,
            chunk: None,
            script_path: None,
        }
    }

    /// Compiles a Koto script, returning the complied chunk if successful
    ///
    /// On success, the chunk is cached as the current chunk for subsequent calls to [Koto::run].
    pub fn compile(&mut self, script: &str) -> Result<Ptr<Chunk>, KotoError> {
        let result = self.runtime.loader().borrow_mut().compile_script(
            script,
            &self.script_path,
            CompilerSettings {
                export_top_level_ids: self.export_top_level_ids,
            },
        );

        match result {
            Ok(chunk) => {
                self.chunk = Some(chunk.clone());
                Ok(chunk)
            }
            Err(error) => Err(KotoError::CompileError(error)),
        }
    }

    /// Clears the loader's cached modules
    pub fn clear_module_cache(&mut self) {
        self.runtime.loader().borrow_mut().clear_cache();
    }

    /// A helper for calling [set_args](Koto::set_args) followed by [run](Koto::run).
    pub fn run_with_args(&mut self, args: &[String]) -> KotoResult {
        self.set_args(args)?;
        self.run()
    }

    /// Runs the chunk last compiled with [compile](Koto::compile)
    pub fn run(&mut self) -> KotoResult {
        let chunk = self.chunk.clone();
        match chunk {
            Some(chunk) => self.run_chunk(chunk),
            None => Err(KotoError::NothingToRun),
        }
    }

    /// Enables or disables the `run_tests` setting
    ///
    /// Currently this is only used when running benchmarks where tests are run once during setup,
    /// and then disabled for repeated runs.
    pub fn set_run_tests(&mut self, enabled: bool) {
        self.run_tests = enabled;
    }

    fn run_chunk(&mut self, chunk: Ptr<Chunk>) -> KotoResult {
        let result = self.runtime.run(chunk)?;

        if self.run_tests {
            let maybe_tests = self.runtime.exports().get_meta_value(&MetaKey::Tests);
            match maybe_tests {
                Some(Value::Map(tests)) => {
                    self.runtime.run_tests(tests)?;
                }
                Some(other) => {
                    return Err(KotoError::InvalidTestsType(
                        other.type_as_string().to_string(),
                    ));
                }
                None => {}
            }
        }

        let maybe_main = self.runtime.exports().get_meta_value(&MetaKey::Main);
        if let Some(main) = maybe_main {
            self.runtime
                .run_function(main, CallArgs::None)
                .map_err(|e| e.into())
        } else {
            Ok(result)
        }
    }

    /// Runs a function in the runtime's exports map by name
    pub fn run_function_by_name(&mut self, function_name: &str, args: CallArgs) -> KotoResult {
        match self.runtime.get_exported_function(function_name) {
            Some(f) => self.run_function(f, args),
            None => Err(KotoError::FunctionNotFound(function_name.into())),
        }
    }

    /// Runs a function in the runtime's exports map by name
    pub fn run_function(&mut self, function: Value, args: CallArgs) -> KotoResult {
        self.runtime
            .run_function(function, args)
            .map_err(|e| e.into())
    }

    /// Converts a [Value] into a [Value::Str] by evaluating `@display` in the runtime
    pub fn value_to_string(&mut self, value: Value) -> Result<String, KotoError> {
        self.runtime.value_to_string(&value).map_err(|e| e.into())
    }

    /// Returns a reference to the runtime's prelude
    pub fn prelude(&self) -> &ValueMap {
        self.runtime.prelude()
    }

    /// Returns a reference to the runtime's exports
    pub fn exports(&self) -> &ValueMap {
        self.runtime.exports()
    }

    /// Sets the arguments for the script, accessible via `koto.args()`
    pub fn set_args(&mut self, args: &[String]) -> Result<(), KotoError> {
        use Value::{Map, Str, Tuple};

        let koto_args = args
            .iter()
            .map(|arg| Str(arg.as_str().into()))
            .collect::<Vec<_>>();

        match self.runtime.prelude().data_mut().get("koto") {
            Some(Map(map)) => {
                map.add_value("args", Tuple(koto_args.into()));
                Ok(())
            }
            _ => Err(KotoError::MissingKotoModuleInPrelude),
        }
    }

    /// Sets the path of the current script, accessible via `koto.script_dir` / `koto.script_path`
    pub fn set_script_path(&mut self, path: Option<PathBuf>) -> Result<(), KotoError> {
        use Value::{Map, Null, Str};

        let (script_dir, script_path) = match &path {
            Some(path) => {
                let path = canonicalize(path)
                    .map_err(|_| KotoError::InvalidScriptPath(path.to_owned()))?;

                let script_dir = path
                    .parent()
                    .map(|p| {
                        let s = p.to_string_lossy();
                        Str(s.into_owned().into())
                    })
                    .unwrap_or(Null);
                let script_path = Str(path.display().to_string().into());

                (script_dir, script_path)
            }
            None => (Null, Null),
        };

        self.script_path = path;

        match self.runtime.prelude().data_mut().get("koto") {
            Some(Map(map)) => {
                map.add_value("script_dir", script_dir);
                map.add_value("script_path", script_path);
                Ok(())
            }
            _ => Err(KotoError::MissingKotoModuleInPrelude),
        }
    }
}
