use crate::{prelude::*, Error, Ptr, Result};
use dunce::canonicalize;
use koto_bytecode::CompilerSettings;
use koto_runtime::{KotoVm, ModuleImportedCallback};
use std::path::PathBuf;

/// The main interface for the Koto language.
///
/// This provides a high-level API for compiling and executing Koto scripts in a Koto [Vm](KotoVm).
///
/// Example:
///
/// ```
/// use koto::prelude::*;
///
/// fn main() -> koto::Result<()> {
///     let mut koto = Koto::default();
///
///     match koto.compile_and_run("1 + 2")? {
///         KValue::Number(result) => {
///             assert_eq!(result, 3);
///         }
///         other => panic!("Unexpected result: {}", koto.value_to_string(other)?),
///     }
///
///     Ok(())
/// }
/// ```
pub struct Koto {
    runtime: KotoVm,
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
    /// Creates a new instance of Koto with default settings
    pub fn new() -> Self {
        Self::with_settings(KotoSettings::default())
    }

    /// Creates a new instance of Koto with the given settings
    pub fn with_settings(settings: KotoSettings) -> Self {
        Self {
            runtime: KotoVm::with_settings(KotoVmSettings {
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

    /// Returns a reference to the runtime's prelude
    pub fn prelude(&self) -> &KMap {
        self.runtime.prelude()
    }

    /// Returns a reference to the runtime's exports
    pub fn exports(&self) -> &KMap {
        self.runtime.exports()
    }

    /// Returns a reference to the runtime's exports
    pub fn exports_mut(&mut self) -> &mut KMap {
        self.runtime.exports_mut()
    }

    /// Compiles a Koto script, returning the complied chunk if successful
    ///
    /// On success, the chunk is cached as the current chunk for subsequent calls to [Koto::run].
    pub fn compile(&mut self, script: &str) -> Result<Ptr<Chunk>> {
        let chunk = self.runtime.loader().borrow_mut().compile_script(
            script,
            &self.script_path,
            CompilerSettings {
                export_top_level_ids: self.export_top_level_ids,
            },
        )?;

        self.chunk = Some(chunk.clone());
        Ok(chunk)
    }

    /// Runs the chunk last compiled with [compile](Koto::compile)
    pub fn run(&mut self) -> Result<KValue> {
        let chunk = self.chunk.clone();
        match chunk {
            Some(chunk) => self.run_chunk(chunk),
            None => Err(Error::NothingToRun),
        }
    }

    /// A helper for calling [set_args](Koto::set_args) followed by [run](Koto::run).
    pub fn run_with_args(&mut self, args: &[String]) -> Result<KValue> {
        self.set_args(args)?;
        self.run()
    }

    /// Compiles and runs a Koto script, and returns the script's result
    ///
    /// This is equivalent to calling [compile](Self::compile) followed by [run](Self::run).
    pub fn compile_and_run(&mut self, script: &str) -> Result<KValue> {
        self.compile(script)?;
        self.run()
    }

    /// Runs a function with the given arguments
    pub fn run_function(&mut self, function: KValue, args: CallArgs) -> Result<KValue> {
        self.runtime
            .run_function(function, args)
            .map_err(|e| e.into())
    }

    /// Runs an instance function with the given arguments
    pub fn run_instance_function(
        &mut self,
        instance: KValue,
        function: KValue,
        args: CallArgs,
    ) -> Result<KValue> {
        self.runtime
            .run_instance_function(instance, function, args)
            .map_err(|e| e.into())
    }

    /// Runs a function in the runtime's exports map
    ///
    /// ```
    /// use koto::prelude::*;
    ///
    /// fn main() -> koto::Result<()> {
    ///     let mut koto = Koto::default();
    ///
    ///     koto.compile_and_run("export say_hello = |name| 'Hello, $name!'")?;
    ///
    ///     match koto.run_exported_function("say_hello", CallArgs::Single("World".into()))? {
    ///         KValue::Str(result) => assert_eq!(result, "Hello, World!"),
    ///         other => panic!(
    ///             "Unexpected result: {}",
    ///             koto.value_to_string(other)?
    ///         ),
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn run_exported_function(&mut self, function_name: &str, args: CallArgs) -> Result<KValue> {
        match self.runtime.get_exported_function(function_name) {
            Some(f) => self.run_function(f, args),
            None => Err(Error::FunctionNotFound),
        }
    }

    /// Converts a [KValue] into a [String] by evaluating `@display` in the runtime
    pub fn value_to_string(&mut self, value: KValue) -> Result<String> {
        self.runtime.value_to_string(&value).map_err(|e| e.into())
    }

    /// Clears the loader's cached modules
    ///
    /// This is useful when a script's dependencies may have changed and need to be recompiled.
    pub fn clear_module_cache(&mut self) {
        self.runtime.loader().borrow_mut().clear_cache();
    }

    /// Sets the arguments that can be accessed from within the script via `koto.args()`
    pub fn set_args(&mut self, args: &[String]) -> Result<()> {
        use KValue::{Map, Str, Tuple};

        let koto_args = args
            .iter()
            .map(|arg| Str(arg.as_str().into()))
            .collect::<Vec<_>>();

        match self.runtime.prelude().data_mut().get("koto") {
            Some(Map(map)) => {
                map.insert("args", Tuple(koto_args.into()));
                Ok(())
            }
            _ => Err(Error::MissingKotoModuleInPrelude),
        }
    }

    /// Enables or disables the `run_tests` setting
    ///
    /// Currently this is only used when running benchmarks where tests are run once during setup,
    /// and then disabled for repeated runs.
    pub fn set_run_tests(&mut self, enabled: bool) {
        self.run_tests = enabled;
    }

    /// Sets the path of the current script, accessible via `koto.script_dir` / `koto.script_path`
    pub fn set_script_path(&mut self, path: Option<PathBuf>) -> Result<()> {
        use KValue::{Map, Null, Str};

        let (script_dir, script_path) = match &path {
            Some(path) => {
                let path =
                    canonicalize(path).map_err(|_| Error::InvalidScriptPath(path.to_owned()))?;

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
                map.insert("script_dir", script_dir);
                map.insert("script_path", script_path);
                Ok(())
            }
            _ => Err(Error::MissingKotoModuleInPrelude),
        }
    }

    fn run_chunk(&mut self, chunk: Ptr<Chunk>) -> Result<KValue> {
        let result = self.runtime.run(chunk)?;

        if self.run_tests {
            let maybe_tests = self.runtime.exports().get_meta_value(&MetaKey::Tests);
            match maybe_tests {
                Some(KValue::Map(tests)) => {
                    self.runtime.run_tests(tests)?;
                }
                Some(other) => {
                    return Err(Error::InvalidTestsType(other.type_as_string().to_string()));
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
}

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
    pub stdin: Ptr<dyn KotoFile>,
    /// The runtime's stdout
    pub stdout: Ptr<dyn KotoFile>,
    /// The runtime's stderr
    pub stderr: Ptr<dyn KotoFile>,
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
            stdin: make_ptr!(stdin),
            ..self
        }
    }

    /// Helper for conveniently defining a custom stdout implementation
    #[must_use]
    pub fn with_stdout(self, stdout: impl KotoFile + 'static) -> Self {
        Self {
            stdout: make_ptr!(stdout),
            ..self
        }
    }

    /// Helper for conveniently defining a custom stderr implementation
    #[must_use]
    pub fn with_stderr(self, stderr: impl KotoFile + 'static) -> Self {
        Self {
            stderr: make_ptr!(stderr),
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
        let default_vm_settings = KotoVmSettings::default();
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
