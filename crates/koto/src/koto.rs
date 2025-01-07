use crate::{prelude::*, Error, Ptr, Result};
use koto_bytecode::CompilerSettings;
use koto_runtime::ModuleImportedCallback;
use std::time::Duration;

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
            runtime: KotoVm::with_settings(settings.vm_settings),
            run_tests: settings.run_tests,
            chunk: None,
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
    /// If successful, the compiled chunk is cached for subsequent calls to [Koto::run].
    ///
    /// Compilation arguments are provided via [`CompileArgs`].
    /// `Into<CompileArgs>` is implemented for `&str` for convenience when
    /// default settings are appropriate, e.g. `koto.compile("1 + 1")`.
    pub fn compile<'a>(&mut self, args: impl Into<CompileArgs<'a>>) -> Result<Ptr<Chunk>> {
        let args = args.into();
        let chunk = self
            .runtime
            .loader()
            .borrow_mut()
            .compile_script(args.script, args.script_path, args.compiler_settings)
            .map_err(Error::from)?;

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

    /// Compiles and runs a Koto script, and returns the script's result
    ///
    /// This is equivalent to calling [compile](Self::compile) followed by [run](Self::run).
    ///
    /// Compilation arguments are provided via [`CompileArgs`].
    /// `Into<CompileArgs>` is implemented for `&str` for convenience when
    /// default settings are appropriate, e.g. `koto.compile_and_run("1 + 1")`.
    pub fn compile_and_run<'a>(&mut self, script: impl Into<CompileArgs<'a>>) -> Result<KValue> {
        self.compile(script)?;
        self.run()
    }

    /// Calls a function with the given arguments
    ///
    /// If the provided value isn't [callable](KValue::is_callable) then an error will be returned.
    pub fn call_function<'a>(
        &mut self,
        function: KValue,
        args: impl Into<CallArgs<'a>>,
    ) -> Result<KValue> {
        self.runtime
            .call_function(function, args)
            .map_err(From::from)
    }

    /// Calls an instance function with the given arguments
    ///
    /// If the provided value isn't [callable](KValue::is_callable) then an error will be returned.
    pub fn call_instance_function<'a>(
        &mut self,
        instance: KValue,
        function: KValue,
        args: impl Into<CallArgs<'a>>,
    ) -> Result<KValue> {
        self.runtime
            .call_instance_function(instance, function, args)
            .map_err(From::from)
    }

    /// Converts a [KValue] into a [String] by evaluating `@display` in the runtime
    pub fn value_to_string(&mut self, value: KValue) -> Result<String> {
        self.runtime.value_to_string(&value).map_err(From::from)
    }

    /// Clears the loader's cached modules
    ///
    /// This is useful when a script's dependencies may have changed and need to be recompiled.
    pub fn clear_module_cache(&mut self) {
        self.runtime.loader().borrow_mut().clear_cache();
    }

    /// Sets the arguments that can be accessed from within the script via `koto.args()`
    pub fn set_args(&mut self, args: impl IntoIterator<Item = String>) -> Result<()> {
        let koto_args = args.into_iter().map(KValue::from).collect::<Vec<_>>();

        match self.runtime.prelude().data_mut().get("koto") {
            Some(KValue::Map(map)) => {
                map.insert("args", KValue::Tuple(koto_args.into()));
                Ok(())
            }
            _ => Err(Error::MissingPrelude),
        }
    }

    /// Enables or disables the `run_tests` setting
    ///
    /// Currently this is only used when running benchmarks where tests are run once during setup,
    /// and then disabled for repeated runs.
    pub fn set_run_tests(&mut self, enabled: bool) {
        self.run_tests = enabled;
    }

    fn run_chunk(&mut self, chunk: Ptr<Chunk>) -> Result<KValue> {
        let result = self.runtime.run(chunk)?;

        if self.run_tests {
            self.runtime.run_tests(self.runtime.exports().clone())?;
        }

        let maybe_main = self.runtime.exports().get_meta_value(&MetaKey::Main);
        if let Some(main) = maybe_main {
            self.runtime.call_function(main, &[]).map_err(From::from)
        } else {
            Ok(result)
        }
    }
}

/// Settings used to control the behaviour of the [Koto] runtime
pub struct KotoSettings {
    /// Whether or not tests should be run when loading a script
    pub run_tests: bool,
    /// Settings that apply to the runtime
    pub vm_settings: KotoVmSettings,
}

impl KotoSettings {
    /// Helper for conveniently defining a maximum execution duration
    #[must_use]
    pub fn with_execution_limit(self, limit: Duration) -> Self {
        Self {
            vm_settings: KotoVmSettings {
                execution_limit: Some(limit),
                ..self.vm_settings
            },
            ..self
        }
    }

    /// Helper for conveniently defining a custom stdin implementation
    #[must_use]
    pub fn with_stdin(self, stdin: impl KotoFile + 'static) -> Self {
        Self {
            vm_settings: KotoVmSettings {
                stdin: make_ptr!(stdin),
                ..self.vm_settings
            },
            ..self
        }
    }

    /// Helper for conveniently defining a custom stdout implementation
    #[must_use]
    pub fn with_stdout(self, stdout: impl KotoFile + 'static) -> Self {
        Self {
            vm_settings: KotoVmSettings {
                stdout: make_ptr!(stdout),
                ..self.vm_settings
            },
            ..self
        }
    }

    /// Helper for conveniently defining a custom stderr implementation
    #[must_use]
    pub fn with_stderr(self, stderr: impl KotoFile + 'static) -> Self {
        Self {
            vm_settings: KotoVmSettings {
                stderr: make_ptr!(stderr),
                ..self.vm_settings
            },
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
            vm_settings: KotoVmSettings {
                module_imported_callback: Some(Box::new(callback)),
                ..self.vm_settings
            },
            ..self
        }
    }
}

impl Default for KotoSettings {
    fn default() -> Self {
        Self {
            run_tests: true,
            vm_settings: KotoVmSettings::default(),
        }
    }
}

/// Arguments for [Koto::compile]
pub struct CompileArgs<'a> {
    /// The script to compile
    pub script: &'a str,
    /// The optional path of the script
    ///
    /// The path provided here becomes accessible within the script via
    /// `koto.script_path`/`koto.script_dir`.
    pub script_path: Option<KString>,
    /// Settings used during compilation
    pub compiler_settings: CompilerSettings,
}

impl<'a> CompileArgs<'a> {
    /// Initializes CompileArgs with the given script and default settings
    pub fn new(script: &'a str) -> Self {
        Self {
            script,
            script_path: None,
            compiler_settings: CompilerSettings::default(),
        }
    }

    /// Sets the script's path
    pub fn script_path(mut self, script_path: impl Into<KString>) -> Self {
        self.script_path = Some(script_path.into());
        self
    }

    /// Sets the [`CompilerSettings::enable_type_checks`] flag, enabled by default.
    pub fn enable_type_checks(mut self, enabled: bool) -> Self {
        self.compiler_settings.enable_type_checks = enabled;
        self
    }

    /// Sets the [`CompilerSettings::export_top_level_ids`] flag, disabled by default.
    pub fn export_top_level_ids(mut self, enabled: bool) -> Self {
        self.compiler_settings.export_top_level_ids = enabled;
        self
    }
}

impl<'a> From<&'a str> for CompileArgs<'a> {
    fn from(script: &'a str) -> Self {
        Self {
            script,
            script_path: None,
            compiler_settings: Default::default(),
        }
    }
}

impl<'a> From<&'a String> for CompileArgs<'a> {
    fn from(script: &'a String) -> Self {
        Self {
            script: script.as_str(),
            script_path: None,
            compiler_settings: Default::default(),
        }
    }
}
