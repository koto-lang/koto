use crate::{Error, Ptr, Result, prelude::*};
use koto_bytecode::CompilerSettings;
use koto_runtime::{ModuleImportedCallback, SystemStderr, SystemStdin, SystemStdout};
use std::{future::poll_fn, task::Poll, time::Duration};

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

    /// Returns a mutable reference to the runtime's exports
    pub fn exports_mut(&mut self) -> &mut KMap {
        self.runtime.exports_mut()
    }

    /// Compiles and runs a Koto script, and returns the script's result
    ///
    /// This is a convenience function, equivalent to calling [compile](Self::compile) followed by
    /// [run](Self::run).
    ///
    /// Compilation arguments are provided via [`CompileArgs`].
    /// `Into<CompileArgs>` is implemented for `&str` for convenience when
    /// default settings are appropriate, e.g. `koto.compile_and_run("1 + 1")`.
    pub fn compile_and_run<'a>(&mut self, script: impl Into<CompileArgs<'a>>) -> Result<KValue> {
        let chunk = self.compile(script)?;
        self.run(chunk)
    }

    /// Compiles and runs a Koto script asynchronously, returning the script's result
    ///
    /// This is a convenience function, equivalent to calling [compile](Self::compile) followed by
    /// [run_async](Self::run_async).
    pub async fn compile_and_run_async<'a>(
        &mut self,
        script: impl Into<CompileArgs<'a>>,
    ) -> Result<KValue> {
        let chunk = self.compile(script)?;
        self.run_async(chunk).await
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
        self.runtime
            .loader()
            .borrow_mut()
            .compile_script(args.script, args.script_path, args.compiler_settings)
            .map_err(Error::from)
    }

    /// Runs a compiled script as a [`Chunk`] and returns the script's result
    ///
    /// 1. The script is run. If a runtime error is encountered it will be returned as an error.
    /// 2. If tests are enabled, the script's exported tests will be run.
    ///    The first test failure will be returned as an error.
    /// 3. If a @main function is exported, it will be called as the last step, with its return
    ///    value being returned as the script's result
    ///
    /// Note that the runtime's exports are persistant between runs; if you want to initialize a new
    /// script then first call `.exports_mut().clear()`.
    pub fn run(&mut self, chunk: Ptr<Chunk>) -> Result<KValue> {
        let result = self.runtime.run(chunk)?;
        let result = self.block_on_output(result)?;

        if self.run_tests {
            let exports = self.runtime.exports().clone();
            let output = self.runtime.run_tests(exports)?;
            self.block_on_output(output)?;
        }

        if let Some(main) = self.runtime.exports().get_meta_value(&MetaKey::Main) {
            let output = self.runtime.call_function(main, &[])?;
            let result = self.block_on_output(output)?;
            self.block_on_task_value(result)
        } else {
            Ok(result)
        }
    }

    /// Runs a compiled script as a [`Chunk`] asynchronously.
    ///
    /// This behaves like [run](Self::run), but lets the host application's async executor drive
    /// suspended Koto work.
    ///
    /// Executor-backed modules such as `task`, `io_async`, and `http` require an async backend
    /// to have been installed in the runtime or host application.
    pub async fn run_async(&mut self, chunk: Ptr<Chunk>) -> Result<KValue> {
        let result = self.runtime.run(chunk)?;
        let result = self.await_output(result).await?;

        if self.run_tests {
            let exports = self.runtime.exports().clone();
            let output = self.runtime.run_tests(exports)?;
            self.await_output(output).await?;
        }

        if let Some(main) = self.runtime.exports().get_meta_value(&MetaKey::Main) {
            let output = self.runtime.call_function(main, &[])?;
            let result = self.await_output(output).await?;
            self.await_task_value(result).await
        } else {
            Ok(result)
        }
    }

    /// Polls a task using the runtime's task executor.
    pub fn poll_task(&self, task: &mut KTask) -> Result<KTaskPoll> {
        self.runtime.poll_task(task).map_err(From::from)
    }

    /// Runs a task to completion, blocking the current thread while it's pending.
    pub fn block_on(&self, task: &mut KTask) -> Result<KValue> {
        task.block_on(&self.runtime).map_err(From::from)
    }

    /// Waits for a task asynchronously using the runtime's task executor.
    pub async fn await_task(&self, task: &mut KTask) -> Result<KValue> {
        poll_fn(
            |context| match self.runtime.poll_task_with_context(task, context) {
                Ok(KTaskPoll::Ready(value)) => Poll::Ready(Ok(value)),
                Ok(KTaskPoll::Pending) => Poll::Pending,
                Err(error) => Poll::Ready(Err(Error::from(error))),
            },
        )
        .await
    }

    /// Calls a function with the given arguments
    ///
    /// If the provided value isn't [callable](KValue::is_callable) then an error will be returned.
    pub fn call_function<'a>(
        &mut self,
        function: KValue,
        args: impl Into<CallArgs<'a>>,
    ) -> Result<KValue> {
        let output = self.runtime.call_function(function, args)?;
        self.block_on_output(output)
    }

    /// Returns a task that calls a function with the given arguments when polled or awaited.
    ///
    /// If the provided value isn't [callable](KValue::is_callable) then an error will be returned.
    pub fn call_function_as_task<'a>(
        &mut self,
        function: KValue,
        args: impl Into<CallArgs<'a>>,
    ) -> Result<KTask> {
        self.runtime
            .call_function_as_task(function, args)
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
        let output = self
            .runtime
            .call_instance_function(instance, function, args)?;
        self.block_on_output(output)
    }

    /// Returns a task that calls an instance function with the given arguments when polled or
    /// awaited.
    ///
    /// If the provided value isn't [callable](KValue::is_callable) then an error will be returned.
    pub fn call_instance_function_as_task<'a>(
        &mut self,
        instance: KValue,
        function: KValue,
        args: impl Into<CallArgs<'a>>,
    ) -> Result<KTask> {
        self.runtime
            .call_instance_function_as_task(instance, function, args)
            .map_err(From::from)
    }

    /// Calls an exported function with the given arguments
    ///
    /// If the requested function isn't present, or if it isn't [callable](KValue::is_callable),
    /// then an error will be returned.
    pub fn call_exported_function<'a>(
        &mut self,
        function_name: &str,
        args: impl Into<CallArgs<'a>>,
    ) -> Result<KValue> {
        match self.exports().get(function_name) {
            Some(f) => {
                let output = self.runtime.call_function(f, args)?;
                self.block_on_output(output)
            }
            None => Err(Error::MissingFunction(function_name.into())),
        }
    }

    /// Returns a task that calls an exported function with the given arguments when polled or
    /// awaited.
    ///
    /// If the requested function isn't present, or if it isn't [callable](KValue::is_callable),
    /// then an error will be returned.
    pub fn call_exported_function_as_task<'a>(
        &mut self,
        function_name: &str,
        args: impl Into<CallArgs<'a>>,
    ) -> Result<KTask> {
        match self.exports().get(function_name) {
            Some(f) => self
                .runtime
                .call_function_as_task(f, args)
                .map_err(From::from),
            None => Err(Error::MissingFunction(function_name.into())),
        }
    }

    /// Converts a [KValue] into a [String] by evaluating `@display` in the runtime
    pub fn value_to_string(&mut self, value: KValue) -> Result<String> {
        let output = self.runtime.value_to_string(&value)?;

        match self.block_on_output(output)? {
            KValue::Str(result) => Ok(result.as_str().to_owned()),
            unexpected => Err(Error::StringError(format!(
                "expected String from @display, found '{}'",
                unexpected.type_as_string()
            ))),
        }
    }

    /// Clears the loader's cached modules
    ///
    /// This is useful when a script's dependencies may have changed and need to be recompiled.
    pub fn clear_module_cache(&mut self) {
        self.runtime.loader().borrow_mut().clear_cache();
    }

    /// Enables or disables the `run_tests` setting
    ///
    /// Currently this is only used when running benchmarks where tests are run once during setup,
    /// and then disabled for repeated runs.
    pub fn set_run_tests(&mut self, enabled: bool) {
        self.run_tests = enabled;
    }

    fn block_on_output(&self, output: VmOutput) -> Result<KValue> {
        output
            .into_task()
            .block_on(&self.runtime)
            .map_err(From::from)
    }

    fn block_on_task_value(&self, value: KValue) -> Result<KValue> {
        match value {
            KValue::Task(mut task) => self.block_on(&mut task),
            value => Ok(value),
        }
    }

    async fn await_output(&self, output: VmOutput) -> Result<KValue> {
        let mut task = output.into_task();
        self.await_task(&mut task).await
    }

    async fn await_task_value(&self, value: KValue) -> Result<KValue> {
        match value {
            KValue::Task(mut task) => self.await_task(&mut task).await,
            value => Ok(value),
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
    /// Helper for conveniently setting the arguments to those of the current process
    ///
    /// # Panics
    ///
    /// This will panic if any argument of the current process is not valid Unicode.
    #[must_use]
    pub fn inherit_args(self) -> Self {
        Self {
            vm_settings: KotoVmSettings {
                args: std::env::args().collect(),
                ..self.vm_settings
            },
            ..self
        }
    }

    /// Helper for conveniently setting the stdio streams to those of the current process
    #[must_use]
    pub fn inherit_io(self) -> Self {
        Self {
            vm_settings: KotoVmSettings {
                stdin: make_ptr!(SystemStdin::default()),
                stdout: make_ptr!(SystemStdout::default()),
                stderr: make_ptr!(SystemStderr::default()),
                ..self.vm_settings
            },
            ..self
        }
    }

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

    /// Helper for conveniently defining custom args
    #[must_use]
    pub fn with_args(self, args: impl IntoIterator<Item: Into<String>>) -> Self {
        Self {
            vm_settings: KotoVmSettings {
                args: args.into_iter().map(Into::into).collect(),
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
