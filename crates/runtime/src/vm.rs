use crate::{
    InstructionFrame, KFunction, Ptr, Result, UnavailableStderr, UnavailableStdin,
    UnavailableStdout,
    core_lib::{CoreLib, io::File, koto::Unimplemented},
    error::{Error, ErrorKind},
    prelude::*,
    types::{
        ActiveTasks, FunctionContext, LocalTaskExecutor, meta_id_to_key, value::RegisterSlice,
    },
};
use instant::Instant;
use koto_bytecode::{Chunk, Instruction, InstructionReader, ModuleLoader, Op};
use koto_parser::{
    ConstantIndex, MetaKeyId, StringAlignment, StringFormatOptions, StringFormatRepresentation,
};
use rustc_hash::FxHasher;
use smallvec::SmallVec;
use std::{
    collections::HashMap,
    fmt,
    hash::BuildHasherDefault,
    path::{Path, PathBuf},
    task::{Context, Waker},
    time::Duration,
};
use unicode_segmentation::UnicodeSegmentation;

/// The output of a VM operation that can suspend.
pub enum VmOutput {
    /// The operation completed immediately.
    Ready(KValue),
    /// The operation is waiting for async work to complete.
    Pending(KTask),
}

impl VmOutput {
    /// Returns true if the operation completed immediately.
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }

    /// Returns true if the operation is waiting for async work to complete.
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending(_))
    }

    /// Converts the output into a task.
    pub fn into_task(self) -> KTask {
        match self {
            Self::Ready(value) => KTask::with_value(value),
            Self::Pending(task) => task,
        }
    }
}

impl From<KValue> for VmOutput {
    fn from(value: KValue) -> Self {
        Self::Ready(value)
    }
}

impl From<KTask> for VmOutput {
    fn from(task: KTask) -> Self {
        Self::Pending(task)
    }
}

#[derive(Clone)]
pub enum ControlFlow {
    Continue,
    Return(KValue),
    Yield(KValue),
    Pending,
}

enum IterationStep {
    Output(Option<KValue>),
    Pending,
}

/// State shared between concurrent VMs
struct VmContext {
    // The settings that were used to initialize the runtime
    settings: KotoVmSettings,
    // The runtime's prelude
    prelude: KMap,
    // The runtime's core library
    core_lib: CoreLib,
    // The module loader used to compile imported modules
    loader: KCell<ModuleLoader>,
    // The cached export maps of imported modules
    module_cache: KCell<ModuleCache>,
    // The active task manager used to spawn and poll async work
    tasks: KCell<ActiveTasks>,
}

impl Default for VmContext {
    fn default() -> Self {
        Self::with_settings(KotoVmSettings::default())
    }
}

impl VmContext {
    fn with_settings(settings: KotoVmSettings) -> Self {
        let core_lib = CoreLib::default();

        core_lib.os.insert(
            "args",
            KValue::Tuple(
                settings
                    .args
                    .iter()
                    .map(|s| KValue::from(s.as_str()))
                    .collect::<Vec<_>>()
                    .into(),
            ),
        );

        core_lib
            .io
            .insert("stdin", File::new(settings.stdin.clone()));

        core_lib
            .io
            .insert("stdout", File::new(settings.stdout.clone()));

        core_lib
            .io
            .insert("stderr", File::new(settings.stderr.clone()));

        let task_executor = settings.task_executor.clone();

        Self {
            settings,
            prelude: core_lib.prelude(),
            core_lib,
            loader: ModuleLoader::default().into(),
            module_cache: ModuleCache::default().into(),
            tasks: ActiveTasks::new(task_executor).into(),
        }
    }
}

/// The trait used by the 'module imported' callback mechanism
pub trait ModuleImportedCallback: Fn(&Path) + KotoSend + KotoSync {}

// Implement the trait for any matching function
impl<T> ModuleImportedCallback for T where T: Fn(&Path) + KotoSend + KotoSync {}

/// The configurable settings that should be used by the Koto runtime
pub struct KotoVmSettings {
    /// Whether or not tests should be run when importing modules
    ///
    /// Default: `true`
    pub run_import_tests: bool,

    /// An optional duration that limits how long execution is allowed to take.
    ///
    /// If the limit is reached without execution ending,
    /// then a [Timeout](ErrorKind::Timeout) error will be returned.
    ///
    /// The VM will check against the execution deadline periodically, with an interval of roughly
    /// one tenth of the overall limit's duration.
    ///
    /// The check is performed between VM instructions, so external functions will still be able to
    /// block execution.
    ///
    /// Default: `None`
    pub execution_limit: Option<Duration>,

    /// An optional callback that is called whenever a module is imported by the runtime
    ///
    /// This allows you to track the runtime's dependencies, which might be useful if you want to
    /// reload the script when one of its dependencies has changed.
    pub module_imported_callback: Option<Box<dyn ModuleImportedCallback>>,

    /// The executor used to spawn and poll async tasks.
    ///
    /// Default: [`LocalTaskExecutor`]
    pub task_executor: Ptr<dyn KotoTaskExecutor>,

    /// The runtime's `stdin`that can be accessed from within the script via `io.stdin`
    ///
    /// Default: [`UnavailableStdin`]
    pub stdin: Ptr<dyn KotoFile>,

    /// The runtime's `stdout`that can be accessed from within the script via `io.stdout`
    ///
    /// Default: [`UnavailableStdout`]
    pub stdout: Ptr<dyn KotoFile>,

    /// The runtime's `stderr` that can be accessed from within the script via `io.stderr`
    ///
    /// Default: [`UnavailableStderr`]
    pub stderr: Ptr<dyn KotoFile>,

    /// The runtime's `args` that can be accessed from within the script via `os.args`
    ///
    /// Default: `vec![]`
    pub args: Vec<String>,
}

impl Default for KotoVmSettings {
    fn default() -> Self {
        Self {
            run_import_tests: true,
            execution_limit: None,
            module_imported_callback: None,
            task_executor: make_ptr!(LocalTaskExecutor),
            stdin: make_ptr!(UnavailableStdin::default()),
            stdout: make_ptr!(UnavailableStdout::default()),
            stderr: make_ptr!(UnavailableStderr::default()),
            args: vec![],
        }
    }
}

/// The Koto runtime's virtual machine
#[derive(Clone)]
pub struct KotoVm {
    // The exports map for the current module
    exports: KMap,
    // Context shared by all VMs in the runtime
    context: Ptr<VmContext>,
    // The VM's instruction reader, containing a pointer to the bytecode chunk that's being executed
    reader: InstructionReader,
    // The VM's register stack
    registers: Vec<KValue>,
    // The current frame's register base
    register_base: usize,
    // The minimum number of registers required by the current frame, declared by the NewFrame op
    min_frame_registers: usize,
    // The VM's call stack
    call_stack: Vec<Frame>,
    // A stack of sequences that are currently under construction
    sequence_builders: Vec<Vec<KValue>>,
    // A stack of strings that are currently under construction
    string_builders: Vec<String>,
    // The ip that produced the most recently read instruction, used for debug and error traces
    instruction_ip: u32,
    // The waker used while this VM is being polled as a task
    task_waker: Option<Waker>,
    // The stack of modules currently being imported by this VM
    module_import_stack: Vec<PathBuf>,
    // The current execution state
    execution_state: ExecutionState,
}

/// The execution state of a VM
#[derive(Debug, Clone)]
pub enum ExecutionState {
    /// The VM is ready to execute instructions
    Inactive,
    /// The VM is currently executing instructions
    Active,
    /// The VM is executing a generator function that has just yielded a value
    Suspended,
    /// The VM is waiting for a pending task
    Pending,
}

impl Default for KotoVm {
    fn default() -> Self {
        Self::with_settings(KotoVmSettings::default())
    }
}

impl KotoVm {
    /// Initializes a Koto VM with the provided settings
    pub fn with_settings(settings: KotoVmSettings) -> Self {
        Self {
            exports: KMap::default(),
            context: VmContext::with_settings(settings).into(),
            reader: InstructionReader::default(),
            registers: Vec::with_capacity(32),
            register_base: 0,
            min_frame_registers: 0,
            call_stack: Vec::new(),
            sequence_builders: Vec::new(),
            string_builders: Vec::new(),
            instruction_ip: 0,
            task_waker: None,
            module_import_stack: Vec::new(),
            execution_state: ExecutionState::Inactive,
        }
    }

    /// Spawn a VM that shares the same execution context
    ///
    /// E.g.
    ///   - An iterator spawns a shared VM that can be used to execute functors
    ///   - A generator function spawns a shared VM to yield incremental results
    ///   - Thrown errors spawn a shared VM to display an error from a custom error type
    #[must_use]
    pub fn spawn_shared_vm(&self) -> Self {
        Self {
            exports: self.exports.clone(),
            context: self.context.clone(),
            reader: self.reader.clone(),
            registers: Vec::with_capacity(8),
            register_base: 0,
            min_frame_registers: 0,
            call_stack: Vec::new(),
            sequence_builders: Vec::new(),
            string_builders: Vec::new(),
            instruction_ip: 0,
            task_waker: None,
            module_import_stack: self.module_import_stack.clone(),
            execution_state: ExecutionState::Inactive,
        }
    }

    /// Spawns an await-compatible VM that shares the same execution context.
    #[must_use]
    pub fn spawn_async_vm(&self) -> AsyncKotoVm {
        AsyncKotoVm::new(self.spawn_shared_vm())
    }

    pub(crate) fn spawn_shared_vm_with_current_instruction(&self) -> Self {
        let mut result = self.spawn_shared_vm();
        result.instruction_ip = self.instruction_ip;
        result
    }

    /// The loader, responsible for loading and compiling Koto scripts and modules
    pub fn loader(&self) -> &KCell<ModuleLoader> {
        &self.context.loader
    }

    /// The prelude, containing items that can be imported within all modules
    pub fn prelude(&self) -> &KMap {
        &self.context.prelude
    }

    /// The active module's exports map
    ///
    /// Note that this is the exports map of the active module, so during execution the returned
    /// map will be of the module that's currently being executed.
    pub fn exports(&self) -> &KMap {
        &self.exports
    }

    /// Returns a mutable reference to the active module's exports map
    pub fn exports_mut(&mut self) -> &mut KMap {
        &mut self.exports
    }

    /// The `stdin` wrapper used by the VM
    pub fn stdin(&self) -> &Ptr<dyn KotoFile> {
        &self.context.settings.stdin
    }

    /// The `stdout` wrapper used by the VM
    pub fn stdout(&self) -> &Ptr<dyn KotoFile> {
        &self.context.settings.stdout
    }

    /// The `stderr` wrapper used by the VM
    pub fn stderr(&self) -> &Ptr<dyn KotoFile> {
        &self.context.settings.stderr
    }

    /// Runs the provided [Chunk].
    ///
    /// If the chunk suspends, then [VmOutput::Pending] will be returned with a task that can be
    /// polled to completion.
    pub fn run(&mut self, chunk: Ptr<Chunk>) -> Result<VmOutput> {
        let task = self.make_run_task(chunk);
        self.poll_task_until_pending_or_ready(task)
    }

    /// Returns a task that will run the provided [Chunk] when polled or awaited.
    pub fn run_as_task(&mut self, chunk: Ptr<Chunk>) -> Result<KTask> {
        let task = self.make_run_task(chunk);
        self.spawn_task(task)
    }

    fn make_run_task(&mut self, chunk: Ptr<Chunk>) -> KTask {
        self.exports.ensure_meta_map();

        let mut vm = self.spawn_shared_vm();
        vm.push_run_frame(chunk);
        KTask::with_vm(vm)
    }

    /// Spawns a task in the runtime's task executor.
    pub fn spawn_task(&self, task: KTask) -> Result<KTask> {
        self.context.tasks.borrow_mut().spawn(task)
    }

    /// Spawns a native future in the runtime's task executor.
    pub fn spawn_future(&self, future: impl KotoFuture) -> Result<KTask> {
        self.spawn_task(KTask::with_future(future))
    }

    /// Returns a task that will complete after the given duration.
    pub fn sleep(&self, duration: Duration) -> Result<KTask> {
        self.context.tasks.borrow_mut().sleep(duration)
    }

    /// Polls a task using the runtime's task executor.
    pub fn poll_task(&self, task: &mut KTask) -> Result<KTaskPoll> {
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);

        self.poll_task_with_context(task, &mut context)
    }

    /// Polls a task with the given context using the runtime's task executor.
    pub fn poll_task_with_context(
        &self,
        task: &mut KTask,
        context: &mut Context<'_>,
    ) -> Result<KTaskPoll> {
        self.poll_woken_tasks_except(Some(task));

        let executor = self.context.tasks.borrow().executor().clone();

        executor.poll(task, context)
    }

    pub(crate) fn current_task_waker(&self) -> Option<Waker> {
        self.task_waker.clone()
    }

    /// Polls any active tasks that have been woken.
    ///
    /// Task failures are stored in their task handles, and can be observed when the task is awaited
    /// or explicitly polled.
    pub fn poll_woken_tasks(&self) -> usize {
        self.poll_woken_tasks_except(None)
    }

    pub(crate) fn poll_woken_tasks_except(&self, excluded_task: Option<&KTask>) -> usize {
        {
            let mut tasks = self.context.tasks.borrow_mut();
            if tasks.is_polling() {
                return 0;
            }
            tasks.set_is_polling(true);
        }

        let tasks_to_poll = self.context.tasks.borrow().woken_tasks(excluded_task);
        let executor = self.context.tasks.borrow().executor().clone();
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        let mut result = 0;

        for mut task in tasks_to_poll {
            let _ = executor.poll(&mut task, &mut context);
            result += 1;
        }

        {
            let mut tasks = self.context.tasks.borrow_mut();
            tasks.set_is_polling(false);
            tasks.remove_inactive_tasks();
        }

        result
    }

    fn push_run_frame(&mut self, chunk: Ptr<Chunk>) -> u8 {
        // Set up an execution frame to run the chunk in
        let frame_base = self.next_register();
        self.registers.push(KValue::Null); // Instance register
        self.push_frame(
            chunk,
            0,
            frame_base,
            None,
            // Provide access to the module's exports
            Some(NonLocals {
                module_exports: self.exports.clone(),
                wildcard_imports: None,
            }),
        );

        // Ensure that execution stops here if an error is thrown
        self.frame_mut().execution_barrier = true;

        frame_base
    }

    /// Continues execution in a suspended VM.
    ///
    /// This is currently used to support generators and tasks.
    pub fn continue_running(&mut self) -> Result<ReturnOrYield> {
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);

        self.continue_running_with_context(&mut context)
    }

    /// Continues execution in a suspended VM with the given task context.
    pub(crate) fn continue_running_with_context(
        &mut self,
        context: &mut Context<'_>,
    ) -> Result<ReturnOrYield> {
        let previous_waker = self.task_waker.replace(context.waker().clone());
        let result = self.continue_running_inner();
        self.task_waker = previous_waker;

        result
    }

    fn continue_running_inner(&mut self) -> Result<ReturnOrYield> {
        if self.call_stack.is_empty() {
            return Ok(ReturnOrYield::Return(KValue::Null));
        }

        let result = self.execute_instructions()?;

        match self.execution_state {
            ExecutionState::Inactive => Ok(ReturnOrYield::Return(result)),
            ExecutionState::Suspended => Ok(ReturnOrYield::Yield(result)),
            ExecutionState::Pending => Ok(ReturnOrYield::Pending),
            ExecutionState::Active => unreachable!(),
        }
    }

    /// Calls a function with some given arguments.
    ///
    /// If the function suspends, then [VmOutput::Pending] will be returned with a task that can be
    /// polled to completion.
    pub fn call_function<'a>(
        &mut self,
        function: KValue,
        args: impl Into<CallArgs<'a>>,
    ) -> Result<VmOutput> {
        let task = self.make_function_call_task(
            None,
            function.is_async_callable(),
            function,
            args.into(),
        )?;
        self.poll_task_until_pending_or_ready(task)
    }

    /// Runs an instance function with some given arguments.
    ///
    /// If the function suspends, then [VmOutput::Pending] will be returned with a task that can be
    /// polled to completion.
    pub fn call_instance_function<'a>(
        &mut self,
        instance: KValue,
        function: KValue,
        args: impl Into<CallArgs<'a>>,
    ) -> Result<VmOutput> {
        let task = self.make_function_call_task(
            Some(instance),
            function.is_async_callable(),
            function,
            args.into(),
        )?;
        self.poll_task_until_pending_or_ready(task)
    }

    /// Returns a task that calls a function with the given arguments when polled or awaited.
    pub fn call_function_as_task<'a>(
        &mut self,
        function: KValue,
        args: impl Into<CallArgs<'a>>,
    ) -> Result<KTask> {
        let task = self.make_function_call_task(
            None,
            function.is_async_callable(),
            function,
            args.into(),
        )?;
        self.spawn_task(task)
    }

    /// Returns a task that calls an instance function with the given arguments when polled or
    /// awaited.
    pub fn call_instance_function_as_task<'a>(
        &mut self,
        instance: KValue,
        function: KValue,
        args: impl Into<CallArgs<'a>>,
    ) -> Result<KTask> {
        let task = self.make_function_call_task(
            Some(instance),
            function.is_async_callable(),
            function,
            args.into(),
        )?;
        self.spawn_task(task)
    }

    pub(crate) fn call_function_without_awaiting_as_task<'a>(
        &mut self,
        function: KValue,
        args: impl Into<CallArgs<'a>>,
    ) -> Result<KTask> {
        let task = self.make_function_call_task(None, false, function, args.into())?;
        self.spawn_task(task)
    }

    fn make_value_task_vm(&self, await_result: bool) -> (KotoVm, u8) {
        let mut vm = self.spawn_shared_vm();

        let result_register = vm.next_register();
        vm.registers.push(KValue::Null); // Result register

        let mut return_bytes = Vec::with_capacity(if await_result { 4 } else { 2 });
        if await_result {
            return_bytes.extend_from_slice(&[Op::Await as u8, result_register]);
        }
        return_bytes.extend_from_slice(&[Op::Return as u8, result_register]);

        Self::push_task_frame(&mut vm, return_bytes, 1);

        (vm, result_register)
    }

    fn push_task_frame(vm: &mut KotoVm, bytes: Vec<u8>, required_registers: u8) {
        let chunk = make_ptr!(Chunk {
            bytes,
            ..Default::default()
        });
        let module_exports = vm.exports.clone();

        vm.push_frame(
            chunk,
            0,
            0,
            None,
            Some(NonLocals {
                module_exports,
                wildcard_imports: None,
            }),
        );
        vm.frame_mut().execution_barrier = true;
        vm.frame_mut().required_registers = required_registers;
    }

    fn make_function_call_task(
        &mut self,
        instance: Option<KValue>,
        await_result: bool,
        function: KValue,
        args: CallArgs,
    ) -> Result<KTask> {
        if !function.is_callable() {
            return unexpected_type("Function", &function);
        }

        let (mut vm, result_register) = self.make_value_task_vm(await_result);

        let args = match (&args, &function) {
            (CallArgs::AsTuple(args), KValue::Function(f)) if f.flags.arg_is_unpacked_tuple() => {
                let start = vm.registers.len();
                vm.registers.extend(args.iter().cloned());
                CallArgs::Single(KValue::TemporaryTuple(RegisterSlice {
                    start,
                    count: args.len(),
                }))
            }
            _ => args,
        };

        let frame_base = vm.next_register();
        vm.registers.push(instance.unwrap_or_default()); // Frame base

        let arg_count = match args {
            CallArgs::Single(arg) => {
                vm.registers.push(arg);
                1
            }
            CallArgs::Separate(args) => {
                vm.registers.extend_from_slice(args);
                args.len() as u8
            }
            CallArgs::AsTuple(args) => {
                vm.registers.push(KValue::Tuple(Vec::from(args).into()));
                1
            }
        };

        vm.call_callable(
            CallInfo {
                result_register: Some(result_register),
                frame_base,
                instance: Some(frame_base),
                arg_count,
                packed_arg_count: 0,
            },
            function,
        )?;

        Ok(KTask::with_vm(vm))
    }

    fn poll_task_until_pending_or_ready(&self, mut task: KTask) -> Result<VmOutput> {
        let poll_result = if let Some(waker) = self.current_task_waker() {
            let mut context = Context::from_waker(&waker);
            self.poll_task_with_context(&mut task, &mut context)?
        } else {
            self.poll_task(&mut task)?
        };

        match poll_result {
            KTaskPoll::Ready(value) => Ok(VmOutput::Ready(value)),
            KTaskPoll::Pending => Ok(VmOutput::Pending(task)),
        }
    }

    /// Returns a displayable string for the given value.
    ///
    /// If display conversion suspends, then [VmOutput::Pending] will be returned with a task that
    /// can be polled to completion.
    pub fn value_to_string(&mut self, value: &KValue) -> Result<VmOutput> {
        let task = self.make_value_to_string_task(value.clone());
        self.poll_task_until_pending_or_ready(task)
    }

    /// Returns a task that provides a displayable string for the given value when polled or awaited.
    pub fn value_to_string_as_task(&mut self, value: KValue) -> Result<KTask> {
        let task = self.make_value_to_string_task(value);
        self.spawn_task(task)
    }

    fn make_value_to_string_task(&self, value: KValue) -> KTask {
        let mut runner = self.spawn_async_vm();
        KTask::with_future(async move { Ok(KValue::from(runner.value_to_string(value).await?)) })
    }

    /// Returns a debug string for the given value.
    ///
    /// If debug conversion suspends, then [VmOutput::Pending] will be returned with a task that can
    /// be polled to completion.
    pub fn value_to_debug_string(&mut self, value: &KValue) -> Result<VmOutput> {
        let task = self.make_value_to_debug_string_task(value.clone());
        self.poll_task_until_pending_or_ready(task)
    }

    /// Returns a task that provides a debug string for the given value when polled or awaited.
    pub fn value_to_debug_string_as_task(&mut self, value: KValue) -> Result<KTask> {
        let task = self.make_value_to_debug_string_task(value);
        self.spawn_task(task)
    }

    fn make_value_to_debug_string_task(&self, value: KValue) -> KTask {
        let mut runner = self.spawn_async_vm();
        KTask::with_future(
            async move { Ok(KValue::from(runner.value_to_debug_string(value).await?)) },
        )
    }

    fn display_value_with_runner(&mut self, result: u8, value: KValue, debug: bool) -> Result<()> {
        let mut runner = AsyncKotoVm::new(self.spawn_shared_vm_with_current_instruction());
        let future = async move {
            let result = if debug {
                runner.value_to_debug_string(value).await?
            } else {
                runner.value_to_string(value).await?
            };
            Ok(result.into())
        };
        self.poll_task_into_register(result, KTask::with_future(future))
    }

    fn poll_task_into_register(&mut self, result: u8, mut task: KTask) -> Result<()> {
        let poll_result = if let Some(waker) = self.current_task_waker() {
            let mut context = Context::from_waker(&waker);
            task.poll_with_context(&mut context)?
        } else {
            task.poll()?
        };

        match poll_result {
            KTaskPoll::Ready(result_value) => {
                self.set_register(result, result_value);
            }
            KTaskPoll::Pending => {
                let task = self.spawn_task(task)?;
                self.set_register(result, task.into());
            }
        }

        Ok(())
    }

    /// Provides the result of running a unary operation on a KValue.
    ///
    /// If the operation suspends, then [VmOutput::Pending] will be returned with a task that can be
    /// polled to completion.
    pub fn run_unary_op(&mut self, op: UnaryOp, value: KValue) -> Result<VmOutput> {
        let task = self.make_unary_op_task(op, value)?;
        self.poll_task_until_pending_or_ready(task)
    }

    /// Returns a task that provides the result of running a unary operation on a value
    /// when polled or awaited.
    pub fn run_unary_op_as_task(&mut self, op: UnaryOp, value: KValue) -> Result<KTask> {
        let task = self.make_unary_op_task(op, value)?;
        self.spawn_task(task)
    }

    fn make_unary_op_task(&mut self, op: UnaryOp, value: KValue) -> Result<KTask> {
        use UnaryOp::*;

        if matches!(op, Next) {
            let mut vm = self.spawn_shared_vm();

            let result_register = vm.next_register();
            vm.registers.push(KValue::Null);
            let value_register = vm.next_register();
            vm.registers.push(value);

            let return_bytes = vec![
                Op::IterNext as u8,
                result_register,
                value_register,
                0,
                0,
                Op::Await as u8,
                result_register,
                Op::Return as u8,
                result_register,
            ];
            Self::push_task_frame(&mut vm, return_bytes, 2);

            return Ok(KTask::with_vm(vm));
        }

        let (mut vm, result_register) = self.make_value_task_vm(true);

        let value_register = vm.next_register();
        vm.registers.push(value);

        vm.run_unary_op_in_register(op, result_register, value_register)?;

        Ok(KTask::with_vm(vm))
    }

    fn run_unary_op_in_register(
        &mut self,
        op: UnaryOp,
        result_register: u8,
        value_register: u8,
    ) -> Result<ControlFlow> {
        use UnaryOp::*;

        match op {
            Debug => self.run_debug_op(result_register, value_register)?,
            Display => self.run_display(result_register, value_register)?,
            Negate => self.run_negate(result_register, value_register)?,
            Iterator => self.run_make_iterator(result_register, value_register, false)?,
            Next => return self.run_iterator_next(Some(result_register), value_register, 0, false),
            NextBack => match self.clone_register(value_register) {
                KValue::Map(m) if m.contains_meta_key(&NextBack.into()) => {
                    let op = m.get_meta_value(&NextBack.into()).unwrap();
                    if !op.is_callable() {
                        return unexpected_type("Callable function from @next_back", &op);
                    }
                    self.call_overridden_op_1(Some(result_register), value_register, op)?
                }
                unexpected => {
                    return unexpected_type(
                        "Value with an implementation of @next_back",
                        &unexpected,
                    );
                }
            },
            Size => self.run_size(result_register, value_register, true)?,
        }

        Ok(ControlFlow::Continue)
    }

    /// Provides the result of running a binary operation on a pair of Values.
    ///
    /// If the operation suspends, then [VmOutput::Pending] will be returned with a task that can be
    /// polled to completion.
    pub fn run_binary_op(&mut self, op: BinaryOp, lhs: KValue, rhs: KValue) -> Result<VmOutput> {
        let task = self.make_binary_op_task(op, lhs, rhs)?;
        self.poll_task_until_pending_or_ready(task)
    }

    /// Returns a task that provides the result of running a binary operation on a pair of Values
    /// when polled or awaited.
    pub fn run_binary_op_as_task(
        &mut self,
        op: BinaryOp,
        lhs: KValue,
        rhs: KValue,
    ) -> Result<KTask> {
        let task = self.make_binary_op_task(op, lhs, rhs)?;
        self.spawn_task(task)
    }

    fn make_binary_op_task(&mut self, op: BinaryOp, lhs: KValue, rhs: KValue) -> Result<KTask> {
        let (mut vm, result_register) = self.make_value_task_vm(true);

        let lhs_register = vm.next_register();
        vm.registers.push(lhs);
        let rhs_register = vm.next_register();
        vm.registers.push(rhs);

        vm.run_binary_op_in_registers(op, result_register, lhs_register, rhs_register)?;

        Ok(KTask::with_vm(vm))
    }

    fn run_binary_op_in_registers(
        &mut self,
        op: BinaryOp,
        result_register: u8,
        lhs_register: u8,
        rhs_register: u8,
    ) -> Result<()> {
        match op {
            BinaryOp::Add | BinaryOp::AddRhs => {
                self.run_add(result_register, lhs_register, rhs_register)
            }
            BinaryOp::Subtract | BinaryOp::SubtractRhs => {
                self.run_subtract(result_register, lhs_register, rhs_register)
            }
            BinaryOp::Multiply | BinaryOp::MultiplyRhs => {
                self.run_multiply(result_register, lhs_register, rhs_register)
            }
            BinaryOp::Divide | BinaryOp::DivideRhs => {
                self.run_divide(result_register, lhs_register, rhs_register)
            }
            BinaryOp::Remainder | BinaryOp::RemainderRhs => {
                self.run_remainder(result_register, lhs_register, rhs_register)
            }
            BinaryOp::Power | BinaryOp::PowerRhs => {
                self.run_power(result_register, lhs_register, rhs_register)
            }
            BinaryOp::AddAssign => {
                self.run_add_assign(lhs_register, rhs_register)?;
                self.set_register(result_register, self.clone_register(lhs_register));
                Ok(())
            }
            BinaryOp::SubtractAssign => {
                self.run_subtract_assign(lhs_register, rhs_register)?;
                self.set_register(result_register, self.clone_register(lhs_register));
                Ok(())
            }
            BinaryOp::MultiplyAssign => {
                self.run_multiply_assign(lhs_register, rhs_register)?;
                self.set_register(result_register, self.clone_register(lhs_register));
                Ok(())
            }
            BinaryOp::DivideAssign => {
                self.run_divide_assign(lhs_register, rhs_register)?;
                self.set_register(result_register, self.clone_register(lhs_register));
                Ok(())
            }
            BinaryOp::RemainderAssign => {
                self.run_remainder_assign(lhs_register, rhs_register)?;
                self.set_register(result_register, self.clone_register(lhs_register));
                Ok(())
            }
            BinaryOp::PowerAssign => {
                self.run_power_assign(lhs_register, rhs_register)?;
                self.set_register(result_register, self.clone_register(lhs_register));
                Ok(())
            }
            BinaryOp::Less => self.run_less(result_register, lhs_register, rhs_register),
            BinaryOp::LessOrEqual => {
                self.run_less_or_equal(result_register, lhs_register, rhs_register)
            }
            BinaryOp::Greater => self.run_greater(result_register, lhs_register, rhs_register),
            BinaryOp::GreaterOrEqual => {
                self.run_greater_or_equal(result_register, lhs_register, rhs_register)
            }
            BinaryOp::Equal => self.run_equal(result_register, lhs_register, rhs_register),
            BinaryOp::NotEqual => self.run_not_equal(result_register, lhs_register, rhs_register),
        }
    }

    /// Provides the result of running a read operation (i.e. access or index) on a pair of values.
    ///
    /// If the operation suspends, then [VmOutput::Pending] will be returned with a task that can be
    /// polled to completion.
    pub fn run_read_op(
        &mut self,
        op: ReadOp,
        container: KValue,
        read_arg: KValue,
    ) -> Result<VmOutput> {
        let task = self.make_read_op_task(op, container, read_arg)?;
        self.poll_task_until_pending_or_ready(task)
    }

    /// Returns a task that provides the result of running a read operation (i.e. access or index)
    /// on a pair of values when polled or awaited.
    pub fn run_read_op_as_task(
        &mut self,
        op: ReadOp,
        container: KValue,
        read_arg: KValue,
    ) -> Result<KTask> {
        let task = self.make_read_op_task(op, container, read_arg)?;
        self.spawn_task(task)
    }

    fn make_read_op_task(
        &mut self,
        op: ReadOp,
        container: KValue,
        read_arg: KValue,
    ) -> Result<KTask> {
        let (mut vm, result_register) = self.make_value_task_vm(true);

        let container_register = vm.next_register();
        vm.registers.push(container);
        let read_arg_register = vm.next_register();
        vm.registers.push(read_arg);

        vm.run_read_op_in_registers(op, result_register, container_register, read_arg_register)?;

        Ok(KTask::with_vm(vm))
    }

    fn run_read_op_in_registers(
        &mut self,
        op: ReadOp,
        result_register: u8,
        container_register: u8,
        read_arg_register: u8,
    ) -> Result<()> {
        match op {
            ReadOp::Index => self.run_index(result_register, container_register, read_arg_register),
            ReadOp::Access => {
                let key_string = match self.clone_register(read_arg_register) {
                    KValue::Str(s) => s,
                    other => return unexpected_type("a String", &other),
                };
                self.run_access(result_register, container_register, key_string)
            }
        }
    }

    /// Provides the result of running a write operation (i.e. via access or index).
    ///
    /// If the operation suspends, then [VmOutput::Pending] will be returned with a task that can be
    /// polled to completion.
    pub fn run_write_op(
        &mut self,
        op: WriteOp,
        container: KValue,
        write_arg: KValue,
        write_value: KValue,
    ) -> Result<VmOutput> {
        let task = self.make_write_op_task(op, container, write_arg, write_value)?;
        self.poll_task_until_pending_or_ready(task)
    }

    /// Returns a task that provides the result of running a write operation (i.e. access or index)
    /// on a value when polled or awaited.
    pub fn run_write_op_as_task(
        &mut self,
        op: WriteOp,
        container: KValue,
        write_arg: KValue,
        write_value: KValue,
    ) -> Result<KTask> {
        let task = self.make_write_op_task(op, container, write_arg, write_value)?;
        self.spawn_task(task)
    }

    fn make_write_op_task(
        &mut self,
        op: WriteOp,
        container: KValue,
        write_arg: KValue,
        write_value: KValue,
    ) -> Result<KTask> {
        let (mut vm, result_register) = self.make_value_task_vm(true);

        let container_register = vm.next_register();
        vm.registers.push(container);
        let write_arg_register = vm.next_register();
        vm.registers.push(write_arg);
        let write_value_register = vm.next_register();
        vm.registers.push(write_value);

        vm.run_write_op_in_registers(
            op,
            Some(result_register),
            container_register,
            write_arg_register,
            write_value_register,
        )?;

        Ok(KTask::with_vm(vm))
    }

    fn run_write_op_in_registers(
        &mut self,
        op: WriteOp,
        result_register: Option<u8>,
        container_register: u8,
        write_arg_register: u8,
        write_value_register: u8,
    ) -> Result<ControlFlow> {
        match op {
            WriteOp::IndexAssign => self.run_index_assign(
                result_register,
                container_register,
                write_arg_register,
                write_value_register,
            ),
            WriteOp::AccessAssign => self.run_access_assign(
                result_register,
                container_register,
                write_arg_register,
                write_value_register,
            ),
        }
    }

    /// Makes a [KIterator] that iterates over the provided value's contents.
    ///
    /// If iterator creation suspends, then [VmOutput::Pending] will be returned with a task that
    /// can be polled to completion.
    pub fn make_iterator(&mut self, value: KValue) -> Result<VmOutput> {
        let task = self.make_iterator_task(value);
        self.poll_task_until_pending_or_ready(task)
    }

    pub(crate) fn make_iterator_from_ready_value(&mut self, value: KValue) -> Result<KIterator> {
        use KValue::*;

        match value {
            Map(ref m) if m.contains_meta_key(&UnaryOp::Next.into()) => {
                KIterator::with_meta_next(self.spawn_shared_vm(), value)
            }
            Map(ref m) if m.contains_meta_key(&UnaryOp::Iterator.into()) => {
                runtime_error!("iterator creation requires an async VM")
            }
            Iterator(i) => Ok(i),
            Range(r) => KIterator::with_range(r),
            List(l) => Ok(KIterator::with_list(l)),
            Tuple(t) => Ok(KIterator::with_tuple(t)),
            Str(s) => Ok(KIterator::with_string(s)),
            Map(m) => Ok(KIterator::with_map(m)),
            Object(ref o) => {
                use IsIterable::*;

                let o_inner = o.try_borrow()?;
                match o_inner.is_iterable() {
                    NotIterable => unexpected_type("Iterable", &value),
                    Iterable => o_inner.make_iterator(self),
                    ForwardIterator | BidirectionalIterator => {
                        KIterator::with_object(self.spawn_shared_vm(), o.clone())
                    }
                }
            }
            unexpected => unexpected_type("Iterable", &unexpected),
        }
    }

    /// Returns a task that will make an iterator from the provided value when polled or awaited.
    pub fn make_iterator_as_task(&mut self, value: KValue) -> Result<KTask> {
        let task = self.make_iterator_task(value);
        self.spawn_task(task)
    }

    fn make_iterator_task(&self, value: KValue) -> KTask {
        let mut runner = self.spawn_async_vm();
        KTask::with_future(async move { Ok(KValue::Iterator(runner.make_iterator(value).await?)) })
    }

    /// Runs any function tagged with `@test` in the provided map.
    ///
    /// Any test failure will be returned as an error.
    ///
    /// If a test suspends, then [VmOutput::Pending] will be returned with a task that can be polled
    /// to completion.
    pub fn run_tests(&mut self, test_map: KMap) -> Result<VmOutput> {
        let task = self.make_run_tests_task(test_map);
        self.poll_task_until_pending_or_ready(task)
    }

    /// Returns a task that will run any function tagged with `@test` in the provided map
    /// when polled or awaited.
    pub fn run_tests_as_task(&mut self, test_map: KMap) -> Result<KTask> {
        let task = self.make_run_tests_task(test_map);
        self.spawn_task(task)
    }

    fn make_run_tests_task(&self, test_map: KMap) -> KTask {
        let mut runner = self.spawn_async_vm();
        KTask::with_future(async move { runner.run_tests(test_map).await })
    }

    fn execute_instructions(&mut self) -> Result<KValue> {
        let mut timeout = self
            .context
            .settings
            .execution_limit
            .map(ExecutionTimeout::new);

        // Every code path in this function must set the execution state to something other
        // than Active before exiting.
        self.execution_state = ExecutionState::Active;

        loop {
            if let Some(timeout) = timeout.as_mut()
                && timeout.check_for_timeout()
            {
                self.execution_state = ExecutionState::Inactive;
                return self
                    .pop_call_stack_on_error(
                        ErrorKind::Timeout(timeout.execution_limit).into(),
                        false,
                    )
                    .map(|_| KValue::Null);
            }

            let control_flow = if self.has_pending_operation() {
                self.poll_pending_operation()
            } else {
                let instruction_ip = self.ip();
                let Some(instruction) = self.reader.next() else {
                    break;
                };
                self.instruction_ip = instruction_ip;
                self.execute_instruction(instruction)
            };

            match control_flow {
                Ok(ControlFlow::Continue) => {}
                Ok(ControlFlow::Return(value)) => {
                    self.execution_state = ExecutionState::Inactive;
                    return Ok(value);
                }
                Ok(ControlFlow::Yield(value)) => {
                    self.execution_state = ExecutionState::Suspended;
                    return Ok(value);
                }
                Ok(ControlFlow::Pending) => {
                    self.execution_state = ExecutionState::Pending;
                    return Ok(KValue::Null);
                }
                Err(error) => match self.pop_call_stack_on_error(error.clone(), true) {
                    Ok((recover_register, ip)) => {
                        let catch_value = match error.error {
                            ErrorKind::KotoError { thrown_value, .. } => thrown_value,
                            _ => KValue::Str(error.to_string().into()),
                        };

                        self.set_register(recover_register, catch_value);
                        self.set_ip(ip);
                    }
                    Err(mut error) => {
                        // The error hasn't been caught, so is being propagated outside of this.
                        // Koto errors need a VM to allow the error value to be displayed,
                        // so spawn one now.
                        if let ErrorKind::KotoError { vm, .. } = &mut error.error {
                            *vm = Some(self.spawn_shared_vm().into());
                        }
                        self.execution_state = ExecutionState::Inactive;
                        return Err(error);
                    }
                },
            }

            self.instruction_ip = self.ip();
        }

        self.execution_state = ExecutionState::Inactive;
        Ok(KValue::Null)
    }

    fn execute_instruction(&mut self, instruction: Instruction) -> Result<ControlFlow> {
        use Instruction::*;

        let mut control_flow = ControlFlow::Continue;

        macro_rules! run_and_poll_implicit_task {
            ($register:expr, $run:expr) => {{
                let old_frame_count = self.call_stack.len();
                $run?;
                if self.call_stack.len() == old_frame_count {
                    control_flow = self.poll_implicit_task_in_register($register)?;
                }
            }};
        }

        match instruction {
            Error { message } => runtime_error!(message)?,
            NewFrame { register_count } => {
                self.frame_mut().required_registers = register_count;
                self.min_frame_registers = self.register_base + register_count as usize;
                self.registers
                    .resize(self.min_frame_registers, KValue::Null);
            }
            Copy { target, source } => self.set_register(target, self.clone_register(source)),
            SetNull { register } => self.set_register(register, KValue::Null),
            SetBool { register, value } => self.set_register(register, value.into()),
            SetNumber { register, value } => self.set_register(register, value.into()),
            LoadFloat { register, constant } => {
                let n = self.reader.chunk.constants.get_f64(constant);
                self.set_register(register, n.into());
            }
            LoadInt { register, constant } => {
                let n = self.reader.chunk.constants.get_i64(constant);
                self.set_register(register, n.into());
            }
            LoadString { register, constant } => {
                let string = self.koto_string_from_constant(constant);
                self.set_register(register, string.into());
            }
            LoadNonLocal { register, constant } => self.run_load_non_local(register, constant)?,
            ExportValue { key, value } => self.run_export_value(key, value)?,
            ExportEntry { entry } => self.run_export_entry(entry)?,
            Import {
                register,
                allow_pending,
            } => control_flow = self.run_import(register, false, allow_pending)?,
            ImportAll {
                register,
                allow_pending,
            } => control_flow = self.run_import(register, true, allow_pending)?,
            MakeTempTuple {
                register,
                start,
                count,
            } => self.set_register(
                register,
                KValue::TemporaryTuple(RegisterSlice {
                    start: self.register_index(start),
                    count: count as usize,
                }),
            ),
            TempTupleToTuple { register, source } => {
                self.run_temp_tuple_to_tuple(register, source)?
            }
            MakeMap {
                register,
                size_hint,
            } => self.set_register(register, KMap::with_capacity(size_hint as usize).into()),
            SequenceStart { size_hint } => self
                .sequence_builders
                .push(Vec::with_capacity(size_hint as usize)),
            SequencePush { value } => self.run_sequence_push(value)?,
            SequencePushN { start, count } => {
                for value_register in start..(start + count) {
                    self.run_sequence_push(value_register)?;
                }
            }
            SequenceToList { register } => self.run_sequence_to_list(register)?,
            SequenceToTuple { register } => self.run_sequence_to_tuple(register)?,
            StringStart { size_hint } => self
                .string_builders
                .push(String::with_capacity(size_hint as usize)),
            StringPush {
                value,
                format_options,
            } => control_flow = self.run_string_push(value, format_options)?,
            StringFinish { register } => self.run_string_finish(register)?,
            Range {
                register,
                start,
                end,
            } => self.run_make_range(register, Some(start), Some(end), false)?,
            RangeInclusive {
                register,
                start,
                end,
            } => self.run_make_range(register, Some(start), Some(end), true)?,
            RangeTo { register, end } => self.run_make_range(register, None, Some(end), false)?,
            RangeToInclusive { register, end } => {
                self.run_make_range(register, None, Some(end), true)?
            }
            RangeFrom { register, start } => {
                self.run_make_range(register, Some(start), None, false)?
            }
            RangeFull { register } => self.run_make_range(register, None, None, false)?,
            MakeIterator { register, iterable } => {
                run_and_poll_implicit_task!(
                    register,
                    self.run_make_iterator(register, iterable, true)
                );
            }
            Function { .. } => self.run_make_function(instruction)?,
            Capture {
                function,
                target,
                source,
            } => self.run_capture_value(function, target, source)?,
            Negate { register, value } => {
                run_and_poll_implicit_task!(register, self.run_negate(register, value));
            }
            Not { register, value } => self.run_not(register, value)?,
            Add { register, lhs, rhs } => {
                run_and_poll_implicit_task!(register, self.run_add(register, lhs, rhs));
            }
            Subtract { register, lhs, rhs } => {
                run_and_poll_implicit_task!(register, self.run_subtract(register, lhs, rhs));
            }
            Multiply { register, lhs, rhs } => {
                run_and_poll_implicit_task!(register, self.run_multiply(register, lhs, rhs));
            }
            Divide { register, lhs, rhs } => {
                run_and_poll_implicit_task!(register, self.run_divide(register, lhs, rhs));
            }
            Remainder { register, lhs, rhs } => {
                run_and_poll_implicit_task!(register, self.run_remainder(register, lhs, rhs));
            }
            Power { register, lhs, rhs } => {
                run_and_poll_implicit_task!(register, self.run_power(register, lhs, rhs));
            }
            AddAssign { lhs, rhs } => self.run_add_assign(lhs, rhs)?,
            SubtractAssign { lhs, rhs } => self.run_subtract_assign(lhs, rhs)?,
            MultiplyAssign { lhs, rhs } => self.run_multiply_assign(lhs, rhs)?,
            DivideAssign { lhs, rhs } => self.run_divide_assign(lhs, rhs)?,
            RemainderAssign { lhs, rhs } => self.run_remainder_assign(lhs, rhs)?,
            PowerAssign { lhs, rhs } => self.run_power_assign(lhs, rhs)?,
            Less { register, lhs, rhs } => {
                run_and_poll_implicit_task!(register, self.run_less(register, lhs, rhs));
            }
            LessOrEqual { register, lhs, rhs } => {
                run_and_poll_implicit_task!(register, self.run_less_or_equal(register, lhs, rhs));
            }
            Greater { register, lhs, rhs } => {
                run_and_poll_implicit_task!(register, self.run_greater(register, lhs, rhs));
            }
            GreaterOrEqual { register, lhs, rhs } => {
                run_and_poll_implicit_task!(
                    register,
                    self.run_greater_or_equal(register, lhs, rhs)
                );
            }
            Equal { register, lhs, rhs } => {
                run_and_poll_implicit_task!(register, self.run_equal(register, lhs, rhs));
            }
            NotEqual { register, lhs, rhs } => {
                run_and_poll_implicit_task!(register, self.run_not_equal(register, lhs, rhs));
            }
            Jump { offset } => self.jump_ip(offset as u32),
            JumpBack { offset } => self.jump_ip_back(offset as u32),
            JumpIfTrue { register, offset } => self.run_jump_if_true(register, offset as u32)?,
            JumpIfFalse { register, offset } => self.run_jump_if_false(register, offset as u32)?,
            JumpIfNull { register, offset } => self.run_jump_if_null(register, offset as u32)?,
            Call {
                result,
                function,
                frame_base,
                arg_count,
                packed_arg_count: unpacked_arg_count,
            } => {
                control_flow = self.call_or_resume_native(
                    CallInfo {
                        result_register: Some(result),
                        frame_base,
                        instance: None,
                        arg_count,
                        packed_arg_count: unpacked_arg_count,
                    },
                    self.clone_register(function),
                )?;
            }
            CallInstance {
                result,
                function,
                instance,
                frame_base,
                arg_count,
                packed_arg_count: unpacked_arg_count,
            } => {
                control_flow = self.call_or_resume_native(
                    CallInfo {
                        result_register: Some(result),
                        frame_base,
                        instance: Some(instance),
                        arg_count,
                        packed_arg_count: unpacked_arg_count,
                    },
                    self.clone_register(function),
                )?;
            }
            Return { register } => {
                if let Some(return_value) = self.pop_frame(self.clone_register(register))? {
                    // If pop_frame returns a new return_value, then execution should stop.
                    control_flow = ControlFlow::Return(return_value);
                }
            }
            Yield { register } => control_flow = ControlFlow::Yield(self.clone_register(register)),
            Await { register } => control_flow = self.run_await(register)?,
            Throw { register } => {
                return Err(crate::Error::from_koto_value(self.clone_register(register)));
            }
            Size { register, value } => {
                run_and_poll_implicit_task!(register, self.run_size(register, value, false));
            }
            IterNext {
                result,
                iterator,
                jump_offset,
                temporary_output,
            } => {
                control_flow =
                    self.run_iterator_next(result, iterator, jump_offset, temporary_output)?;
                if matches!(control_flow, ControlFlow::Pending) {
                    self.set_ip(self.instruction_ip);
                }
            }
            TempIndex {
                register,
                value,
                index,
            } => {
                run_and_poll_implicit_task!(register, self.run_temp_index(register, value, index));
            }
            SliceFrom {
                register,
                value,
                index,
            } => {
                let old_frame_count = self.call_stack.len();
                control_flow = self.run_slice(register, value, index, false)?;
                if matches!(control_flow, ControlFlow::Continue)
                    && self.call_stack.len() == old_frame_count
                {
                    control_flow = self.poll_implicit_task_in_register(register)?;
                }
            }
            SliceTo {
                register,
                value,
                index,
            } => {
                let old_frame_count = self.call_stack.len();
                control_flow = self.run_slice(register, value, index, true)?;
                if matches!(control_flow, ControlFlow::Continue)
                    && self.call_stack.len() == old_frame_count
                {
                    control_flow = self.poll_implicit_task_in_register(register)?;
                }
            }
            Index {
                register,
                value,
                index,
            } => {
                run_and_poll_implicit_task!(register, self.run_index(register, value, index));
            }
            IndexMut {
                register,
                index,
                value,
            } => control_flow = self.run_index_assign(None, register, index, value)?,
            AccessAssign {
                register,
                key,
                value,
            } => control_flow = self.run_access_assign(None, register, key, value)?,
            MetaInsert {
                register,
                value,
                id,
            } => self.run_meta_insert(register, value, id)?,
            MetaInsertNamed {
                register,
                value,
                id,
                name,
            } => self.run_meta_insert_named(register, value, id, name)?,
            MetaExport { value, id } => self.run_meta_export(value, id)?,
            MetaExportNamed { id, name, value } => self.run_meta_export_named(id, name, value)?,
            Access {
                register,
                value,
                key,
            } => {
                run_and_poll_implicit_task!(
                    register,
                    self.run_access(register, value, self.koto_string_from_constant(key))
                );
            }
            TryAccess {
                register,
                value,
                key,
                jump_offset,
            } => {
                run_and_poll_implicit_task!(
                    register,
                    self.run_try_access(
                        register,
                        value,
                        self.koto_string_from_constant(key),
                        jump_offset as u32,
                    )
                );
            }
            AccessString {
                register,
                value,
                key,
            } => {
                let key_string = match self.clone_register(key) {
                    KValue::Str(s) => s,
                    other => return unexpected_type("a String", &other),
                };
                run_and_poll_implicit_task!(register, self.run_access(register, value, key_string));
            }
            TryAccessString {
                register,
                value,
                key,
                jump_offset,
            } => {
                let key_string = match self.clone_register(key) {
                    KValue::Str(s) => s,
                    other => return unexpected_type("a String", &other),
                };
                run_and_poll_implicit_task!(
                    register,
                    self.run_try_access(register, value, key_string, jump_offset as u32)
                );
            }
            TryStart {
                arg_register,
                catch_offset,
            } => {
                let catch_ip = self.ip() + catch_offset as u32;
                self.frame_mut().catch_stack.push((arg_register, catch_ip));
            }
            TryEnd => {
                self.frame_mut().catch_stack.pop();
            }
            Debug { register, constant } => {
                control_flow = self.run_debug_instruction(register, constant)?;
            }
            CheckSizeEqual { register, size } => {
                control_flow = self.run_check_size_equal(register, size)?;
            }
            CheckSizeMin { register, size } => {
                control_flow = self.run_check_size_min(register, size)?;
            }
            AssertType {
                value,
                allow_null,
                type_string,
            } => self.run_assert_type(value, type_string, allow_null)?,
            CheckType {
                value,
                allow_null,
                type_string,
                jump_offset,
            } => self.run_check_type(value, jump_offset as u32, type_string, allow_null)?,
        }

        Ok(control_flow)
    }

    fn run_load_non_local(&mut self, register: u8, constant_index: ConstantIndex) -> Result<()> {
        let name = self.get_constant_str(constant_index);

        let non_local = self
            .frame()
            .non_local(name)
            .or_else(|| self.context.prelude.get(name));

        if let Some(non_local) = non_local {
            self.set_register(register, non_local);
            Ok(())
        } else {
            runtime_error!("'{name}' not found")
        }
    }

    fn run_export_value(&mut self, key_register: u8, value_register: u8) -> Result<()> {
        let key = ValueKey::try_from(self.clone_register(key_register))?;
        let value = self.clone_register(value_register);
        self.exports.data_mut().insert(key, value);
        Ok(())
    }

    fn run_export_entry(&mut self, entry_register: u8) -> Result<()> {
        let maybe_entry = self.clone_register(entry_register);
        let maybe_key_value_pair = match &maybe_entry {
            KValue::Tuple(tuple) => match tuple.data() {
                [key, value] => Some((key.clone(), value.clone())),
                _ => None,
            },
            KValue::TemporaryTuple(temp_tuple) => {
                match self.register_slice_raw(temp_tuple.start, temp_tuple.count) {
                    [key, value] => Some((key.clone(), value.clone())),
                    _ => None,
                }
            }
            _ => None,
        };
        let Some((key, value)) = maybe_key_value_pair else {
            dbg!(&self.registers);
            return unexpected_type("Key/Value pair to export", &maybe_entry);
        };
        self.exports
            .data_mut()
            .insert(ValueKey::try_from(key)?, value);
        Ok(())
    }

    fn run_temp_tuple_to_tuple(&mut self, register: u8, source_register: u8) -> Result<()> {
        match self.clone_register(source_register) {
            KValue::TemporaryTuple(temp_registers) => {
                let tuple = KTuple::from(
                    self.register_slice_raw(temp_registers.start, temp_registers.count),
                );
                self.set_register(register, KValue::Tuple(tuple));
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    fn run_make_range(
        &mut self,
        register: u8,
        start_register: Option<u8>,
        end_register: Option<u8>,
        inclusive: bool,
    ) -> Result<()> {
        use KValue::Number;

        let start = start_register.map(|r| self.get_register(r));
        let end = end_register.map(|r| self.get_register(r));

        let (range_start, range_end) = match (start, end) {
            (Some(Number(start)), Some(Number(end))) => {
                (Some(start.into()), Some((end.into(), inclusive)))
            }
            (Some(Number(start)), None) => (Some(start.into()), None),
            (None, Some(Number(end))) => (None, Some((end.into(), inclusive))),
            (None, None) => (None, None),
            (None | Some(Number(_)), Some(unexpected)) => {
                return unexpected_type("a Number for the range's end", unexpected);
            }
            (Some(unexpected), _) => {
                return unexpected_type("a Number for the range's start", unexpected);
            }
        };

        self.set_register(register, KRange::new(range_start, range_end).into());
        Ok(())
    }

    // Runs the MakeIterator instruction
    //
    // This function is distinct from the public `make_iterator`, which will defer to this function
    // when the input value implements @iterator.
    //
    // `temp_iterator` is used for temporary unpacking operations.
    fn run_make_iterator(
        &mut self,
        result_register: u8,
        iterable_register: u8,
        temp_iterator: bool,
    ) -> Result<()> {
        use KValue::*;

        let value = self.clone_register(iterable_register);

        let result = match value {
            Map(ref map) if map.contains_meta_key(&UnaryOp::Next.into()) => {
                KIterator::with_meta_next(self.spawn_shared_vm(), value)?.into()
            }
            Map(ref map) if map.contains_meta_key(&UnaryOp::Iterator.into()) => {
                let Some(op) = map.get_meta_value(&UnaryOp::Iterator.into()) else {
                    unreachable!()
                };
                if op.is_callable() || op.is_generator() {
                    return self.call_overridden_op_1(Some(result_register), iterable_register, op);
                } else {
                    return unexpected_type("callable function from @iterator", &op);
                }
            }
            Iterator(_) => value,
            Range(ref r) if temp_iterator && r.is_bounded() => value,
            Tuple(_) | Str(_) | TemporaryTuple(_) if temp_iterator => {
                // Immutable sequences can be iterated over directly when used in temporary
                // situations like argument unpacking.
                value
            }
            Range(range) => KIterator::with_range(range)?.into(),
            List(list) => KIterator::with_list(list).into(),
            Tuple(tuple) => KIterator::with_tuple(tuple).into(),
            Str(s) => KIterator::with_string(s).into(),
            Map(map) => KIterator::with_map(map).into(),
            Object(o) => {
                use IsIterable::*;
                let o_inner = o.try_borrow()?;
                match o_inner.is_iterable() {
                    NotIterable => KIterator::once(o.clone().into())?.into(),
                    Iterable => o_inner.make_iterator(self)?.into(),
                    ForwardIterator | BidirectionalIterator => {
                        KIterator::with_object(self.spawn_shared_vm(), o.clone())?.into()
                    }
                }
            }
            _ => {
                // Single values become 'once' iterators
                // This behaviour differs from the public `make_iterator` behaviour which expects
                // that the value is iterable.
                KIterator::once(value)?.into()
            }
        };

        self.set_register(result_register, result);
        Ok(())
    }

    fn run_iterator_next(
        &mut self,
        result_register: Option<u8>,
        iterable_register: u8,
        jump_offset: u16,
        output_is_temporary: bool,
    ) -> Result<ControlFlow> {
        use KValue::*;

        // Temporary iterators need to be removed from the register so that they can be mutated in
        // place (there should be no other references), and then returned to the iterator.
        let iterable_is_temporary = matches!(
            self.get_register(iterable_register),
            Range(_) | Tuple(_) | Str(_) | TemporaryTuple { .. }
        );

        let output = if iterable_is_temporary {
            let (output, new_iterable) = match self.remove_register(iterable_register) {
                Range(mut r) => {
                    let output = r.pop_front()?;
                    (output.map(KValue::from), Range(r))
                }
                Tuple(mut t) => {
                    let output = t.pop_front();
                    (output, Tuple(t))
                }
                Str(mut s) => {
                    let output = s.pop_front();
                    (output.map(KValue::from), Str(s))
                }
                TemporaryTuple(RegisterSlice { start, count }) => {
                    if count > 0 {
                        (
                            Some(self.registers[start].clone()),
                            TemporaryTuple(RegisterSlice {
                                start: start + 1,
                                count: count - 1,
                            }),
                        )
                    } else {
                        (None, TemporaryTuple(RegisterSlice { start, count }))
                    }
                }
                _ => {
                    // The match arms here match the arms when calculating iterable_is_temporary
                    unreachable!()
                }
            };

            self.set_register(iterable_register, new_iterable);
            IterationStep::Output(output)
        } else {
            match self.clone_register(iterable_register) {
                Iterator(mut iterator) => match if let Some(waker) = self.task_waker.clone() {
                    let mut context = Context::from_waker(&waker);
                    iterator.next_output_with_context(&mut context)
                } else {
                    iterator.next_output()
                } {
                    KIteratorNext::Output(output) => match output {
                        KIteratorOutput::Value(value) => IterationStep::Output(Some(value)),
                        KIteratorOutput::ValuePair(first, second) => {
                            if let Some(result) = result_register {
                                if output_is_temporary {
                                    // Place the value pair in a temporary tuple following the
                                    // result register. The assumption here is that the values
                                    // following the result register are available for re-use,
                                    // if that turns out to not be true in all cases then a
                                    // different approach will be needed.
                                    let start = result + 1;
                                    let first_index = self.register_index(start);
                                    let second_index = first_index + 1;
                                    if second_index >= self.registers.len() {
                                        self.registers.resize(second_index + 1, KValue::Null);
                                    }
                                    self.registers[first_index] = first;
                                    self.registers[second_index] = second;
                                    IterationStep::Output(Some(TemporaryTuple(RegisterSlice {
                                        start: first_index,
                                        count: 2,
                                    })))
                                } else {
                                    IterationStep::Output(Some(Tuple(vec![first, second].into())))
                                }
                            } else {
                                // The output is going to be ignored, but we use Some here to
                                // indicate that iteration should continue.
                                IterationStep::Output(Some(Null))
                            }
                        }
                        KIteratorOutput::Error(error) => {
                            return runtime_error!(error.to_string());
                        }
                    },
                    KIteratorNext::Pending => IterationStep::Pending,
                    KIteratorNext::Done => IterationStep::Output(None),
                },
                Map(m) if m.contains_meta_key(&UnaryOp::Next.into()) => {
                    let op = m.get_meta_value(&UnaryOp::Next.into()).unwrap();
                    if !op.is_callable() {
                        return unexpected_type("Callable function from @next", &op);
                    }

                    let old_frame_count = self.call_stack.len();
                    let register_len_before_temp = self.registers.len();
                    let (op_result_register, op_result_is_temporary) = match result_register {
                        Some(register) => (register, false),
                        None => {
                            let register = self.next_register();
                            self.registers.push(KValue::Null);
                            (register, true)
                        }
                    };
                    let register_len_before_call = self.registers.len();

                    self.call_overridden_op_1(Some(op_result_register), iterable_register, op)?;

                    let output = if self.call_stack.len() == old_frame_count {
                        let output = self.clone_register(op_result_register);
                        self.registers.truncate(if op_result_is_temporary {
                            register_len_before_temp
                        } else {
                            register_len_before_call
                        });
                        output
                    } else {
                        self.frame_mut().execution_barrier = true;
                        match self.execute_instructions() {
                            Ok(_) if matches!(self.execution_state, ExecutionState::Pending) => {
                                return Ok(ControlFlow::Pending);
                            }
                            Ok(output) => {
                                if op_result_is_temporary {
                                    self.registers.truncate(register_len_before_temp);
                                }
                                output
                            }
                            Err(error) => {
                                self.pop_frame(KValue::Null)?;
                                return Err(error);
                            }
                        }
                    };

                    match output {
                        Null => IterationStep::Output(None),
                        output => IterationStep::Output(Some(output)),
                    }
                }
                unexpected => return unexpected_type("Iterator", &unexpected),
            }
        };

        match (output, result_register) {
            (IterationStep::Output(Some(output)), Some(register)) => {
                self.set_register(register, output);
            }
            (IterationStep::Output(Some(_)), None) => {
                // No result register, so the output can be discarded
            }
            (IterationStep::Output(None), Some(register)) => {
                // The iterator is finished, so jump to the provided offset
                self.set_register(register, Null);
                self.jump_ip(jump_offset as u32);
            }
            (IterationStep::Output(None), None) => {
                self.jump_ip(jump_offset as u32);
            }
            (IterationStep::Pending, _) => return Ok(ControlFlow::Pending),
        }

        Ok(ControlFlow::Continue)
    }

    fn run_temp_index(&mut self, result: u8, value: u8, index: i8) -> Result<()> {
        use KValue::*;

        let index_op = ReadOp::Index.into();
        let lhs = self.get_register(value);

        let result_value = match lhs {
            List(list) => {
                let index = signed_index_to_unsigned(index, list.data().len());
                list.data().get(index).cloned().unwrap_or(Null)
            }
            Tuple(tuple) => {
                let index = signed_index_to_unsigned(index, tuple.len());
                tuple.get(index).cloned().unwrap_or(Null)
            }
            TemporaryTuple(RegisterSlice { start, count }) => {
                let count = *count;
                if (index.unsigned_abs() as usize) < count {
                    let index = signed_index_to_unsigned(index, count);
                    self.registers[start + index].clone()
                } else {
                    Null
                }
            }
            Str(s) => {
                let index = signed_index_to_unsigned(index, s.len());
                s.with_bounds(index..index + 1).into()
            }
            Range(r) => {
                let result: KNumber = if index < 0 {
                    let Some((end, inclusive)) = r.end() else {
                        return runtime_error!(
                            "Unable to index a {} with {}",
                            lhs.type_as_string(),
                            index
                        );
                    };

                    let end = if inclusive { end + 1 } else { end };
                    end + index as i64
                } else {
                    let Some(start) = r.start() else {
                        return runtime_error!(
                            "Unable to index a {} with {}",
                            lhs.type_as_string(),
                            index
                        );
                    };
                    start + index as i64
                }
                .into();

                if r.contains(result) {
                    result.into()
                } else {
                    Null
                }
            }
            Map(map) if map.contains_meta_key(&index_op) => {
                let op = map.get_meta_value(&index_op).unwrap();
                let lhs = lhs.clone();
                return self.call_overridden_op_2(Some(result), lhs, index.into(), op);
            }
            Map(map) => {
                let data = map.data();
                let index = signed_index_to_unsigned(index, data.len());
                match data.get_index(index) {
                    Some((key, value)) => Tuple(vec![key.value().clone(), value.clone()].into()),
                    None => Null,
                }
            }
            value @ Object(o) => {
                let o = o.try_borrow()?;
                if let Some(size) = o.size() {
                    let index = signed_index_to_unsigned(index, size);
                    o.index(&index.into())?
                } else {
                    return unexpected_type("a value with a defined size", value);
                }
            }
            unexpected => return unexpected_type("an indexable value", unexpected),
        };

        self.set_register(result, result_value);

        Ok(())
    }

    fn run_slice(
        &mut self,
        register: u8,
        value: u8,
        index: i8,
        is_slice_to: bool,
    ) -> Result<ControlFlow> {
        use KValue::*;

        let index_op = ReadOp::Index.into();

        let result = match self.clone_register(value) {
            List(list) => {
                let index = signed_index_to_unsigned(index, list.data().len());
                if is_slice_to {
                    list.data()
                        .get(..index)
                        .map_or(Null, |entries| List(KList::from_slice(entries)))
                } else {
                    list.data()
                        .get(index..)
                        .map_or(Null, |entries| List(KList::from_slice(entries)))
                }
            }
            Tuple(tuple) => {
                let index = signed_index_to_unsigned(index, tuple.len());
                if is_slice_to {
                    tuple.make_sub_tuple(0..index).into()
                } else {
                    tuple.make_sub_tuple(index..tuple.len()).into()
                }
            }
            Str(s) => {
                let index = signed_index_to_unsigned(index, s.len());
                if is_slice_to {
                    s.with_bounds(0..index).into()
                } else {
                    s.with_bounds(index..s.len()).into()
                }
            }
            Map(m) if m.contains_meta_key(&index_op) => {
                match self.run_unary_op(UnaryOp::Size, self.clone_register(value))? {
                    VmOutput::Ready(size) => {
                        return self.finish_meta_slice(register, m, index, is_slice_to, size);
                    }
                    VmOutput::Pending(task) => {
                        self.set_pending_operation(PendingOperation::Slice(PendingSlice::Size {
                            result_register: register,
                            map: m,
                            index,
                            is_slice_to,
                            task,
                        }))?;
                        return Ok(ControlFlow::Pending);
                    }
                }
            }
            Map(m) => {
                let data = m.data();
                let index = signed_index_to_unsigned(index, data.len());
                if is_slice_to {
                    data.make_data_slice(..index)
                        .map_or(Null, |slice| KMap::with_data(slice).into())
                } else {
                    data.make_data_slice(index..)
                        .map_or(Null, |slice| KMap::with_data(slice).into())
                }
            }
            Object(o) => {
                let o = o.try_borrow()?;
                if let Some(size) = o.size() {
                    let index = signed_index_to_unsigned(index, size) as i64;
                    let range = if is_slice_to {
                        0..index
                    } else {
                        index..size as i64
                    };
                    o.index(&KRange::from(range).into())?
                } else {
                    KValue::Null
                }
            }
            unexpected => return unexpected_type("a sliceable value", &unexpected),
        };

        self.set_register(register, result);

        Ok(ControlFlow::Continue)
    }

    fn finish_meta_slice(
        &mut self,
        result_register: u8,
        map: KMap,
        index: i8,
        is_slice_to: bool,
        size: KValue,
    ) -> Result<ControlFlow> {
        let KValue::Number(size) = size else {
            return unexpected_type("number for value size", &size);
        };
        let size = usize::from(size);
        let index = signed_index_to_unsigned(index, size) as i64;
        let range = if is_slice_to {
            0..index
        } else {
            index..size as i64
        };
        let output = self.run_read_op(ReadOp::Index, map.into(), KRange::from(range).into())?;
        self.finish_meta_slice_read(result_register, output)
    }

    fn finish_meta_slice_read(
        &mut self,
        result_register: u8,
        output: VmOutput,
    ) -> Result<ControlFlow> {
        let result = match output {
            VmOutput::Ready(result) => result,
            VmOutput::Pending(task) => {
                self.set_pending_operation(PendingOperation::Slice(PendingSlice::Read {
                    result_register,
                    task,
                }))?;
                return Ok(ControlFlow::Pending);
            }
        };

        self.set_register(result_register, result);
        self.poll_implicit_task_in_register(result_register)
    }

    fn poll_pending_slice(&mut self, pending: PendingSlice) -> Result<ControlFlow> {
        let (result_register, mut task, on_ready) = match pending {
            PendingSlice::Size {
                result_register,
                map,
                index,
                is_slice_to,
                task,
            } => (
                result_register,
                task,
                PendingSliceContinuation::Size {
                    map,
                    index,
                    is_slice_to,
                },
            ),
            PendingSlice::Read {
                result_register,
                task,
            } => (result_register, task, PendingSliceContinuation::Read),
        };

        let poll_result = if let Some(waker) = self.task_waker.clone() {
            let mut context = Context::from_waker(&waker);
            task.poll_with_context(&mut context)?
        } else {
            task.poll()?
        };

        match poll_result {
            KTaskPoll::Ready(size) => match on_ready {
                PendingSliceContinuation::Size {
                    map,
                    index,
                    is_slice_to,
                } => self.finish_meta_slice(result_register, map, index, is_slice_to, size),
                PendingSliceContinuation::Read => {
                    self.finish_meta_slice_read(result_register, VmOutput::Ready(size))
                }
            },
            KTaskPoll::Pending => {
                let pending = match on_ready {
                    PendingSliceContinuation::Size {
                        map,
                        index,
                        is_slice_to,
                    } => PendingSlice::Size {
                        result_register,
                        map,
                        index,
                        is_slice_to,
                        task,
                    },
                    PendingSliceContinuation::Read => PendingSlice::Read {
                        result_register,
                        task,
                    },
                };
                self.set_pending_operation(PendingOperation::Slice(pending))?;
                Ok(ControlFlow::Pending)
            }
        }
    }

    fn run_make_function(&mut self, function_instruction: Instruction) -> Result<()> {
        match function_instruction {
            Instruction::Function {
                register,
                arg_count,
                optional_arg_count,
                capture_count,
                flags,
                size,
            } => {
                let total_captures_count = optional_arg_count + capture_count;
                let captures = if total_captures_count > 0 {
                    // Initialize the function's captures with Null
                    let mut captures = ValueVec::new();
                    captures.resize(total_captures_count as usize, KValue::Null);
                    Some(KList::with_data(captures))
                } else {
                    None
                };

                let non_locals = if flags.non_local_access() {
                    let non_locals = self.frame().non_locals.clone();
                    if non_locals.is_none() {
                        return runtime_error!(ErrorKind::UnexpectedError);
                    }
                    non_locals
                } else {
                    None
                };

                let context = if captures.is_some() || non_locals.is_some() {
                    Some(Ptr::from(FunctionContext {
                        captures,
                        non_locals,
                    }))
                } else {
                    None
                };

                let function = KFunction::new(
                    self.chunk(),
                    self.ip(),
                    arg_count,
                    optional_arg_count,
                    flags,
                    context,
                );

                self.jump_ip(size as u32);
                self.set_register(register, KValue::Function(function));
                Ok(())
            }
            _ => unreachable!(),
        }
    }

    fn run_capture_value(&mut self, function: u8, capture_index: u8, value: u8) -> Result<()> {
        let Some(function) = self.get_register_safe(function) else {
            // E.g. `x = (1..10).find |n| n == x`
            // The function was temporary and has been removed from the value stack,
            // but the capture of `x` is still attempted. It would be cleaner for the compiler to
            // detect this case but for now a runtime error will have to do.
            return runtime_error!("function not found while attempting to capture a value");
        };

        match function {
            KValue::Function(f) => {
                if let Some(captures) = f.captures() {
                    captures.data_mut()[capture_index as usize] = self.clone_register(value);
                }
                Ok(())
            }
            unexpected => unexpected_type("Function while capturing value", unexpected),
        }
    }

    fn run_await(&mut self, register: u8) -> Result<ControlFlow> {
        if let KValue::Task(mut task) = self.clone_register(register) {
            let poll_result = if let Some(waker) = self.task_waker.clone() {
                let mut context = Context::from_waker(&waker);
                self.poll_task_with_context(&mut task, &mut context)?
            } else {
                self.poll_task(&mut task)?
            };

            match poll_result {
                KTaskPoll::Ready(result) => {
                    self.set_register(register, result);
                }
                KTaskPoll::Pending => {
                    self.set_ip(self.instruction_ip);
                    return Ok(ControlFlow::Pending);
                }
            }
        }

        Ok(ControlFlow::Continue)
    }

    fn run_negate(&mut self, result: u8, value: u8) -> Result<()> {
        use KValue::*;
        use UnaryOp::Negate;

        let result_value = match self.clone_register(value) {
            Number(n) => Number(-n),
            Map(m) if m.contains_meta_key(&Negate.into()) => {
                let op = m.get_meta_value(&Negate.into()).unwrap();
                return self.call_overridden_op_1(Some(result), value, op);
            }
            Object(o) => o.try_borrow()?.negate()?,
            unexpected => return unexpected_type("negatable value", &unexpected),
        };
        self.set_register(result, result_value);

        Ok(())
    }

    fn run_not(&mut self, result: u8, value: u8) -> Result<()> {
        use KValue::*;

        let result_bool = match &self.get_register(value) {
            Null => true,
            Bool(b) if !b => true,
            _ => false, // All other values coerce to true, so return false
        };
        self.set_register(result, result_bool.into());

        Ok(())
    }

    fn run_debug_op(&mut self, result: u8, value: u8) -> Result<()> {
        use UnaryOp::Debug;

        match self.clone_register(value) {
            KValue::Map(m) if m.contains_meta_key(&Debug.into()) => {
                let op = m.get_meta_value(&Debug.into()).unwrap();
                self.call_overridden_op_1(Some(result), value, op)
            }
            other => self.display_value_with_runner(result, other, true),
        }
    }

    fn run_display(&mut self, result: u8, value: u8) -> Result<()> {
        use UnaryOp::Display;

        match self.clone_register(value) {
            KValue::Map(m) if m.contains_meta_key(&Display.into()) => {
                let op = m.get_meta_value(&Display.into()).unwrap();
                self.call_overridden_op_1(Some(result), value, op)
            }
            other => self.display_value_with_runner(result, other, false),
        }
    }

    fn run_add(&mut self, result: u8, lhs: u8, rhs: u8) -> Result<()> {
        use BinaryOp::{Add, AddRhs};
        use KValue::*;
        use macros::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);

        let result_value = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Number(a + b),
            (Str(a), Str(b)) => {
                let result = a.to_string() + b.as_ref();
                Str(result.into())
            }
            (List(a), List(b)) => {
                let result: ValueVec = a.data().iter().chain(b.data().iter()).cloned().collect();
                List(KList::with_data(result))
            }
            (Tuple(a), Tuple(b)) => {
                let result: Vec<_> = a.iter().chain(b.iter()).cloned().collect();
                Tuple(result.into())
            }
            (Map(m), _) if m.contains_meta_key(&Add.into()) => {
                let lhs_value = lhs_value.clone();
                let rhs_value = rhs_value.clone();
                call_metamap_arithmetic_op!(self, Add, add, m, lhs_value, rhs_value, result)
            }
            (Object(o), _) => {
                call_object_arithmetic_op!(self, Add, add, o, lhs_value, rhs_value, result)
            }
            (_, Map(m)) if m.contains_meta_key(&AddRhs.into()) => {
                call_metamap_binary_op_rhs!(self, AddRhs, m, lhs_value, rhs_value, result);
            }
            (_, Object(o)) => call_object_binary_op!(AddRhs, add_rhs, o, lhs_value, rhs_value),
            (Map(a), Map(b)) => {
                let mut data = a.data().clone();
                data.extend(b.data().iter().map(|(k, v)| (k.clone(), v.clone())));
                let meta = match (a.meta_map(), b.meta_map()) {
                    (None, None) => None,
                    (Some(meta_a), None) => Some(meta_a.borrow().clone()),
                    (None, Some(meta_b)) => Some(meta_b.borrow().clone()),
                    (Some(meta_a), Some(meta_b)) => {
                        let mut result = meta_a.borrow().clone();
                        result.extend(&meta_b.borrow());
                        Some(result)
                    }
                };
                Map(KMap::with_contents(data, meta))
            }
            _ => return binary_op_error(lhs_value, rhs_value, Add),
        };

        self.set_register(result, result_value);
        Ok(())
    }

    fn run_subtract(&mut self, result: u8, lhs: u8, rhs: u8) -> Result<()> {
        macros::run_arithmetic_op!(
            self,
            Subtract,
            subtract,
            |a: &KNumber, b: &KNumber| a - b,
            result,
            lhs,
            rhs
        )
    }

    fn run_multiply(&mut self, result: u8, lhs: u8, rhs: u8) -> Result<()> {
        macros::run_arithmetic_op!(
            self,
            Multiply,
            multiply,
            |a: &KNumber, b: &KNumber| a * b,
            result,
            lhs,
            rhs
        )
    }

    fn run_divide(&mut self, result: u8, lhs: u8, rhs: u8) -> Result<()> {
        macros::run_arithmetic_op!(
            self,
            Divide,
            divide,
            |a: &KNumber, b: &KNumber| a / b,
            result,
            lhs,
            rhs
        )
    }

    fn run_remainder(&mut self, result: u8, lhs: u8, rhs: u8) -> Result<()> {
        use BinaryOp::{Remainder, RemainderRhs};
        use KValue::*;
        use macros::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result_value = match (lhs_value, rhs_value) {
            (Number(_), Number(KNumber::I64(b))) if *b == 0 => {
                // Special case for integer remainder when the divisor is zero,
                // avoid a panic and return NaN instead.
                Number(f64::NAN.into())
            }
            (Number(a), Number(b)) => Number(a % b),
            (Map(m), _) if m.contains_meta_key(&Remainder.into()) => {
                let lhs_value = lhs_value.clone();
                let rhs_value = rhs_value.clone();
                call_metamap_arithmetic_op!(
                    self, Remainder, remainder, m, lhs_value, rhs_value, result
                )
            }
            (Object(o), _) => {
                call_object_arithmetic_op!(
                    self, Remainder, remainder, o, lhs_value, rhs_value, result
                )
            }
            (_, Map(m)) if m.contains_meta_key(&RemainderRhs.into()) => {
                call_metamap_binary_op_rhs!(self, RemainderRhs, m, lhs_value, rhs_value, result);
            }
            (_, Object(o)) => {
                call_object_binary_op!(RemainderRhs, remainder_rhs, o, lhs_value, rhs_value)
            }
            _ => return binary_op_error(lhs_value, rhs_value, Remainder),
        };
        self.set_register(result, result_value);

        Ok(())
    }

    fn run_power(&mut self, result: u8, lhs: u8, rhs: u8) -> Result<()> {
        macros::run_arithmetic_op!(
            self,
            Power,
            power,
            |a: &KNumber, b: &KNumber| a.pow(*b),
            result,
            lhs,
            rhs
        )
    }

    fn run_add_assign(&mut self, lhs: u8, rhs: u8) -> Result<()> {
        macros::run_compound_assign_op!(
            self,
            AddAssign,
            add_assign,
            |a: &KNumber, b: &KNumber| a + b,
            lhs,
            rhs
        )
    }

    fn run_subtract_assign(&mut self, lhs: u8, rhs: u8) -> Result<()> {
        macros::run_compound_assign_op!(
            self,
            SubtractAssign,
            subtract_assign,
            |a: &KNumber, b: &KNumber| a - b,
            lhs,
            rhs
        )
    }

    fn run_multiply_assign(&mut self, lhs: u8, rhs: u8) -> Result<()> {
        macros::run_compound_assign_op!(
            self,
            MultiplyAssign,
            multiply_assign,
            |a: &KNumber, b: &KNumber| a * b,
            lhs,
            rhs
        )
    }

    fn run_divide_assign(&mut self, lhs: u8, rhs: u8) -> Result<()> {
        macros::run_compound_assign_op!(
            self,
            DivideAssign,
            divide_assign,
            |a: &KNumber, b: &KNumber| a / b,
            lhs,
            rhs
        )
    }

    fn run_remainder_assign(&mut self, lhs: u8, rhs: u8) -> Result<()> {
        macros::run_compound_assign_op!(
            self,
            RemainderAssign,
            remainder_assign,
            |a: &KNumber, b: &KNumber| a % b,
            lhs,
            rhs
        )
    }

    fn run_power_assign(&mut self, lhs: u8, rhs: u8) -> Result<()> {
        macros::run_compound_assign_op!(
            self,
            PowerAssign,
            power_assign,
            |a: &KNumber, b: &KNumber| a.pow(*b),
            lhs,
            rhs
        )
    }

    fn run_less(&mut self, result: u8, lhs: u8, rhs: u8) -> Result<()> {
        use BinaryOp::Less;
        use KValue::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result_value = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Bool(a < b),
            (Str(a), Str(b)) => Bool(a.as_str() < b.as_str()),
            (Map(m), _) if m.contains_meta_key(&Less.into()) => {
                macros::call_metamap_binary_op!(self, Less, m, lhs_value, rhs_value, result);
            }
            (Object(o), _) => o.try_borrow()?.less(rhs_value)?.into(),
            _ => return binary_op_error(lhs_value, rhs_value, Less),
        };
        self.set_register(result, result_value);

        Ok(())
    }

    fn run_less_or_equal(&mut self, result: u8, lhs: u8, rhs: u8) -> Result<()> {
        use BinaryOp::{Equal, Less, LessOrEqual};
        use KValue::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result_value = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Bool(a <= b),
            (Str(a), Str(b)) => Bool(a.as_str() <= b.as_str()),
            (Map(m), _) if m.contains_meta_key(&LessOrEqual.into()) => {
                macros::call_metamap_binary_op!(self, LessOrEqual, m, lhs_value, rhs_value, result);
            }
            (Map(m), _)
                if m.contains_meta_key(&Less.into()) && m.contains_meta_key(&Equal.into()) =>
            {
                let lhs_value = lhs_value.clone();
                let rhs_value = rhs_value.clone();
                let less_op = m.get_meta_value(&Less.into()).unwrap();
                let equal_op = m.get_meta_value(&Equal.into()).unwrap();
                return self.run_overridden_comparison_fallback(
                    result,
                    lhs_value,
                    rhs_value,
                    less_op,
                    Some(equal_op),
                    ComparisonFallback::LessOrEqual,
                );
            }
            (Object(o), _) => o.try_borrow()?.less_or_equal(rhs_value)?.into(),
            _ => return binary_op_error(lhs_value, rhs_value, LessOrEqual),
        };
        self.set_register(result, result_value);

        Ok(())
    }

    fn run_greater(&mut self, result: u8, lhs: u8, rhs: u8) -> Result<()> {
        use BinaryOp::{Equal, Greater, Less};
        use KValue::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result_value = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Bool(a > b),
            (Str(a), Str(b)) => Bool(a.as_str() > b.as_str()),
            (Map(m), _) if m.contains_meta_key(&Greater.into()) => {
                macros::call_metamap_binary_op!(self, Greater, m, lhs_value, rhs_value, result);
            }
            (Map(m), _)
                if m.contains_meta_key(&Less.into()) && m.contains_meta_key(&Equal.into()) =>
            {
                let lhs_value = lhs_value.clone();
                let rhs_value = rhs_value.clone();
                let less_op = m.get_meta_value(&Less.into()).unwrap();
                let equal_op = m.get_meta_value(&Equal.into()).unwrap();
                return self.run_overridden_comparison_fallback(
                    result,
                    lhs_value,
                    rhs_value,
                    less_op,
                    Some(equal_op),
                    ComparisonFallback::Greater,
                );
            }
            (Object(o), _) => o.try_borrow()?.greater(rhs_value)?.into(),
            _ => return binary_op_error(lhs_value, rhs_value, Greater),
        };
        self.set_register(result, result_value);

        Ok(())
    }

    fn run_greater_or_equal(&mut self, result: u8, lhs: u8, rhs: u8) -> Result<()> {
        use BinaryOp::{GreaterOrEqual, Less};
        use KValue::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result_value = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Bool(a >= b),
            (Str(a), Str(b)) => Bool(a.as_str() >= b.as_str()),
            (Map(m), _) if m.contains_meta_key(&GreaterOrEqual.into()) => {
                use macros::call_metamap_binary_op;
                call_metamap_binary_op!(self, GreaterOrEqual, m, lhs_value, rhs_value, result);
            }
            (Map(m), _) if m.contains_meta_key(&Less.into()) => {
                let lhs_value = lhs_value.clone();
                let rhs_value = rhs_value.clone();
                let less_op = m.get_meta_value(&Less.into()).unwrap();
                return self.run_overridden_comparison_fallback(
                    result,
                    lhs_value,
                    rhs_value,
                    less_op,
                    None,
                    ComparisonFallback::GreaterOrEqual,
                );
            }
            (Object(o), _) => o.try_borrow()?.greater_or_equal(rhs_value)?.into(),
            _ => return binary_op_error(lhs_value, rhs_value, GreaterOrEqual),
        };
        self.set_register(result, result_value);

        Ok(())
    }

    fn run_equal(&mut self, result: u8, lhs: u8, rhs: u8) -> Result<()> {
        use BinaryOp::Equal;
        use KValue::*;
        use macros::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result_value = match (lhs_value, rhs_value) {
            (Null, Null) => true,
            (Null, _) | (_, Null) => false,
            (Number(a), Number(b)) => a == b,
            (Bool(a), Bool(b)) => a == b,
            (Str(a), Str(b)) => a == b,
            (Range(a), Range(b)) => a == b,
            (List(a), List(b)) => {
                let a = a.clone();
                let b = b.clone();
                let data_a = a.data();
                let data_b = b.data();
                return self.run_value_range_equality(result, &data_a, &data_b, false);
            }
            (Tuple(a), Tuple(b)) => {
                let a = a.clone();
                let b = b.clone();
                return self.run_value_range_equality(result, &a, &b, false);
            }
            (Map(m), _) if m.contains_meta_key(&Equal.into()) => {
                call_metamap_binary_op!(self, Equal, m, lhs_value, rhs_value, result);
            }
            (Map(map), _) => {
                if let Map(rhs_map) = rhs_value {
                    let a = map.clone();
                    let b = rhs_map.clone();
                    return self.run_value_map_equality(result, a, b, false);
                } else {
                    false
                }
            }
            (Object(o), _) => o.try_borrow()?.equal(rhs_value)?,
            (Function(a), Function(b)) => {
                let a = a.clone();
                let b = b.clone();
                return self.run_function_equality(result, a, b, false);
            }
            _ => false,
        };

        self.set_register(result, result_value.into());

        Ok(())
    }

    fn run_not_equal(&mut self, result: u8, lhs: u8, rhs: u8) -> Result<()> {
        use BinaryOp::{Equal, NotEqual};
        use KValue::*;
        use macros::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result_value = match (lhs_value, rhs_value) {
            (Null, Null) => false,
            (Null, _) | (_, Null) => true,
            (Number(a), Number(b)) => a != b,
            (Bool(a), Bool(b)) => a != b,
            (Str(a), Str(b)) => a != b,
            (Range(a), Range(b)) => a != b,
            (List(a), List(b)) => {
                let a = a.clone();
                let b = b.clone();
                let data_a = a.data();
                let data_b = b.data();
                return self.run_value_range_equality(result, &data_a, &data_b, true);
            }
            (Tuple(a), Tuple(b)) => {
                let a = a.clone();
                let b = b.clone();
                return self.run_value_range_equality(result, &a, &b, true);
            }
            (Map(m), _) if m.contains_meta_key(&NotEqual.into()) => {
                call_metamap_binary_op!(self, NotEqual, m, lhs_value, rhs_value, result);
            }
            (Map(m), _) if m.contains_meta_key(&Equal.into()) => {
                let op = m.get_meta_value(&Equal.into()).unwrap();
                return self.run_overridden_comparison(
                    result,
                    lhs_value.clone(),
                    rhs_value.clone(),
                    op,
                    true,
                );
            }
            (Map(map), _) => {
                if let Map(rhs_map) = rhs_value {
                    let a = map.clone();
                    let b = rhs_map.clone();
                    return self.run_value_map_equality(result, a, b, true);
                } else {
                    true
                }
            }
            (Object(o), _) => o.try_borrow()?.not_equal(rhs_value)?,
            (Function(a), Function(b)) => {
                let a = a.clone();
                let b = b.clone();
                return self.run_function_equality(result, a, b, true);
            }
            _ => true,
        };
        self.set_register(result, result_value.into());

        Ok(())
    }

    fn run_function_equality(
        &mut self,
        result: u8,
        a: KFunction,
        b: KFunction,
        invert: bool,
    ) -> Result<()> {
        if a.chunk == b.chunk && a.ip == b.ip {
            match (a.captures(), b.captures()) {
                (None, None) => self.set_register(result, (!invert).into()),
                (Some(captures_a), Some(captures_b)) => {
                    let captures_a = captures_a.clone();
                    let captures_b = captures_b.clone();
                    let data_a = captures_a.data();
                    let data_b = captures_b.data();
                    return self.run_value_range_equality(result, &data_a, &data_b, invert);
                }
                _ => self.set_register(result, invert.into()),
            }
        } else {
            self.set_register(result, invert.into());
        }

        Ok(())
    }

    fn run_value_range_equality(
        &mut self,
        result: u8,
        range_a: &[KValue],
        range_b: &[KValue],
        invert: bool,
    ) -> Result<()> {
        if range_a.len() != range_b.len() {
            self.set_register(result, invert.into());
            return Ok(());
        }

        let range_a = range_a.iter().cloned().collect::<ValueVec>();
        let range_b = range_b.iter().cloned().collect::<ValueVec>();
        let mut runner = self.spawn_async_vm();
        let future = async move {
            let is_equal = compare_value_ranges(&mut runner, range_a, range_b).await?;
            Ok(KValue::Bool(is_equal != invert))
        };

        self.poll_task_into_register(result, KTask::with_future(future))
    }

    fn run_value_map_equality(
        &mut self,
        result: u8,
        map_a: KMap,
        map_b: KMap,
        invert: bool,
    ) -> Result<()> {
        if map_a.len() != map_b.len() {
            self.set_register(result, invert.into());
            return Ok(());
        }

        let map_a = map_a.data().clone();
        let map_b = map_b.data().clone();
        let mut runner = self.spawn_async_vm();
        let future = async move {
            let is_equal = compare_value_maps(&mut runner, map_a, map_b).await?;
            Ok(KValue::Bool(is_equal != invert))
        };

        self.poll_task_into_register(result, KTask::with_future(future))
    }

    fn run_overridden_comparison(
        &mut self,
        result: u8,
        lhs: KValue,
        rhs: KValue,
        op: KValue,
        invert: bool,
    ) -> Result<()> {
        let mut runner = self.spawn_async_vm();
        let future = async move {
            let result = call_overridden_comparison(&mut runner, lhs, rhs, op).await?;
            Ok(KValue::Bool(result != invert))
        };

        self.poll_task_into_register(result, KTask::with_future(future))
    }

    fn run_overridden_comparison_fallback(
        &mut self,
        result: u8,
        lhs: KValue,
        rhs: KValue,
        less_op: KValue,
        equal_op: Option<KValue>,
        fallback: ComparisonFallback,
    ) -> Result<()> {
        let mut runner = self.spawn_async_vm();
        let future = async move {
            let less =
                call_overridden_comparison(&mut runner, lhs.clone(), rhs.clone(), less_op).await?;
            let result = match fallback {
                ComparisonFallback::LessOrEqual => {
                    let Some(equal_op) = equal_op else {
                        return runtime_error!(ErrorKind::UnexpectedError);
                    };
                    less || call_overridden_comparison(&mut runner, lhs, rhs, equal_op).await?
                }
                ComparisonFallback::Greater => {
                    let Some(equal_op) = equal_op else {
                        return runtime_error!(ErrorKind::UnexpectedError);
                    };
                    !(less || call_overridden_comparison(&mut runner, lhs, rhs, equal_op).await?)
                }
                ComparisonFallback::GreaterOrEqual => !less,
            };
            Ok(KValue::Bool(result))
        };

        self.poll_task_into_register(result, KTask::with_future(future))
    }

    fn call_overridden_op_1(
        &mut self,
        result_register: Option<u8>,
        value_register: u8,
        op: KValue,
    ) -> Result<()> {
        let pending_behavior = if result_register.is_some() {
            PendingCallBehavior::ReturnTask
        } else {
            PendingCallBehavior::Suspend
        };
        // Set up the call registers at the end of the stack
        let frame_base = self.new_frame_base()?;
        self.registers.push(self.clone_register(value_register)); // Frame base
        self.call_callable_with_pending_behavior(
            CallInfo {
                result_register,
                frame_base,
                instance: Some(frame_base),
                arg_count: 0,
                packed_arg_count: 0,
            },
            op,
            pending_behavior,
        )?;
        Ok(())
    }

    fn call_overridden_op_2(
        &mut self,
        result_register: Option<u8>,
        instance: KValue,
        arg: KValue,
        op: KValue,
    ) -> Result<()> {
        let pending_behavior = if result_register.is_some() {
            PendingCallBehavior::ReturnTask
        } else {
            PendingCallBehavior::Suspend
        };
        // Set up the call registers at the end of the stack
        let frame_base = self.new_frame_base()?;

        self.registers.push(instance); // Frame base
        self.registers.push(arg);

        self.call_callable_with_pending_behavior(
            CallInfo {
                result_register,
                frame_base,
                instance: Some(frame_base),
                arg_count: 1,
                packed_arg_count: 0,
            },
            op,
            pending_behavior,
        )?;
        Ok(())
    }

    fn call_overridden_op_3(
        &mut self,
        result_register: Option<u8>,
        instance: KValue,
        arg_1: KValue,
        arg_2: KValue,
        op: KValue,
    ) -> Result<()> {
        let pending_behavior = if result_register.is_some() {
            PendingCallBehavior::ReturnTask
        } else {
            PendingCallBehavior::Suspend
        };
        // Set up the call registers at the end of the stack
        let frame_base = self.new_frame_base()?;

        self.registers.push(instance); // Frame base
        self.registers.push(arg_1);
        self.registers.push(arg_2);

        self.call_callable_with_pending_behavior(
            CallInfo {
                result_register,
                frame_base,
                instance: Some(frame_base),
                arg_count: 2,
                packed_arg_count: 0,
            },
            op,
            pending_behavior,
        )?;
        Ok(())
    }

    fn call_discarded_overridden_op_3(
        &mut self,
        instance: KValue,
        arg_1: KValue,
        arg_2: KValue,
        op: KValue,
    ) -> Result<ControlFlow> {
        let should_poll_result = op.is_async_callable()
            || matches!(op, KValue::NativeFunction(_) | KValue::NativeVmFunction(_));
        if should_poll_result {
            let truncate_registers_to = self.registers.len();
            let result_register = self.next_register();
            self.registers.push(KValue::Null);

            self.call_overridden_op_3(Some(result_register), instance, arg_1, arg_2, op)?;
            self.poll_discarded_implicit_task_in_register(result_register, truncate_registers_to)
        } else {
            self.call_overridden_op_3(None, instance, arg_1, arg_2, op)?;
            Ok(ControlFlow::Continue)
        }
    }

    fn run_jump_if_true(&mut self, register: u8, offset: u32) -> Result<()> {
        match self.get_register(register) {
            KValue::Null => {}
            KValue::Bool(b) if !b => {}
            _ => self.jump_ip(offset),
        }
        Ok(())
    }

    fn run_jump_if_false(&mut self, register: u8, offset: u32) -> Result<()> {
        match self.get_register(register) {
            KValue::Null => self.jump_ip(offset),
            KValue::Bool(b) if !b => self.jump_ip(offset),
            _ => {}
        }
        Ok(())
    }

    fn run_jump_if_null(&mut self, register: u8, offset: u32) -> Result<()> {
        if matches!(self.get_register(register), KValue::Null) {
            self.jump_ip(offset)
        }
        Ok(())
    }

    fn run_size(
        &mut self,
        result_register: u8,
        value_register: u8,
        throw_if_value_has_no_size: bool,
    ) -> Result<()> {
        use KValue::*;

        let size_key = UnaryOp::Size.into();
        let value = self.get_register(value_register);

        let size = match value {
            List(l) => Some(l.len()),
            Tuple(t) => Some(t.len()),
            Str(l) => Some(l.len()),
            Range(r) => r.size(),
            Map(m) if m.contains_meta_key(&size_key) => {
                let op = m.get_meta_value(&size_key).unwrap();
                return self.call_overridden_op_1(Some(result_register), value_register, op);
            }
            Map(m) => Some(m.len()),
            Object(o) => o.try_borrow()?.size(),
            TemporaryTuple(RegisterSlice { count, .. }) => Some(*count),
            _ => None,
        };

        if let Some(size) = size {
            self.set_register(result_register, size.into());
            Ok(())
        } else if throw_if_value_has_no_size {
            unexpected_type("a value with a defined size", value)
        } else {
            self.set_register(result_register, Null);
            Ok(())
        }
    }

    fn successful_import(
        &mut self,
        import_register: u8,
        imported: KValue,
        import_all: bool,
    ) -> Result<()> {
        self.set_register(import_register, imported.clone());

        if import_all {
            self.frame_mut()
                .non_locals
                .get_or_insert_default()
                .add_wildcard_import(imported);
        }

        Ok(())
    }

    fn run_import(
        &mut self,
        import_register: u8,
        import_all: bool,
        allow_pending: bool,
    ) -> Result<ControlFlow> {
        let import_name = match self.clone_register(import_register) {
            KValue::Str(s) => s,
            value @ KValue::Map(_) => {
                self.successful_import(import_register, value, import_all)?;
                return Ok(ControlFlow::Continue);
            }
            other => return unexpected_type("import id or string, or accessible value", &other),
        };

        // Is the import available as a non-local?
        let maybe_non_local = self
            .frame()
            .non_local(&import_name)
            .or_else(|| self.context.prelude.get(&import_name));
        if let Some(value) = maybe_non_local {
            self.successful_import(import_register, value, import_all)?;
            return Ok(ControlFlow::Continue);
        }

        // Attempt to compile the imported module from disk,
        // using the current source path as the relative starting location
        let source_path = self.reader.chunk.path.clone();
        let compile_result = self.context.loader.borrow_mut().compile_module(
            &import_name,
            source_path
                .as_ref()
                .map(|path_string| Path::new(path_string.as_str())),
        )?;

        // Has the module been loaded previously?
        let maybe_in_cache = self
            .context
            .module_cache
            .borrow()
            .get(&compile_result.path)
            .cloned();
        match maybe_in_cache {
            Some(ModuleCacheEntry::Loading(task)) => {
                if self
                    .module_import_stack
                    .iter()
                    .any(|path| path == &compile_result.path)
                {
                    return runtime_error!("recursive import of module '{import_name}'");
                }

                let pending = PendingImport {
                    import_register,
                    import_all,
                    import_name,
                    module_path: compile_result.path,
                    task,
                    remove_cache_on_sync_error: false,
                };

                return match self.poll_pending_import(pending)? {
                    ControlFlow::Pending if !allow_pending => {
                        let pending = match self.frame_mut().pending_operation.take() {
                            Some(PendingOperation::Import(pending)) => pending,
                            _ => return runtime_error!(ErrorKind::UnexpectedError),
                        };
                        if pending.remove_cache_on_sync_error {
                            self.context
                                .module_cache
                                .borrow_mut()
                                .remove(&pending.module_path);
                        }
                        runtime_error!("import of '{}' requires await", pending.import_name)
                    }
                    control_flow => Ok(control_flow),
                };
            }
            Some(ModuleCacheEntry::Loaded(cached_exports)) if compile_result.loaded_from_cache => {
                self.successful_import(import_register, cached_exports.into(), import_all)?;
                return Ok(ControlFlow::Continue);
            }
            _ => {}
        }

        // The module needs to be loaded, which involves the following steps.
        //   - Execute the module's script.
        //   - If the module contains @tests, run them.
        //   - If the module contains a @main function, run it.
        //   - If the steps above are successful, then cache the resulting exports map.

        let task = self.make_import_task(compile_result.path.clone(), compile_result.chunk);

        self.context.module_cache.borrow_mut().insert(
            compile_result.path.clone(),
            ModuleCacheEntry::Loading(task.clone()),
        );

        let pending = PendingImport {
            import_register,
            import_all,
            import_name,
            module_path: compile_result.path,
            task,
            remove_cache_on_sync_error: true,
        };

        match self.poll_pending_import(pending)? {
            ControlFlow::Pending if !allow_pending => {
                let pending = match self.frame_mut().pending_operation.take() {
                    Some(PendingOperation::Import(pending)) => pending,
                    _ => return runtime_error!(ErrorKind::UnexpectedError),
                };
                if pending.remove_cache_on_sync_error {
                    self.context
                        .module_cache
                        .borrow_mut()
                        .remove(&pending.module_path);
                }
                runtime_error!("import of '{}' requires await", pending.import_name)
            }
            control_flow => Ok(control_flow),
        }
    }

    fn make_import_task(&self, module_path: PathBuf, chunk: Ptr<Chunk>) -> KTask {
        let mut module_exports = KMap::default();
        module_exports.ensure_meta_map();

        let mut vm = self.spawn_shared_vm();
        vm.exports = module_exports.clone();
        vm.module_import_stack.push(module_path);
        let run_import_tests = self.context.settings.run_import_tests;

        KTask::with_future(async move {
            let mut runner = AsyncKotoVm::new(vm);

            runner.run(chunk).await?;

            if run_import_tests {
                runner.run_tests(module_exports.clone()).await?;
            }

            match module_exports.get_meta_value(&MetaKey::Main) {
                Some(main) if main.is_callable() => {
                    runner.call_function_with_args(main, vec![]).await?;
                }
                Some(unexpected) => return unexpected_type("callable function", &unexpected),
                None => {}
            }

            Ok(KValue::Map(module_exports))
        })
    }

    fn poll_pending_import(&mut self, mut pending: PendingImport) -> Result<ControlFlow> {
        let poll_result = if let Some(waker) = self.task_waker.clone() {
            let mut context = Context::from_waker(&waker);
            self.poll_task_with_context(&mut pending.task, &mut context)
        } else {
            self.poll_task(&mut pending.task)
        };

        match poll_result {
            Ok(KTaskPoll::Ready(KValue::Map(module_exports))) => {
                let should_call_callback = !matches!(
                    self.context.module_cache.borrow().get(&pending.module_path),
                    Some(ModuleCacheEntry::Loaded(_))
                );

                self.context.module_cache.borrow_mut().insert(
                    pending.module_path.clone(),
                    ModuleCacheEntry::Loaded(module_exports.clone()),
                );

                if should_call_callback
                    && let Some(callback) = &self.context.settings.module_imported_callback
                {
                    callback(&pending.module_path);
                }

                self.successful_import(
                    pending.import_register,
                    module_exports.into(),
                    pending.import_all,
                )?;
                Ok(ControlFlow::Continue)
            }
            Ok(KTaskPoll::Ready(unexpected)) => {
                self.context
                    .module_cache
                    .borrow_mut()
                    .remove(&pending.module_path);
                unexpected_type("imported module", &unexpected)
            }
            Ok(KTaskPoll::Pending) => {
                self.set_pending_operation(PendingOperation::Import(pending))?;
                Ok(ControlFlow::Pending)
            }
            Err(error) => {
                self.context
                    .module_cache
                    .borrow_mut()
                    .remove(&pending.module_path);
                Err(error)
            }
        }
    }

    fn run_index_assign(
        &mut self,
        result_register: Option<u8>,
        indexable_register: u8,
        index_register: u8,
        value_register: u8,
    ) -> Result<ControlFlow> {
        use KValue::*;

        let indexable = self.clone_register(indexable_register);
        let index_value = self.get_register(index_register);
        let value = self.get_register(value_register);

        let control_flow = match indexable {
            List(list) => {
                let mut list_data = list.data_mut();
                let list_len = list_data.len();
                match index_value {
                    Number(index) => {
                        let u_index = usize::from(index);
                        if *index >= 0.0 && u_index < list_len {
                            list_data[u_index] = value.clone();
                        } else {
                            return runtime_error!("invalid index ({index})");
                        }
                    }
                    Range(range) => {
                        for i in range.indices(list_len) {
                            list_data[i] = value.clone();
                        }
                    }
                    unexpected => return unexpected_type("Number or Range", unexpected),
                }
                Ok(ControlFlow::Continue)
            }
            Map(map) if map.contains_meta_key(&WriteOp::IndexAssign.into()) => {
                let op = map.get_meta_value(&WriteOp::IndexAssign.into()).unwrap();
                let index_value = index_value.clone();
                let value = value.clone();

                self.call_discarded_overridden_op_3(map.into(), index_value, value, op)
            }
            Map(map) => match index_value {
                Number(index) => {
                    let mut map_data = map.data_mut();
                    let map_len = map_data.len();
                    let u_index = usize::from(index);
                    if *index >= 0.0 && u_index < map_len {
                        match value {
                            Tuple(new_entry) if new_entry.len() == 2 => {
                                let key = ValueKey::try_from(new_entry[0].clone())?;
                                // There's no API on IndexMap for replacing an entry,
                                // so use swap_remove_index to remove the old entry,
                                // then insert the new entry at the end of the map,
                                // followed by swap_indices to swap the new entry back into position.
                                map_data.swap_remove_index(u_index);
                                map_data.insert(key, new_entry[1].clone());
                                map_data.swap_indices(u_index, map_len - 1);
                                Ok(ControlFlow::Continue)
                            }
                            unexpected => unexpected_type("Tuple with 2 elements", unexpected),
                        }
                    } else {
                        runtime_error!("invalid index ({index})")
                    }
                }
                unexpected => unexpected_type("Number", unexpected),
            },
            Object(o) => {
                o.try_borrow_mut()?.index_assign(index_value, value)?;
                Ok(ControlFlow::Continue)
            }
            unexpected => unexpected_type("a mutable indexable value", &unexpected),
        }?;

        if let Some(result_register) = result_register {
            self.set_register(result_register, self.clone_register(value_register));
        }

        Ok(control_flow)
    }

    fn validate_index(&self, n: KNumber, size: Option<usize>) -> Result<usize> {
        let index = usize::from(n);

        if n < 0.0 {
            return runtime_error!("negative indices aren't allowed ('{n}')");
        } else if let Some(size) = size
            && index >= size
        {
            return runtime_error!("index out of bounds - index: {n}, size: {size}");
        }

        Ok(index)
    }

    fn run_index(
        &mut self,
        result_register: u8,
        value_register: u8,
        index_register: u8,
    ) -> Result<()> {
        use KValue::*;

        let value = self.clone_register(value_register);
        let index = self.clone_register(index_register);

        let result = match (&value, index) {
            (List(l), Number(n)) => {
                let index = self.validate_index(n, Some(l.len()))?;
                l.data()[index].clone()
            }
            (List(l), Range(range)) => {
                let indices = range.indices(l.len());
                List(KList::from_slice(&l.data()[indices]))
            }
            (Tuple(t), Number(n)) => {
                let index = self.validate_index(n, Some(t.len()))?;
                t[index].clone()
            }
            (Tuple(t), Range(range)) => {
                let indices = range.indices(t.len());
                let Some(result) = t.make_sub_tuple(indices) else {
                    // `range.indices` is guaranteed to return valid indices for the tuple
                    unreachable!();
                };
                Tuple(result)
            }
            (Str(s), Number(n)) => {
                let index = self.validate_index(n, Some(s.len()))?;
                let Some(result) = s.with_bounds(index..index + 1) else {
                    return runtime_error!(
                        "indexing with ({index}) would result in invalid UTF-8 data"
                    );
                };
                Str(result)
            }
            (Str(s), Range(range)) => {
                let indices = range.indices(s.len());
                let Some(result) = s.with_bounds(indices) else {
                    return runtime_error!(
                        "indexing with ({range}) would result in invalid UTF-8 data"
                    );
                };
                Str(result)
            }
            (Map(m), index) if m.contains_meta_key(&ReadOp::Index.into()) => {
                let op = m.get_meta_value(&ReadOp::Index.into()).unwrap();
                return self.call_overridden_op_2(Some(result_register), value, index, op);
            }
            (Map(m), Number(n)) => {
                let entries = m.data();
                let index = self.validate_index(n, Some(entries.len()))?;
                let Some((key, value)) = entries.get_index(index) else {
                    // The index has just been validated
                    unreachable!();
                };
                let result = KTuple::from(vec![key.value().clone(), value.clone()]);
                Tuple(result)
            }
            (Range(r), Number(n)) if r.start().is_some() => {
                let start = r.start().unwrap();
                let index = self.validate_index(n, r.size())?;
                Number((start + index as i64).into())
            }
            (Object(o), index) => o.try_borrow()?.index(&index)?,
            (unexpected_value, unexpected_index) => {
                return runtime_error!(
                    "Unable to index '{}' with '{}'",
                    unexpected_value.type_as_string(),
                    unexpected_index.type_as_string(),
                );
            }
        };

        self.set_register(result_register, result);

        Ok(())
    }

    fn run_access_assign(
        &mut self,
        result_register: Option<u8>,
        map_register: u8,
        key_register: u8,
        value_register: u8,
    ) -> Result<ControlFlow> {
        let key = self.get_register(key_register);
        let value = self.get_register(value_register);

        let control_flow = match self.get_register(map_register) {
            KValue::Map(map) if map.contains_meta_key(&WriteOp::AccessAssign.into()) => {
                let op = map.get_meta_value(&WriteOp::AccessAssign.into()).unwrap();
                self.call_discarded_overridden_op_3(
                    map.clone().into(),
                    key.clone(),
                    value.clone(),
                    op,
                )
            }
            KValue::Map(map) => {
                let key = ValueKey::try_from(key.clone())?;
                map.data_mut().insert(key, value.clone());
                Ok(ControlFlow::Continue)
            }
            KValue::Object(o) => match key {
                KValue::Str(key) => {
                    o.try_borrow_mut()?.access_assign(key, value)?;
                    Ok(ControlFlow::Continue)
                }
                unexpected => unexpected_type("String", unexpected),
            },
            unexpected => unexpected_type("a value that supports assignment via '.'", unexpected),
        }?;

        if let Some(result_register) = result_register {
            self.set_register(result_register, self.clone_register(value_register));
        }

        Ok(control_flow)
    }

    fn run_meta_insert(&mut self, map_register: u8, value: u8, meta_id: MetaKeyId) -> Result<()> {
        let value = self.clone_register(value);
        let meta_key = match meta_id_to_key(meta_id, None) {
            Ok(meta_key) => meta_key,
            Err(error) => return runtime_error!("error while preparing meta key: {error}"),
        };

        match self.get_register_mut(map_register) {
            KValue::Map(map) => {
                map.insert_meta(meta_key, value);
                Ok(())
            }
            unexpected => unexpected_type("Map", unexpected),
        }
    }

    fn run_meta_insert_named(
        &mut self,
        map_register: u8,
        value_register: u8,
        meta_id: MetaKeyId,
        name_register: u8,
    ) -> Result<()> {
        let value = self.clone_register(value_register);

        let meta_key = match self.clone_register(name_register) {
            KValue::Str(name) => match meta_id_to_key(meta_id, Some(name)) {
                Ok(key) => key,
                Err(error) => return runtime_error!("error while preparing meta key: {error}"),
            },
            other => return unexpected_type("String", &other),
        };

        match self.get_register_mut(map_register) {
            KValue::Map(map) => {
                map.insert_meta(meta_key, value);
                Ok(())
            }
            unexpected => unexpected_type("Map", unexpected),
        }
    }

    fn run_meta_export(&mut self, value: u8, meta_id: MetaKeyId) -> Result<()> {
        let value = self.clone_register(value);
        let meta_key = match meta_id_to_key(meta_id, None) {
            Ok(meta_key) => meta_key,
            Err(error) => return runtime_error!("error while preparing meta key: {error}"),
        };

        self.exports.insert_meta(meta_key, value);
        Ok(())
    }

    fn run_meta_export_named(
        &mut self,
        meta_id: MetaKeyId,
        name_register: u8,
        value_register: u8,
    ) -> Result<()> {
        let value = self.clone_register(value_register);

        let meta_key = match self.clone_register(name_register) {
            KValue::Str(name) => match meta_id_to_key(meta_id, Some(name)) {
                Ok(key) => key,
                Err(error) => return runtime_error!("error while preparing meta key: {error}"),
            },
            other => return unexpected_type("String", &other),
        };

        self.exports.insert_meta(meta_key, value);
        Ok(())
    }

    fn run_access(
        &mut self,
        result_register: u8,
        value_register: u8,
        key_string: KString,
    ) -> Result<()> {
        self.run_access_inner(result_register, value_register, key_string, true)?;
        Ok(())
    }

    fn run_try_access(
        &mut self,
        result_register: u8,
        value_register: u8,
        key_string: KString,
        jump_offset: u32,
    ) -> Result<()> {
        if !self.run_access_inner(result_register, value_register, key_string, false)? {
            self.jump_ip(jump_offset);
        }
        Ok(())
    }

    // Runs `.` access on a value.
    //
    // If the given key was found then `true` will be returned.
    //
    // If `error_if_not_found` is `true`, then an error will be returned if the key wasn't found,
    // otherwise `false` will be returned.
    fn run_access_inner(
        &mut self,
        result_register: u8,
        value_register: u8,
        key_string: KString,
        error_if_not_found: bool,
    ) -> Result<bool> {
        use KValue::*;

        let accessed_value = self.clone_register(value_register);
        let key = ValueKey::from(key_string.clone());

        macro_rules! core_op {
            ($module:ident, $iterator_fallback:expr) => {{
                if let Some(op) = self.get_core_op(
                    &key,
                    &self.context.core_lib.$module,
                    $iterator_fallback,
                    stringify!($module),
                    error_if_not_found,
                )? {
                    self.set_register(result_register, op);
                    Ok(true)
                } else {
                    Ok(false)
                }
            }};
        }

        match &accessed_value {
            List(_) => core_op!(list, true),
            Number(_) => core_op!(number, false),
            Range(_) => core_op!(range, true),
            Str(_) => core_op!(string, true),
            Tuple(_) => core_op!(tuple, true),
            Iterator(_) => core_op!(iterator, false),
            Map(map) if map.contains_meta_key(&ReadOp::Access.into()) => {
                let op = map.get_meta_value(&ReadOp::Access.into()).unwrap();
                self.call_overridden_op_2(Some(result_register), accessed_value, key.into(), op)?;
                Ok(true)
            }
            Map(map) => {
                let mut access_map = map.clone();
                let mut access_result = None;
                while access_result.is_none() {
                    let maybe_value = access_map.get(&key);
                    match maybe_value {
                        Some(value) => access_result = Some(value),
                        // Fallback to the map module when there's no metamap
                        None if access_map.meta_map().is_none() => {
                            return core_op!(map, error_if_not_found);
                        }
                        _ => match access_map.get_meta_value(&MetaKey::Named(key_string.clone())) {
                            Some(value) => access_result = Some(value),
                            None => match access_map.get_meta_value(&MetaKey::Base) {
                                Some(Map(base)) => {
                                    // Attempt the access again with the base map
                                    access_map = base;
                                }
                                Some(unexpected) => {
                                    return unexpected_type("Map as base value", &unexpected);
                                }
                                None => break,
                            },
                        },
                    }
                }

                // Iterator fallback?
                if access_result.is_none()
                    && (map.contains_meta_key(&UnaryOp::Iterator.into())
                        || map.contains_meta_key(&UnaryOp::Next.into()))
                {
                    access_result = self.get_core_op(
                        &key,
                        &self.context.core_lib.iterator,
                        false,
                        &accessed_value.type_as_string(),
                        error_if_not_found,
                    )?;
                }

                match access_result {
                    Some(value) => {
                        self.set_register(result_register, value);
                        Ok(true)
                    }
                    None => {
                        if error_if_not_found {
                            runtime_error!(
                                "'{key}' not found in '{}'",
                                accessed_value.type_as_string()
                            )
                        } else {
                            Ok(false)
                        }
                    }
                }
            }
            Object(o) => {
                let o = o.try_borrow()?;

                let mut result = None;

                if let KValue::Str(key) = key.value() {
                    result = o.access(key)?;
                }

                // Iterator fallback?
                if result.is_none() && !matches!(o.is_iterable(), IsIterable::NotIterable) {
                    result = self.get_core_op(
                        &key,
                        &self.context.core_lib.iterator,
                        false,
                        &o.type_string(),
                        error_if_not_found,
                    )?;
                }

                if let Some(result) = result {
                    self.set_register(result_register, result);
                    Ok(true)
                } else if error_if_not_found {
                    runtime_error!("'{key}' not found in '{}'", o.type_string())
                } else {
                    Ok(false)
                }
            }
            unexpected => unexpected_type("a value that supports '.' access", unexpected),
        }
    }

    fn get_core_op(
        &self,
        key: &ValueKey,
        module: &KMap,
        iterator_fallback: bool,
        module_name: &str,
        error_if_not_found: bool,
    ) -> Result<Option<KValue>> {
        let maybe_op = match module.get(key) {
            None if iterator_fallback => self.context.core_lib.iterator.get(key),
            maybe_op => maybe_op,
        };

        if let Some(result) = maybe_op {
            Ok(Some(result))
        } else if error_if_not_found {
            runtime_error!("'{key}' not found in the '{module_name}' module")
        } else {
            Ok(None)
        }
    }

    fn call_native_function(
        &mut self,
        call_info: &CallInfo,
        callable: ExternalCallable,
        pending_behavior: PendingCallBehavior,
    ) -> Result<ControlFlow> {
        match callable {
            ExternalCallable::Function(f) => {
                let mut call_context =
                    CallContext::new(self, call_info.frame_base, call_info.arg_count);
                let result = (f.function)(&mut call_context)?;
                self.finish_native_call(call_info, result);
                Ok(ControlFlow::Continue)
            }
            ExternalCallable::VmFunction(f) => {
                let mut call_context =
                    VmCallContext::new(self, call_info.frame_base, call_info.arg_count);
                match (f.function)(&mut call_context)? {
                    VmOutput::Ready(result) => {
                        self.finish_native_call(call_info, result);
                        Ok(ControlFlow::Continue)
                    }
                    VmOutput::Pending(task) => match pending_behavior {
                        PendingCallBehavior::Suspend => {
                            self.set_pending_native_call(call_info, task)?;
                            Ok(ControlFlow::Pending)
                        }
                        PendingCallBehavior::ReturnTask => {
                            if let Some(result_register) = call_info.result_register {
                                self.set_register(result_register, task.into());
                                self.finish_native_call_registers(call_info.frame_base);
                                Ok(ControlFlow::Continue)
                            } else {
                                self.set_pending_native_call(call_info, task)?;
                                Ok(ControlFlow::Pending)
                            }
                        }
                    },
                }
            }
            ExternalCallable::Object(o) => {
                let mut call_context =
                    CallContext::new(self, call_info.frame_base, call_info.arg_count);
                let result = o.try_borrow_mut()?.call(&mut call_context)?;
                self.finish_native_call(call_info, result);
                Ok(ControlFlow::Continue)
            }
        }
    }

    fn call_or_resume_native(
        &mut self,
        call_info: CallInfo,
        callable: KValue,
    ) -> Result<ControlFlow> {
        self.call_callable(call_info, callable)
    }

    fn has_pending_operation(&self) -> bool {
        self.call_stack
            .last()
            .is_some_and(|frame| frame.pending_operation.is_some())
    }

    fn set_pending_operation(&mut self, pending: PendingOperation) -> Result<()> {
        if self.call_stack.is_empty() {
            return runtime_error!("execution is pending");
        }

        debug_assert!(self.frame().pending_operation.is_none());
        self.frame_mut().pending_operation = Some(pending);

        Ok(())
    }

    fn poll_pending_operation(&mut self) -> Result<ControlFlow> {
        let Some(pending) = self.frame_mut().pending_operation.take() else {
            return Ok(ControlFlow::Continue);
        };

        match pending {
            PendingOperation::NativeCall(pending) => self.poll_pending_native_call(pending),
            PendingOperation::PackedCall(pending) => self.poll_pending_packed_call(pending),
            PendingOperation::ImplicitTask(pending) => self.poll_pending_implicit_task(pending),
            PendingOperation::DebugInstruction(pending) => {
                self.poll_pending_debug_instruction(pending)
            }
            PendingOperation::CheckSize(pending) => self.poll_pending_check_size(pending),
            PendingOperation::Slice(pending) => self.poll_pending_slice(pending),
            PendingOperation::StringPush(pending) => self.poll_pending_string_push(pending),
            PendingOperation::Import(pending) => self.poll_pending_import(pending),
        }
    }

    fn set_pending_native_call(&mut self, call_info: &CallInfo, task: KTask) -> Result<()> {
        self.set_pending_operation(PendingOperation::NativeCall(PendingNativeCall {
            result_register: call_info.result_register,
            frame_base: call_info.frame_base,
            task,
        }))
    }

    fn poll_pending_native_call(&mut self, mut pending: PendingNativeCall) -> Result<ControlFlow> {
        let poll_result = if let Some(waker) = self.task_waker.clone() {
            let mut context = Context::from_waker(&waker);
            pending.task.poll_with_context(&mut context)?
        } else {
            pending.task.poll()?
        };

        match poll_result {
            KTaskPoll::Ready(result) => {
                self.finish_pending_native_call(&pending, result);
                Ok(ControlFlow::Continue)
            }
            KTaskPoll::Pending => {
                self.set_pending_operation(PendingOperation::NativeCall(pending))?;
                Ok(ControlFlow::Pending)
            }
        }
    }

    fn finish_pending_native_call(&mut self, pending: &PendingNativeCall, result: KValue) {
        if let Some(result_register) = pending.result_register {
            self.set_register(result_register, result);
        }

        self.finish_native_call_registers(pending.frame_base);
    }

    fn finish_native_call(&mut self, call_info: &CallInfo, result: KValue) {
        if let Some(result_register) = call_info.result_register {
            self.set_register(result_register, result);
        }

        self.finish_native_call_registers(call_info.frame_base);
    }

    fn finish_native_call_registers(&mut self, frame_base: u8) {
        if !self.call_stack.is_empty() {
            // External function calls don't use the push/pop frame mechanism,
            // so drop the call args here now that the call has been completed,
            self.truncate_registers(frame_base);
            // Ensure that the calling frame still has the required number of registers
            let min_frame_registers = self.register_index(self.frame().required_registers);
            if self.registers.len() < min_frame_registers {
                self.registers.resize(min_frame_registers, KValue::Null);
            }
        }
    }

    fn poll_implicit_task_in_register(&mut self, result_register: u8) -> Result<ControlFlow> {
        self.poll_implicit_task_in_register_impl(result_register, None)
    }

    fn poll_discarded_implicit_task_in_register(
        &mut self,
        result_register: u8,
        truncate_registers_to: usize,
    ) -> Result<ControlFlow> {
        self.poll_implicit_task_in_register_impl(result_register, Some(truncate_registers_to))
    }

    fn poll_implicit_task_in_register_impl(
        &mut self,
        result_register: u8,
        truncate_registers_to: Option<usize>,
    ) -> Result<ControlFlow> {
        match self.clone_register(result_register) {
            KValue::Task(task) => self.poll_pending_implicit_task(PendingImplicitTask {
                result_register,
                task,
                truncate_registers_to,
            }),
            _ => {
                if let Some(truncate_registers_to) = truncate_registers_to {
                    self.registers.truncate(truncate_registers_to);
                }
                Ok(ControlFlow::Continue)
            }
        }
    }

    fn poll_pending_implicit_task(
        &mut self,
        mut pending: PendingImplicitTask,
    ) -> Result<ControlFlow> {
        let poll_result = if let Some(waker) = self.task_waker.clone() {
            let mut context = Context::from_waker(&waker);
            pending.task.poll_with_context(&mut context)?
        } else {
            pending.task.poll()?
        };

        match poll_result {
            KTaskPoll::Ready(result) => {
                self.set_register(pending.result_register, result);
                if let Some(truncate_registers_to) = pending.truncate_registers_to {
                    self.registers.truncate(truncate_registers_to);
                }
                Ok(ControlFlow::Continue)
            }
            KTaskPoll::Pending => {
                self.set_pending_operation(PendingOperation::ImplicitTask(pending))?;
                Ok(ControlFlow::Pending)
            }
        }
    }

    // Similar to `call_koto_function`, but sets up the frame in a new VM for a suspended function
    fn make_suspended_function_vm(
        &mut self,
        call_info: &CallInfo,
        f: &KFunction,
    ) -> Result<KotoVm> {
        let mut vm = self.spawn_shared_vm();
        // Push a frame for running the function
        vm.push_frame(
            f.chunk.clone(),
            f.ip,
            0, // Arguments will be copied starting in register 0
            None,
            f.non_locals(),
        );
        // Set the VM's state as suspended
        vm.execution_state = ExecutionState::Suspended;

        // Place the instance in the first register of the vm
        let instance = self
            .get_register_safe(call_info.frame_base)
            .cloned()
            .unwrap_or(KValue::Null);
        vm.registers.push(instance);

        let call_arg_base = call_info.frame_base + 1;
        let expected_arg_count = f.expected_arg_count();

        // Copy any regular (non-variadic) arguments into the vm
        vm.registers.extend(
            self.register_slice(call_arg_base, expected_arg_count.min(call_info.arg_count))
                .iter()
                .cloned(),
        );

        // Fill in any missing arguments with default values
        apply_optional_arguments(
            &mut vm.registers,
            f,
            call_info.arg_count,
            expected_arg_count,
        )?;

        // Copy any extra arguments into the vm,
        // they'll get extracted into a tuple in apply_variadic_arguments
        vm.registers.extend(
            self.register_slice(
                call_arg_base + expected_arg_count,
                call_info.arg_count.saturating_sub(expected_arg_count),
            )
            .iter()
            .cloned(),
        );

        // Move variadic arguments into a tuple
        apply_variadic_arguments(
            &mut vm.registers,
            1, // The first argument goes into register 1 in the suspended vm
            call_info,
            f,
            expected_arg_count,
        )?;

        // Captures and temp tuple values are placed in the registers following the arguments
        apply_captures(&mut vm.registers, f);

        Ok(vm)
    }

    fn call_generator(&mut self, call_info: &CallInfo, f: &KFunction) -> Result<()> {
        let generator_vm = self.make_suspended_function_vm(call_info, f)?;

        // Move the generator vm into an iterator and then place it in the result register
        if let Some(result_register) = call_info.result_register {
            self.set_register(result_register, KIterator::with_vm(generator_vm).into());
        }

        Ok(())
    }

    fn call_task(&mut self, call_info: &CallInfo, f: &KFunction) -> Result<()> {
        let task_vm = self.make_suspended_function_vm(call_info, f)?;

        if let Some(result_register) = call_info.result_register {
            self.set_register(
                result_register,
                self.spawn_task(KTask::with_vm(task_vm))?.into(),
            );
        }

        Ok(())
    }

    fn call_koto_function(&mut self, call_info: &CallInfo, f: &KFunction) -> Result<()> {
        debug_assert!(!f.flags.is_generator());
        debug_assert!(!f.flags.is_async());

        // The caller instance is in the frame base register,
        // and then arguments start from register frame_base + 1.
        let call_arg_base_index = self.register_index(call_info.frame_base + 1);
        let expected_arg_count = f.expected_arg_count();

        // Ensure that any temporary registers used to prepare the call args have been removed
        // from the value stack.
        self.registers
            .truncate(call_arg_base_index + call_info.arg_count as usize);

        // Fill in any missing arguments with default values
        apply_optional_arguments(
            &mut self.registers,
            f,
            call_info.arg_count,
            expected_arg_count,
        )?;

        // Move variadic arguments into a tuple
        apply_variadic_arguments(
            &mut self.registers,
            call_arg_base_index,
            call_info,
            f,
            expected_arg_count,
        )?;

        // Captures and temp tuple values are placed in the registers following the arguments
        apply_captures(&mut self.registers, f);

        // Set up a new frame for the called function
        self.push_frame(
            f.chunk.clone(),
            f.ip,
            call_info.frame_base,
            call_info.result_register,
            f.non_locals(),
        );

        Ok(())
    }

    fn call_callable(&mut self, info: CallInfo, callable: KValue) -> Result<ControlFlow> {
        self.call_callable_with_pending_behavior(info, callable, PendingCallBehavior::Suspend)
    }

    fn call_callable_with_pending_behavior(
        &mut self,
        mut info: CallInfo,
        callable: KValue,
        pending_behavior: PendingCallBehavior,
    ) -> Result<ControlFlow> {
        use KValue::*;

        if let Some(instance) = info.instance {
            // The instance will only match the frame base when the call stack has been set up
            // manually, like in `call_and_run_function`.
            // Koto bytecode may or may not have placed the instance in the frame base.
            if instance != info.frame_base {
                self.set_register(info.frame_base, self.clone_register(instance));
            }
        } else {
            // If there's no instance for the call then ensure that the frame base is null.
            self.set_register(info.frame_base, KValue::Null);
        }

        if let Some(unpacking) = self.unpack_packed_arguments(&mut info)? {
            self.set_pending_operation(PendingOperation::PackedCall(PendingPackedCall {
                call_info: info,
                callable,
                pending_behavior,
                unpacking,
            }))?;
            return Ok(ControlFlow::Pending);
        }

        match callable {
            Function(f) => {
                if f.flags.is_generator() {
                    self.call_generator(&info, &f)?;
                } else if f.flags.is_async() {
                    self.call_task(&info, &f)?;
                } else {
                    self.call_koto_function(&info, &f)?;
                }
                Ok(ControlFlow::Continue)
            }
            NativeFunction(f) => {
                self.call_native_function(&info, ExternalCallable::Function(f), pending_behavior)
            }
            NativeVmFunction(f) => {
                self.call_native_function(&info, ExternalCallable::VmFunction(f), pending_behavior)
            }
            Object(o) => {
                self.call_native_function(&info, ExternalCallable::Object(o), pending_behavior)
            }
            Map(ref m) if m.contains_meta_key(&MetaKey::Call) => {
                let f = m.get_meta_value(&MetaKey::Call).unwrap();
                // Set the callable value as the instance by placing it in the frame base,
                // and then passing the @|| function into call_callable
                self.set_register(info.frame_base, callable);
                self.call_callable_with_pending_behavior(
                    CallInfo {
                        instance: Some(info.frame_base),
                        ..info
                    },
                    f,
                    pending_behavior,
                )
            }
            unexpected => unexpected_type("callable function", &unexpected),
        }
    }

    fn unpack_packed_arguments(
        &mut self,
        info: &mut CallInfo,
    ) -> Result<Option<PackedArgumentUnpacking>> {
        if info.packed_arg_count == 0 {
            return Ok(None);
        }

        // The indices of the registers that need to be unpacked are place in the registers
        // following the call args.
        let first_arg_index = self.register_index(info.frame_base + 1);
        let first_packed_arg_index = first_arg_index + info.arg_count as usize;
        let packed_arg_count = info.packed_arg_count as usize;
        info.packed_arg_count = 0;
        let last_packed_arg_index = first_packed_arg_index + packed_arg_count;
        let packed_arg_registers = self
            .registers
            .drain(first_packed_arg_index..last_packed_arg_index)
            .map(|packed_arg_register| match packed_arg_register {
                KValue::Number(n) => Ok(usize::from(n)),
                unexpected => unexpected_type("Number", &unexpected),
            })
            .collect::<Result<SmallVec<[usize; 4]>>>()?;

        let unpacking = PackedArgumentUnpacking {
            first_arg_index,
            packed_arg_registers,
            next_packed_arg_index: 0,
            original_arg_count: info.arg_count as isize,
            active: None,
        };

        self.poll_packed_argument_unpacking(info, unpacking)
    }

    fn poll_pending_packed_call(&mut self, mut pending: PendingPackedCall) -> Result<ControlFlow> {
        match self.poll_packed_argument_unpacking(&mut pending.call_info, pending.unpacking)? {
            Some(unpacking) => {
                pending.unpacking = unpacking;
                self.set_pending_operation(PendingOperation::PackedCall(pending))?;
                Ok(ControlFlow::Pending)
            }
            None => self.call_callable_with_pending_behavior(
                pending.call_info,
                pending.callable,
                pending.pending_behavior,
            ),
        }
    }

    fn poll_packed_argument_unpacking(
        &mut self,
        info: &mut CallInfo,
        mut unpacking: PackedArgumentUnpacking,
    ) -> Result<Option<PackedArgumentUnpacking>> {
        loop {
            if let Some(active) = unpacking.active.take() {
                match active {
                    PendingPackedArgument::IteratorTask {
                        packed_arg_register,
                        unpack_index,
                        mut task,
                        unpacked_values,
                    } => {
                        let poll_result = if let Some(waker) = self.task_waker.clone() {
                            let mut context = Context::from_waker(&waker);
                            self.poll_task_with_context(&mut task, &mut context)
                        } else {
                            self.poll_task(&mut task)
                        }
                        .map_err(|error| {
                            error.with_context(format!(
                                "while unpacking argument at index {packed_arg_register}"
                            ))
                        })?;

                        match poll_result {
                            KTaskPoll::Ready(KValue::Iterator(iterator)) => {
                                unpacking.active = Some(PendingPackedArgument::Iterator {
                                    unpack_index,
                                    iterator,
                                    unpacked_values,
                                });
                            }
                            KTaskPoll::Ready(unexpected) => {
                                return unexpected_type("Iterator", &unexpected);
                            }
                            KTaskPoll::Pending => {
                                unpacking.active = Some(PendingPackedArgument::IteratorTask {
                                    packed_arg_register,
                                    unpack_index,
                                    task,
                                    unpacked_values,
                                });
                                return Ok(Some(unpacking));
                            }
                        }
                    }
                    PendingPackedArgument::Iterator {
                        unpack_index,
                        mut iterator,
                        mut unpacked_values,
                    } => {
                        let max_unpacked_args = (u8::MAX - info.arg_count - 1) as usize; // -1 for frame base

                        loop {
                            let next_output = if let Some(waker) = self.task_waker.clone() {
                                let mut context = Context::from_waker(&waker);
                                iterator.next_output_with_context(&mut context)
                            } else {
                                iterator.next_output()
                            };

                            match next_output {
                                KIteratorNext::Output(KIteratorOutput::Value(value)) => {
                                    if unpacked_values.len() == max_unpacked_args {
                                        return runtime_error!(
                                            "Call argument limit reached during unpacking"
                                        );
                                    }
                                    unpacked_values.push(value);
                                }
                                KIteratorNext::Output(KIteratorOutput::ValuePair(a, b)) => {
                                    if unpacked_values.len() == max_unpacked_args {
                                        return runtime_error!(
                                            "Call argument limit reached during unpacking"
                                        );
                                    }
                                    unpacked_values.push(KTuple::from(&[a, b]).into());
                                }
                                KIteratorNext::Output(KIteratorOutput::Error(error)) => {
                                    return Err(error);
                                }
                                KIteratorNext::Pending => {
                                    unpacking.active = Some(PendingPackedArgument::Iterator {
                                        unpack_index,
                                        iterator,
                                        unpacked_values,
                                    });
                                    return Ok(Some(unpacking));
                                }
                                KIteratorNext::Done => {
                                    Self::finish_packed_argument(
                                        &mut self.registers,
                                        info,
                                        unpack_index,
                                        &mut unpacked_values,
                                    );
                                    break;
                                }
                            }
                        }
                    }
                }
            } else if let Some(packed_arg_register) = unpacking
                .packed_arg_registers
                .get(unpacking.next_packed_arg_index)
            {
                unpacking.next_packed_arg_index += 1;

                // Get the index of the argument that needs to be unpacked,
                // taking in to account the offset resulting from unpacking previous packed
                // arguments. Packed arguments can be empty, which can result in a negative offset,
                // e.g. `f []..., x...`
                //         ^ The first argument is empty, so the second argument is shifted by -1
                let arg_offset = info.arg_count as isize - unpacking.original_arg_count;
                let unpack_index = ((unpacking.first_arg_index + packed_arg_register) as isize
                    + arg_offset) as usize;

                // First, swap-remove the argument to be unpacked,
                // replacing the argument with null and keeping any trailing registers in place.
                self.registers.push(KValue::Null);
                let iterable = self.registers.swap_remove(unpack_index);

                // Convert the value into an iterator.
                let task = self.make_iterator_as_task(iterable).map_err(|error| {
                    error.with_context(format!(
                        "while unpacking argument at index {packed_arg_register}"
                    ))
                })?;
                unpacking.active = Some(PendingPackedArgument::IteratorTask {
                    packed_arg_register: *packed_arg_register,
                    unpack_index,
                    task,
                    unpacked_values: ValueVec::new(),
                });
            } else {
                return Ok(None);
            }
        }
    }

    fn finish_packed_argument(
        registers: &mut Vec<KValue>,
        info: &mut CallInfo,
        unpack_index: usize,
        unpacked_values: &mut ValueVec,
    ) {
        info.arg_count -= 1; // Subtract 1 for the arg that was unpacked
        info.arg_count += unpacked_values.len() as u8; // Add the unpacked value count

        // Splice the unpacked args into the register stack, replacing the register that
        // was occupied by the original argument.
        registers.splice(unpack_index..unpack_index + 1, unpacked_values.drain(..));
    }

    fn run_debug_instruction(
        &mut self,
        register: u8,
        expression_constant: ConstantIndex,
    ) -> Result<ControlFlow> {
        let value = self.clone_register(register);
        let prefix = self.debug_instruction_prefix();
        let expression_string = self.get_constant_str(expression_constant).to_string();

        let value_string = match self.run_unary_op(UnaryOp::Debug, value)? {
            VmOutput::Ready(KValue::Str(s)) => s,
            VmOutput::Pending(task) => {
                return self.pending_debug_instruction(task, prefix, expression_string);
            }
            VmOutput::Ready(unexpected) => {
                return unexpected_type("a displayable value", &unexpected);
            }
        };

        self.finish_debug_instruction(&prefix, &expression_string, value_string.as_ref())
    }

    fn pending_debug_instruction(
        &mut self,
        task: KTask,
        prefix: String,
        expression_string: String,
    ) -> Result<ControlFlow> {
        self.set_pending_operation(PendingOperation::DebugInstruction(
            PendingDebugInstruction {
                task,
                prefix,
                expression_string,
            },
        ))?;
        Ok(ControlFlow::Pending)
    }

    fn debug_instruction_prefix(&self) -> String {
        match (
            self.reader
                .chunk
                .debug_info
                .get_source_span(self.instruction_ip),
            self.reader.chunk.path.as_ref(),
        ) {
            (Some(span), Some(path)) => format!("[{}: {}] ", path, span.start.line + 1),
            (Some(span), None) => format!("[{}] ", span.start.line + 1),
            (None, Some(path)) => format!("[{path}: #ERR] "),
            (None, None) => "[#ERR] ".to_string(),
        }
    }

    fn finish_debug_instruction(
        &self,
        prefix: &str,
        expression_string: &str,
        value_string: &str,
    ) -> Result<ControlFlow> {
        self.stdout()
            .write_line(&format!("{prefix}{expression_string}: {value_string}"))?;
        Ok(ControlFlow::Continue)
    }

    fn poll_pending_debug_instruction(
        &mut self,
        mut pending: PendingDebugInstruction,
    ) -> Result<ControlFlow> {
        let poll_result = if let Some(waker) = self.task_waker.clone() {
            let mut context = Context::from_waker(&waker);
            pending.task.poll_with_context(&mut context)?
        } else {
            pending.task.poll()?
        };

        match poll_result {
            KTaskPoll::Ready(KValue::Str(value_string)) => self.finish_debug_instruction(
                &pending.prefix,
                &pending.expression_string,
                value_string.as_ref(),
            ),
            KTaskPoll::Ready(unexpected) => unexpected_type("a displayable value", &unexpected),
            KTaskPoll::Pending => {
                self.set_pending_operation(PendingOperation::DebugInstruction(pending))?;
                Ok(ControlFlow::Pending)
            }
        }
    }

    fn run_check_size_equal(
        &mut self,
        value_register: u8,
        expected_size: usize,
    ) -> Result<ControlFlow> {
        self.run_check_size(value_register, expected_size, CheckSizeMode::Equal)
    }

    fn run_check_size_min(
        &mut self,
        value_register: u8,
        expected_size: usize,
    ) -> Result<ControlFlow> {
        self.run_check_size(value_register, expected_size, CheckSizeMode::Min)
    }

    fn run_check_size(
        &mut self,
        value_register: u8,
        expected_size: usize,
        mode: CheckSizeMode,
    ) -> Result<ControlFlow> {
        match self.run_unary_op(UnaryOp::Size, self.clone_register(value_register))? {
            VmOutput::Ready(size) => self.finish_check_size(size, expected_size, mode),
            VmOutput::Pending(task) => {
                self.set_pending_operation(PendingOperation::CheckSize(PendingCheckSize {
                    task,
                    expected_size,
                    mode,
                }))?;
                Ok(ControlFlow::Pending)
            }
        }
    }

    fn finish_check_size(
        &self,
        size: KValue,
        expected_size: usize,
        mode: CheckSizeMode,
    ) -> Result<ControlFlow> {
        let KValue::Number(size) = size else {
            return unexpected_type("number for value size", &size);
        };
        let size = usize::from(size);

        match mode {
            CheckSizeMode::Equal if size == expected_size => Ok(ControlFlow::Continue),
            CheckSizeMode::Equal => {
                runtime_error!("the container has a size of '{size}', expected '{expected_size}'")
            }
            CheckSizeMode::Min if size >= expected_size => Ok(ControlFlow::Continue),
            CheckSizeMode::Min => {
                runtime_error!(
                    "The container has a size of '{size}', expected a minimum of  '{expected_size}'"
                )
            }
        }
    }

    fn poll_pending_check_size(&mut self, mut pending: PendingCheckSize) -> Result<ControlFlow> {
        let poll_result = if let Some(waker) = self.task_waker.clone() {
            let mut context = Context::from_waker(&waker);
            pending.task.poll_with_context(&mut context)?
        } else {
            pending.task.poll()?
        };

        match poll_result {
            KTaskPoll::Ready(size) => {
                self.finish_check_size(size, pending.expected_size, pending.mode)
            }
            KTaskPoll::Pending => {
                self.set_pending_operation(PendingOperation::CheckSize(pending))?;
                Ok(ControlFlow::Pending)
            }
        }
    }

    fn run_assert_type(
        &self,
        value_register: u8,
        type_index: ConstantIndex,
        allow_null: bool,
    ) -> Result<()> {
        if self.compare_value_type(value_register, type_index, allow_null) {
            Ok(())
        } else {
            let expected_type = self.get_constant_str(type_index);
            let value = self.get_register(value_register);
            if allow_null {
                unexpected_type(&format!("{expected_type}?"), value)
            } else {
                unexpected_type(expected_type, value)
            }
        }
    }

    fn run_check_type(
        &mut self,
        value_register: u8,
        jump_offset: u32,
        type_index: ConstantIndex,
        allow_null: bool,
    ) -> Result<()> {
        if !self.compare_value_type(value_register, type_index, allow_null) {
            self.jump_ip(jump_offset);
        }
        Ok(())
    }

    fn compare_value_type(
        &self,
        value_register: u8,
        type_index: ConstantIndex,
        allow_null: bool,
    ) -> bool {
        let value = self.get_register(value_register);

        if allow_null && matches!(value, KValue::Null) {
            return true;
        }

        match self.get_constant_str(type_index) {
            "Any" => true,
            "Callable" => value.is_callable(),
            "Indexable" => value.is_indexable(),
            "Iterable" => value.is_iterable(),
            expected_type => {
                if value.type_as_string() == expected_type {
                    true
                } else {
                    // The type didn't match, so look for a base value to check
                    let mut value = value.clone();

                    loop {
                        match value {
                            KValue::Map(m) if m.contains_meta_key(&MetaKey::Base) => {
                                let base = m.get_meta_value(&MetaKey::Base).unwrap();
                                if base.type_as_string() == expected_type {
                                    return true;
                                } else {
                                    // The base didn't match the expected type,
                                    // but continue looping to check the base's base.
                                    value = base;
                                }
                            }
                            _ => break,
                        }
                    }

                    false
                }
            }
        }
    }

    fn run_sequence_push(&mut self, value_register: u8) -> Result<()> {
        let value = self.clone_register(value_register);
        if let Some(builder) = self.sequence_builders.last_mut() {
            builder.push(value);
            Ok(())
        } else {
            runtime_error!(ErrorKind::MissingSequenceBuilder)
        }
    }

    fn run_sequence_to_list(&mut self, register: u8) -> Result<()> {
        if let Some(result) = self.sequence_builders.pop() {
            let list = KList::with_data(ValueVec::from_vec(result));
            self.set_register(register, list.into());
            Ok(())
        } else {
            runtime_error!(ErrorKind::MissingSequenceBuilder)
        }
    }

    fn run_sequence_to_tuple(&mut self, register: u8) -> Result<()> {
        if let Some(result) = self.sequence_builders.pop() {
            self.set_register(register, KTuple::from(result).into());
            Ok(())
        } else {
            runtime_error!(ErrorKind::MissingSequenceBuilder)
        }
    }

    fn run_string_push(
        &mut self,
        value_register: u8,
        format_options: Option<StringFormatOptions>,
    ) -> Result<ControlFlow> {
        let value = self.clone_register(value_register);
        let value_is_number = matches!(&value, KValue::Number(_));

        // Render the value as a string, applying the precision option if specified
        let precision = format_options.and_then(|options| options.precision);
        let representation = format_options.and_then(|options| options.representation);
        let rendered = match value {
            KValue::Number(n) => match (precision, representation) {
                (_, Some(representation)) => {
                    let n = i64::from(n);
                    match representation {
                        StringFormatRepresentation::Debug => format!("{n:?}"),
                        StringFormatRepresentation::HexLower => format!("{n:x}"),
                        StringFormatRepresentation::HexUpper => format!("{n:X}"),
                        StringFormatRepresentation::Binary => format!("{n:b}"),
                        StringFormatRepresentation::Octal => format!("{n:o}"),
                        StringFormatRepresentation::ExpLower => format!("{n:e}"),
                        StringFormatRepresentation::ExpUpper => format!("{n:E}"),
                    }
                }
                (Some(precision), None) if n.is_f64() || n.is_i64_in_f64_range() => {
                    format!("{:.*}", precision as usize, f64::from(n))
                }
                _ => n.to_string(),
            },
            other => match representation {
                Some(StringFormatRepresentation::Debug) => {
                    match self.run_unary_op(UnaryOp::Debug, other)? {
                        VmOutput::Ready(KValue::Str(rendered)) => {
                            truncate_string(rendered.as_str(), precision)
                        }
                        VmOutput::Pending(task) => {
                            self.set_pending_operation(PendingOperation::StringPush(
                                PendingStringPush {
                                    task,
                                    format_options,
                                    value_is_number,
                                },
                            ))?;
                            return Ok(ControlFlow::Pending);
                        }
                        VmOutput::Ready(other) => return unexpected_type("String", &other),
                    }
                }
                _ => match self.run_unary_op(UnaryOp::Display, other)? {
                    VmOutput::Ready(KValue::Str(rendered)) => {
                        truncate_string(rendered.as_str(), precision)
                    }
                    VmOutput::Pending(task) => {
                        self.set_pending_operation(PendingOperation::StringPush(
                            PendingStringPush {
                                task,
                                format_options,
                                value_is_number,
                            },
                        ))?;
                        return Ok(ControlFlow::Pending);
                    }
                    VmOutput::Ready(other) => return unexpected_type("String", &other),
                },
            },
        };

        self.finish_string_push(rendered, format_options, value_is_number)?;
        Ok(ControlFlow::Continue)
    }

    fn poll_pending_string_push(&mut self, mut pending: PendingStringPush) -> Result<ControlFlow> {
        let poll_result = if let Some(waker) = self.task_waker.clone() {
            let mut context = Context::from_waker(&waker);
            self.poll_task_with_context(&mut pending.task, &mut context)?
        } else {
            self.poll_task(&mut pending.task)?
        };

        match poll_result {
            KTaskPoll::Ready(KValue::Str(rendered)) => {
                let rendered = truncate_string(
                    rendered.as_str(),
                    pending.format_options.and_then(|options| options.precision),
                );
                self.finish_string_push(rendered, pending.format_options, pending.value_is_number)?;
                Ok(ControlFlow::Continue)
            }
            KTaskPoll::Ready(unexpected) => unexpected_type("String", &unexpected),
            KTaskPoll::Pending => {
                self.set_pending_operation(PendingOperation::StringPush(pending))?;
                Ok(ControlFlow::Pending)
            }
        }
    }

    fn finish_string_push(
        &mut self,
        rendered: String,
        format_options: Option<StringFormatOptions>,
        value_is_number: bool,
    ) -> Result<()> {
        // Apply other formatting options to the rendered string
        let result = match format_options {
            Some(options) => {
                let len = rendered.graphemes(true).count();
                let min_width = options.min_width.unwrap_or(0) as usize;
                if len < min_width {
                    let fill = match options.fill_character {
                        Some(constant) => self.koto_string_from_constant(constant),
                        None => KString::from(" "),
                    };
                    let fill_chars = min_width - len;

                    match options.alignment {
                        StringAlignment::Default => {
                            if value_is_number {
                                // Right-alignment by default for numbers
                                fill.repeat(fill_chars) + &rendered
                            } else {
                                // Left alignment by default for non-numbers
                                rendered + &fill.repeat(fill_chars)
                            }
                        }
                        StringAlignment::Left => rendered + &fill.repeat(fill_chars),
                        StringAlignment::Center => {
                            let half_fill_chars = fill_chars as f32 / 2.0;
                            format!(
                                "{}{}{}",
                                fill.repeat(half_fill_chars.floor() as usize),
                                rendered,
                                fill.repeat(half_fill_chars.ceil() as usize),
                            )
                        }
                        StringAlignment::Right => fill.repeat(fill_chars) + &rendered,
                    }
                } else {
                    rendered
                }
            }
            None => rendered,
        };

        // Add the result to the string builder
        if let Some(builder) = self.string_builders.last_mut() {
            builder.push_str(&result);
            Ok(())
        } else {
            runtime_error!(ErrorKind::MissingStringBuilder)
        }
    }

    fn run_string_finish(&mut self, register: u8) -> Result<()> {
        // Move the string builder out of its register to avoid cloning the string data
        if let Some(result) = self.string_builders.pop() {
            self.set_register(register, result.into());
            Ok(())
        } else {
            runtime_error!(ErrorKind::MissingStringBuilder)
        }
    }

    /// The bytecode chunk currently active in the VM
    pub fn chunk(&self) -> Ptr<Chunk> {
        self.reader.chunk.clone()
    }

    /// The ip that produced the most recently executed instruction
    ///
    /// For native functions accessing the VM from [`CallContext`] or [`MethodContext`],
    /// this will refer to the call instruction currently being executed,
    /// which can be useful for building error messages with more informative stack traces.
    pub fn instruction_frame(&self) -> InstructionFrame {
        InstructionFrame {
            chunk: self.chunk(),
            instruction: self.instruction_ip,
        }
    }

    fn set_chunk_and_ip(&mut self, chunk: Ptr<Chunk>, ip: u32) {
        self.reader = InstructionReader {
            chunk,
            ip: ip as usize,
        };
    }

    fn ip(&self) -> u32 {
        self.reader.ip as u32
    }

    fn set_ip(&mut self, ip: u32) {
        self.reader.ip = ip as usize;
    }

    fn jump_ip(&mut self, offset: u32) {
        self.reader.ip += offset as usize;
    }

    fn jump_ip_back(&mut self, offset: u32) {
        self.reader.ip -= offset as usize;
    }

    fn frame(&self) -> &Frame {
        self.call_stack.last().expect("Empty call stack")
    }

    fn frame_mut(&mut self) -> &mut Frame {
        self.call_stack.last_mut().expect("Empty call stack")
    }

    // Pushes a new frame onto the call stack
    //
    // This is used for the main top-level frame, and for any function calls.
    //
    // - The `frame_base` register should already exist in the register stack.
    // - If the new frame's return value should be copied to a register in the calling frame,
    //   then `return_register` should be lower in the stack than `frame_base`.
    // - The frame will use the provided `non_locals` if they're defined, otherwise the frame will
    //   inherit the parent's non-locals.
    fn push_frame(
        &mut self,
        chunk: Ptr<Chunk>,
        ip: u32,
        frame_base: u8,
        return_register: Option<u8>,
        non_locals: Option<NonLocals>,
    ) {
        let return_ip = self.ip();
        if let Some(frame) = self.call_stack.last_mut() {
            frame.return_instruction_ip = self.instruction_ip;
            frame.return_resume_ip = return_ip;
            frame.return_value_register = return_register;
        };

        let previous_frame_base = self.register_base;
        let new_frame_base = previous_frame_base + frame_base as usize;

        self.call_stack
            .push(Frame::new(chunk.clone(), non_locals, new_frame_base));
        self.register_base = new_frame_base;
        self.set_chunk_and_ip(chunk, ip);
    }

    // Pops the current frame from the call stack
    //
    // If there is a new current frame after popping, and if execution should continue
    // (i.e. `frame.execution_barrier` is false), the return value will be placed in the current
    // frame's return register, and `None` will be returned. Otherwise, the return value will be
    // passed back to the caller as `Some`.
    fn pop_frame(&mut self, return_value: KValue) -> Result<Option<KValue>> {
        let Some(popped_frame) = self.call_stack.pop() else {
            return runtime_error!(ErrorKind::EmptyCallStack);
        };

        if self.call_stack.is_empty() {
            // The call stack is empty, so clean up by resetting the register base.
            self.register_base = 0;
            self.min_frame_registers = 0;
            Ok(Some(return_value))
        } else {
            let return_frame = self.frame();
            let return_register = return_frame.return_value_register;
            let resume_ip = return_frame.return_resume_ip;
            let chunk = return_frame.chunk.clone();
            let return_instruction_ip = return_frame.return_instruction_ip;
            let register_base = return_frame.register_base;
            let required_registers = return_frame.required_registers;

            self.instruction_ip = return_instruction_ip;
            self.register_base = register_base;
            self.min_frame_registers = self.register_base + required_registers as usize;
            self.set_chunk_and_ip(chunk, resume_ip);

            // If the popped frame should stop execution then return the value
            if popped_frame.execution_barrier {
                Ok(Some(return_value))
            } else {
                // Execution will continue, so minimize the register stack by discarding registers
                // used by this frame that are no longer needed.
                self.registers
                    .resize(self.min_frame_registers, KValue::Null);

                if let Some(return_register) = return_register {
                    self.set_register(return_register, return_value);
                }

                Ok(None)
            }
        }
    }

    // Called when an error occurs and the stack needs to be unwound
    //
    // If `allow_catch` is true and a `catch` expression is encountered then the recovery register
    // and ip will be returned. Otherwise, the error will be returned with the popped frames added
    // to the error's stack trace.
    fn pop_call_stack_on_error(
        &mut self,
        mut error: Error,
        allow_catch: bool,
    ) -> Result<(u8, u32)> {
        error.extend_trace(self.instruction_frame());

        while let Some(frame) = self.call_stack.last() {
            match frame.catch_stack.last() {
                Some((error_register, catch_ip)) if allow_catch => {
                    return Ok((*error_register, *catch_ip));
                }
                _ => {
                    if frame.execution_barrier {
                        break;
                    }

                    self.pop_frame(KValue::Null)?;

                    if !self.call_stack.is_empty() {
                        error.extend_trace(self.instruction_frame());
                    }
                }
            }
        }

        Err(error)
    }

    fn new_frame_base(&self) -> Result<u8> {
        u8::try_from(self.registers.len() - self.register_base)
            .map_err(|_| "Overflow of the current frame's register stack".into())
    }

    fn register_index(&self, register: u8) -> usize {
        self.register_base + register as usize
    }

    // Returns the register id that corresponds to the next push to the value stack
    fn next_register(&self) -> u8 {
        (self.registers.len() - self.register_base) as u8
    }

    // Sets the register, which must already be available in the stack
    fn set_register(&mut self, register: u8, value: KValue) {
        let index = self.register_index(register);
        self.registers[index] = value;
    }

    #[track_caller]
    fn clone_register(&self, register: u8) -> KValue {
        self.get_register(register).clone()
    }

    // Moves the register's value out of the stack, replacing it with null
    #[track_caller]
    fn remove_register(&mut self, register: u8) -> KValue {
        self.registers.push(KValue::Null);
        self.registers.swap_remove(self.register_index(register))
    }

    #[track_caller]
    pub(crate) fn get_register(&self, register: u8) -> &KValue {
        let index = self.register_index(register);
        match self.registers.get(index) {
            Some(value) => value,
            None => {
                panic!(
                    "Out of bounds access, index: {index}, register: {register}, ip: {}
  Caller: {}",
                    self.instruction_ip,
                    std::panic::Location::caller()
                );
            }
        }
    }

    pub(crate) fn get_register_safe(&self, register: u8) -> Option<&KValue> {
        let index = self.register_index(register);
        self.registers.get(index)
    }

    fn get_register_mut(&mut self, register: u8) -> &mut KValue {
        let index = self.register_index(register);
        &mut self.registers[index]
    }

    // Provides a slice of registers, with a start register relative to the current frame base.
    pub(crate) fn register_slice(&self, start: u8, count: u8) -> &[KValue] {
        if count > 0 {
            let start = self.register_index(start);
            &self.registers[start..start + count as usize]
        } else {
            &[]
        }
    }

    // Provides a slice of registers, with a start register index in the register stack.
    pub(crate) fn register_slice_raw(&self, start: usize, count: usize) -> &[KValue] {
        &self.registers[start..start + count]
    }

    fn truncate_registers(&mut self, len: u8) {
        self.registers.truncate(self.register_base + len as usize);
    }

    fn get_constant_str(&self, constant_index: ConstantIndex) -> &str {
        self.reader.chunk.constants.get_str(constant_index)
    }

    fn koto_string_from_constant(&self, constant_index: ConstantIndex) -> KString {
        self.reader
            .chunk
            .constants
            .get_string_slice(constant_index)
            .into()
    }
}

impl fmt::Debug for KotoVm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Vm")
    }
}

fn binary_op_error(lhs: &KValue, rhs: &KValue, op: BinaryOp) -> Result<()> {
    runtime_error!(ErrorKind::InvalidBinaryOp {
        lhs: lhs.clone(),
        rhs: rhs.clone(),
        op,
    })
}

fn signed_index_to_unsigned(index: i8, size: usize) -> usize {
    if index < 0 {
        size - (index as isize).unsigned_abs().min(size)
    } else {
        index as usize
    }
}

// See [KotoVm::call_koto_function] and [KotoVm::call_generator]
fn apply_optional_arguments(
    registers: &mut Vec<KValue>,
    f: &KFunction,
    call_arg_count: u8,
    expected_arg_count: u8,
) -> Result<()> {
    if call_arg_count < expected_arg_count {
        let default_values_to_apply = (expected_arg_count - call_arg_count) as usize;
        let optional_arg_count = f.optional_arg_count as usize;
        if default_values_to_apply > optional_arg_count {
            return runtime_error!(ErrorKind::InsufficientArguments {
                expected: f.arg_count - f.optional_arg_count,
                actual: call_arg_count,
            });
        }

        let Some(captures) = f.captures() else {
            // Non-zero default arg count without captures is unexpected
            return runtime_error!(ErrorKind::UnexpectedError);
        };
        if captures.len() < default_values_to_apply {
            // There should never be fewer captures than default args
            return runtime_error!(ErrorKind::UnexpectedError);
        }

        let default_values_to_skip = optional_arg_count - default_values_to_apply;
        registers.extend(
            captures
                .data()
                .iter()
                .skip(default_values_to_skip)
                .take(default_values_to_apply)
                .cloned(),
        );
    }

    Ok(())
}

// See [KotoVm::call_koto_function] and [KotoVm::call_generator]
fn apply_variadic_arguments(
    registers: &mut Vec<KValue>,
    arg_base_index: usize, // The index in `registers` of the first call arg
    call_info: &CallInfo,
    f: &KFunction,
    expected_arg_count: u8,
) -> Result<()> {
    if f.flags.is_variadic() {
        // The last defined arg is the start of the var_args,
        // e.g. f = |x, y, z...|
        // arg index 2 is the first vararg, and where the tuple will be placed
        let varargs_count = call_info.arg_count.saturating_sub(expected_arg_count) as usize;
        let varargs_start = arg_base_index + expected_arg_count as usize;
        let varargs = if call_info.arg_count >= expected_arg_count {
            KTuple::from(&registers[varargs_start..varargs_start + varargs_count])
        } else {
            KTuple::default()
        };
        // Remove the variadic args from the register stack
        registers.resize(varargs_start, KValue::Null);
        // Push the variadic args back on to the stack as a tuple
        registers.push(KValue::Tuple(varargs));
    } else if call_info.arg_count > expected_arg_count {
        return runtime_error!(ErrorKind::TooManyArguments {
            expected: expected_arg_count,
            actual: call_info.arg_count
        });
    }
    Ok(())
}

async fn compare_value_ranges(
    runner: &mut AsyncKotoVm,
    range_a: ValueVec,
    range_b: ValueVec,
) -> Result<bool> {
    for (value_a, value_b) in range_a.into_iter().zip(range_b) {
        let result = runner
            .run_binary_op(BinaryOp::Equal, value_a, value_b)
            .await?;
        match comparison_bool(result)? {
            true => {}
            false => return Ok(false),
        }
    }

    Ok(true)
}

async fn compare_value_maps(
    runner: &mut AsyncKotoVm,
    map_a: ValueMap,
    map_b: ValueMap,
) -> Result<bool> {
    for (key_a, value_a) in map_a.iter() {
        let Some(value_b) = map_b.get(key_a).cloned() else {
            return Ok(false);
        };
        let result = runner
            .run_binary_op(BinaryOp::Equal, value_a.clone(), value_b)
            .await?;
        match comparison_bool(result)? {
            true => {}
            false => return Ok(false),
        }
    }

    Ok(true)
}

async fn call_overridden_comparison(
    runner: &mut AsyncKotoVm,
    instance: KValue,
    arg: KValue,
    op: KValue,
) -> Result<bool> {
    let result = runner
        .call_instance_function_with_args(instance, op, vec![arg])
        .await?;
    comparison_bool(result)
}

fn comparison_bool(value: KValue) -> Result<bool> {
    match value {
        KValue::Bool(result) => Ok(result),
        unexpected => runtime_error!(
            "Expected Bool from comparison, found '{}'",
            unexpected.type_as_string()
        ),
    }
}

// See [KotoVm::call_koto_function] and [KotoVm::call_generator]
fn truncate_string(rendered: &str, precision: Option<u32>) -> String {
    match precision {
        Some(precision) => {
            // `precision` acts as a maximum width for non-number values
            let mut truncated = String::with_capacity((precision as usize).min(rendered.len()));
            for grapheme in rendered.graphemes(true).take(precision as usize) {
                truncated.push_str(grapheme);
            }
            truncated
        }
        None => rendered.to_owned(),
    }
}

fn apply_captures(registers: &mut Vec<KValue>, f: &KFunction) {
    if let Some(captures) = f.captures() {
        // Copy the captures list into the registers following the args
        registers.extend(
            captures
                .data()
                .iter()
                .skip(f.optional_arg_count as usize)
                .cloned(),
        );
    }
}

// Used when calling iterator.copy on a generator
//
// The idea here is to clone the VM, and then scan through the value stack to make copies of
// any iterators that it finds. This makes simple generators copyable, although any captured or
// contained iterators in the generator VM will have shared state. This behaviour is noted in the
// documentation for iterator.copy and should hopefully be sufficient.
pub(crate) fn clone_generator_vm(vm: &KotoVm) -> Result<KotoVm> {
    let mut result = vm.clone();
    for value in result.registers.iter_mut() {
        if let KValue::Iterator(i) = value {
            *i = i.make_copy()?;
        }
    }
    Ok(result)
}

/// Function call arguments
///
/// Typical use will be to use the `From` implementations, either providing a single value that
/// implements `Into<KValue>`, or an array or slice of [KValue]s.
///
/// See [KotoVm::call_function].
pub enum CallArgs<'a> {
    /// Represents a function call with a single argument.
    Single(KValue),

    /// Arguments are provided separately and are passed directly to the function.
    Separate(&'a [KValue]),

    /// Arguments are bundled together as a tuple and then passed to the function.
    ///
    /// If the called function unpacks the tuple in its arguments list,
    /// then a temporary tuple will be used, which avoids the allocation of a regular [KTuple].
    AsTuple(&'a [KValue]),
}

impl<T> From<T> for CallArgs<'static>
where
    T: Into<KValue>,
{
    fn from(value: T) -> Self {
        CallArgs::Single(value.into())
    }
}

impl<'a> From<&'a [KValue]> for CallArgs<'a> {
    fn from(args: &'a [KValue]) -> Self {
        CallArgs::Separate(args)
    }
}

impl<'a, const N: usize> From<&'a [KValue; N]> for CallArgs<'a> {
    fn from(args: &'a [KValue; N]) -> Self {
        CallArgs::Separate(args.as_ref())
    }
}

// A cache of imported module state.
type ModuleCache = HashMap<PathBuf, ModuleCacheEntry, BuildHasherDefault<FxHasher>>;

#[derive(Clone)]
enum ModuleCacheEntry {
    Loading(KTask),
    Loaded(KMap),
}

// A frame in the VM's call stack
#[derive(Clone)]
struct Frame {
    // The chunk being interpreted in this frame
    pub chunk: Ptr<Chunk>,
    // The non-local values that are available within this frame
    pub non_locals: Option<NonLocals>,
    // The index in the VM's value stack of the first frame register.
    // The frame's instance is always in register 0 (Null if not set).
    // Call arguments followed by local values are in registers starting from index 1.
    pub register_base: usize,
    // The number of registers required by this frame
    pub required_registers: u8,
    // When returning to this frame, the ip that produced the most recently read instruction
    pub return_instruction_ip: u32,
    // When returning to this frame, the ip that should be jumped to for resumed execution
    pub return_resume_ip: u32,
    // When returning to this frame, the register that should receive the return value
    pub return_value_register: Option<u8>,
    // A stack of catch points for handling errors
    pub catch_stack: Vec<(u8, u32)>, // catch error register, catch ip
    // True if the frame should prevent execution from continuing after the frame is exited.
    // e.g.
    //   - a function is being called externally from the VM
    //   - an overridden operator is being executed as a result of a regular instruction
    //   - an external function is calling back into the VM with a functor
    //   - a module is being imported
    pub execution_barrier: bool,
    // The operation that's waiting for async work to complete.
    pending_operation: Option<PendingOperation>,
}

impl Frame {
    fn new(chunk: Ptr<Chunk>, non_locals: Option<NonLocals>, register_base: usize) -> Self {
        Self {
            chunk,
            non_locals,
            register_base,
            required_registers: 0,
            return_resume_ip: 0,
            return_value_register: None,
            return_instruction_ip: 0,
            catch_stack: vec![],
            execution_barrier: false,
            pending_operation: None,
        }
    }

    fn non_local(&self, name: &str) -> Option<KValue> {
        self.non_locals
            .as_ref()
            .and_then(|non_locals| non_locals.get(name))
    }
}

#[derive(Clone, Default)]
pub struct NonLocals {
    wildcard_imports: Option<Ptr<Vec<KValue>>>,
    module_exports: KMap,
}

impl NonLocals {
    fn get(&self, name: &str) -> Option<KValue> {
        if let Some(wildcard_imports) = &self.wildcard_imports {
            // Check any wildcard imports in reverse order (most recent import takes precedence)
            for wildcard_import in wildcard_imports.iter().rev() {
                let result = match wildcard_import {
                    KValue::Map(m) => m.get(name),
                    KValue::Object(o) => o
                        .try_borrow()
                        .ok()
                        .and_then(|o| o.access(&name.into()).ok().flatten()),
                    _ => None,
                };
                if let Some(result) = result {
                    return Some(result);
                }
            }
        }

        // Check the module's exports
        self.module_exports.get(name)
    }

    fn add_wildcard_import(&mut self, new_import: KValue) {
        let already_imported = self
            .wildcard_imports
            .as_ref()
            .and_then(|imports| {
                imports
                    .iter()
                    .find(|import| import.is_same_instance(&new_import))
            })
            .is_some();

        if !already_imported {
            Ptr::make_mut(self.wildcard_imports.get_or_insert_default()).push(new_import);
        }
    }
}

#[derive(Clone)]
enum PendingOperation {
    NativeCall(PendingNativeCall),
    PackedCall(PendingPackedCall),
    ImplicitTask(PendingImplicitTask),
    DebugInstruction(PendingDebugInstruction),
    CheckSize(PendingCheckSize),
    Slice(PendingSlice),
    StringPush(PendingStringPush),
    Import(PendingImport),
}

#[derive(Clone)]
struct PendingNativeCall {
    result_register: Option<u8>,
    frame_base: u8,
    task: KTask,
}

#[derive(Clone)]
struct PendingPackedCall {
    call_info: CallInfo,
    callable: KValue,
    pending_behavior: PendingCallBehavior,
    unpacking: PackedArgumentUnpacking,
}

#[derive(Clone)]
struct PackedArgumentUnpacking {
    first_arg_index: usize,
    packed_arg_registers: SmallVec<[usize; 4]>,
    next_packed_arg_index: usize,
    original_arg_count: isize,
    active: Option<PendingPackedArgument>,
}

#[derive(Clone)]
enum PendingPackedArgument {
    IteratorTask {
        packed_arg_register: usize,
        unpack_index: usize,
        task: KTask,
        unpacked_values: ValueVec,
    },
    Iterator {
        unpack_index: usize,
        iterator: KIterator,
        unpacked_values: ValueVec,
    },
}

#[derive(Clone)]
struct PendingImplicitTask {
    result_register: u8,
    task: KTask,
    truncate_registers_to: Option<usize>,
}

#[derive(Clone)]
struct PendingDebugInstruction {
    task: KTask,
    prefix: String,
    expression_string: String,
}

#[derive(Clone)]
struct PendingCheckSize {
    task: KTask,
    expected_size: usize,
    mode: CheckSizeMode,
}

#[derive(Clone, Copy)]
enum CheckSizeMode {
    Equal,
    Min,
}

#[derive(Clone)]
enum PendingSlice {
    Size {
        result_register: u8,
        map: KMap,
        index: i8,
        is_slice_to: bool,
        task: KTask,
    },
    Read {
        result_register: u8,
        task: KTask,
    },
}

enum PendingSliceContinuation {
    Size {
        map: KMap,
        index: i8,
        is_slice_to: bool,
    },
    Read,
}

#[derive(Clone)]
struct PendingStringPush {
    task: KTask,
    format_options: Option<StringFormatOptions>,
    value_is_number: bool,
}

#[derive(Clone)]
struct PendingImport {
    import_register: u8,
    import_all: bool,
    import_name: KString,
    module_path: PathBuf,
    task: KTask,
    remove_cache_on_sync_error: bool,
}

// See Vm::call_external
enum ExternalCallable {
    Function(KNativeFunction),
    VmFunction(KNativeVmFunction),
    Object(KObject),
}

#[derive(Clone, Copy)]
enum PendingCallBehavior {
    Suspend,
    ReturnTask,
}

#[derive(Clone, Copy)]
enum ComparisonFallback {
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

// See Vm::call_callable
#[derive(Clone, Debug)]
struct CallInfo {
    result_register: Option<u8>,
    frame_base: u8,
    instance: Option<u8>,
    arg_count: u8,
    packed_arg_count: u8,
}

struct ExecutionTimeout {
    // The instant at which the deadline was last checked
    last_check: Instant,
    // The time at which a timeout will be reached
    deadline: Instant,
    // The target number of seconds to wait between deadline checks
    interval_seconds: f64,
    // The number of instructions that should elapse before the next check
    interval_instructions: usize,
    // The number of instructions that have elapsed since the last check
    instructions_since_last_check: usize,
    // The maximum amount of time that execution is allowed to take
    execution_limit: Duration,
}

impl ExecutionTimeout {
    fn new(execution_limit: Duration) -> Self {
        let now = Instant::now();
        let interval_seconds = (execution_limit / 10).as_secs_f64();

        // A rough baseline instruction count that gets adjusted per interval based on the actual
        // execution duration.
        let first_interval_instruction_count = if cfg!(debug_assertions) {
            10_000_000.0
        } else {
            100_000_000.0
        } * interval_seconds;

        Self {
            last_check: now,
            deadline: now + execution_limit,
            interval_seconds,
            interval_instructions: first_interval_instruction_count as usize,
            instructions_since_last_check: 0,
            execution_limit,
        }
    }

    // Returns true if the deadline has been reached, and false otherwise
    //
    // This should only be called once per instruction.
    fn check_for_timeout(&mut self) -> bool {
        if self.instructions_since_last_check < self.interval_instructions {
            self.instructions_since_last_check += 1;
            false
        } else {
            let now = Instant::now();
            if now >= self.deadline {
                true
            } else {
                // If the deadline is near then use the remaining time as the next interval's
                // duration.
                let remaining = (self.deadline - now).as_secs_f64();
                let next_interval_duration = self.interval_seconds.min(remaining);

                // Adjust the interval based on how much time elapsed in the previous interval
                // compared to the next interval's target duration.
                let elapsed = (now - self.last_check).as_secs_f64();
                let interval_adjustment = next_interval_duration / elapsed;
                self.interval_instructions =
                    (self.interval_instructions as f64 * interval_adjustment) as usize;

                self.instructions_since_last_check = 0;
                self.last_check = now;

                false
            }
        }
    }
}

/// The result of continuing a suspended [KotoVm].
#[allow(missing_docs)]
pub enum ReturnOrYield {
    Return(KValue),
    Yield(KValue),
    Pending,
}

// A collection of macros that avoid duplicated boilerplate in the various operator functions
mod macros {
    macro_rules! call_metamap_binary_op_rhs {
        ($self:expr, $op:ident, $map:expr, $lhs_value:expr, $rhs_value:expr, $result_register:expr) => {{
            let op = $map.get_meta_value(&$op.into()).unwrap();
            let lhs_value = $lhs_value.clone();
            let rhs_value = $rhs_value.clone();
            // Call the op, swapping the LHS and RHS
            return $self.call_overridden_op_2(Some($result_register), rhs_value, lhs_value, op);
        }};
    }

    macro_rules! call_object_binary_op {
        ($op:ident, $trait_fn:ident, $object:expr, $lhs_value:expr, $rhs_value:expr) => {{
            match $object.try_borrow()?.$trait_fn($lhs_value) {
                Ok(result) => result,
                Err(error) => {
                    if error.is_unimplemented_error() {
                        return binary_op_error($lhs_value, $rhs_value, $op);
                    } else {
                        return Err(error);
                    }
                }
            }
        }};
    }

    macro_rules! call_metamap_binary_op {
        // Used when the call result needs to be assigned to a register, (e.g. Add)
        ($self:expr, $op:ident, $map:expr, $lhs_value:expr, $rhs_value:expr, $result_register:expr) => {{
            let op = $map.get_meta_value(&$op.into()).unwrap();
            let lhs_value = $lhs_value.clone();
            let rhs_value = $rhs_value.clone();

            return $self.call_overridden_op_2(Some($result_register), lhs_value, rhs_value, op);
        }};

        // Used when the call result can be discarded, the result is always the modified LHS
        // (e.g. AddAssign)
        ($self:expr, $op:ident, $map:expr, $lhs_value:expr, $rhs_value:expr) => {{
            let op = $map.get_meta_value(&$op.into()).unwrap();
            let lhs_value = $lhs_value.clone();
            let rhs_value = $rhs_value.clone();
            return $self.call_overridden_op_2(None, lhs_value, rhs_value, op);
        }};
    }

    // Arithmetic ops fall back to the RHS when possible
    macro_rules! call_metamap_arithmetic_op {
        ($self:expr, $op:ident, $op_rhs:ident, $trait_fn:ident, $trait_fn_rhs:ident, $map:expr, $lhs:expr, $rhs:expr, $result_register:expr) => {{
            let op = $map.get_meta_value(&$op.into()).unwrap();
            let old_frame_count = $self.call_stack.len();

            // Call the map's op function
            $self.call_overridden_op_2(
                Some($result_register),
                $lhs.clone(),
                $rhs.clone(),
                op,
            )?;

            if $self.call_stack.len() == old_frame_count {
                $self.clone_register($result_register)
            } else {
                // Execute the function immediately so that we can check for
                // `koto.unimplemented` errors.
                $self.frame_mut().execution_barrier = true;
                match $self.execute_instructions() {
                    Ok(result) => result,
                    Err(error) => {
                        // Pop the frame given that an error has been thrown
                        $self.pop_frame(KValue::Null)?;
                        // Check for a `koto.unimplemented` error
                        let ErrorKind::KotoError { thrown_value, .. } = &error.error else {
                            // A non-unimplemented error was thrown, so propagate it
                            return Err(error);
                        };

                        if !matches!(thrown_value, KValue::Object(o) if o.is_a::<Unimplemented>())
                        {
                            // A non-unimplemented error was thrown, so propagate it
                            return Err(error);
                        }

                        match &$rhs {
                            Object(o_rhs) => {
                                call_object_binary_op!(
                                    $op_rhs,
                                    $trait_fn_rhs,
                                    o_rhs,
                                    &$lhs,
                                    &$rhs
                                )
                                .into()
                            }
                            Map(m) if m.contains_meta_key(&$op_rhs.into()) => {
                                call_metamap_binary_op_rhs!(
                                    $self,
                                    $op_rhs,
                                    m,
                                    $lhs,
                                    $rhs,
                                    $result_register
                                );
                            }
                            _ => return binary_op_error(&$lhs, &$rhs, $op),
                        }
                    }
                }
            }
        }};

        ($self:expr, $op:ident, $trait_fn:ident, $map:expr, $lhs:expr, $rhs:expr, $result_register:expr) => {
            paste::paste! {
                call_metamap_arithmetic_op!(
                    $self,
                    $op,
                    [<$op Rhs>],
                    $trait_fn,
                    [<$trait_fn _rhs>],
                    $map,
                    $lhs,
                    $rhs,
                    $result_register
                )
            }
        };
    }

    // Arithmetic ops fall back to the RHS when possible
    macro_rules! call_object_arithmetic_op {
        ($self:expr,
         $op:ident,
         $op_rhs:ident,
         $trait_fn:ident,
         $trait_fn_rhs:ident,
         $object:expr,
         $lhs_value:expr,
         $rhs_value:expr,
         $result_register:expr) => {{
            let object = $object.clone();
            match object.try_borrow()?.$trait_fn($rhs_value) {
                Ok(result) => result,
                Err(error) if error.is_unimplemented_error() => match $rhs_value {
                    Object(o_rhs) => {
                        call_object_binary_op!(
                            $op_rhs,
                            $trait_fn_rhs,
                            o_rhs,
                            $lhs_value,
                            $rhs_value
                        )
                    }
                    Map(m) if m.contains_meta_key(&$op_rhs.into()) => {
                        call_metamap_binary_op_rhs!(
                            $self,
                            $op_rhs,
                            m,
                            $lhs_value,
                            $rhs_value,
                            $result_register
                        );
                    }
                    _ => return binary_op_error($lhs_value, $rhs_value, $op),
                },
                Err(error) => return Err(error),
            }
        }};

        ($self:expr,
         $op:ident,
         $trait_fn:ident,
         $object:expr,
         $lhs_value:expr,
         $rhs_value:expr,
         $result_register:expr) => {{
            paste::paste! {
                call_object_arithmetic_op!(
                    $self,
                    $op,
                    [<$op Rhs>],
                    $trait_fn,
                    [<$trait_fn _rhs>],
                    $object,
                    $lhs_value,
                    $rhs_value,
                    $result_register
                )
            }
        }};
    }

    macro_rules! run_arithmetic_op {
        ($self:expr,
         $op:ident,
         $trait_fn:ident,
         $op_expr:expr,
         $result:expr,
         $lhs:expr,
         $rhs:expr) => {{
             paste::paste! {
                use BinaryOp::{$op, [<$op Rhs>]};
                use KValue::{Map, Number, Object};
                use macros::*;

                let lhs_value = $self.get_register($lhs);
                let rhs_value = $self.get_register($rhs);
                let result_value = match (lhs_value, rhs_value) {
                    (Number(a), Number(b)) => Number($op_expr(a, b)),
                    (Map(m), _) if m.contains_meta_key(&$op.into()) => {
                        let lhs_value = lhs_value.clone();
                        let rhs_value = rhs_value.clone();
                        call_metamap_arithmetic_op!($self, $op, $trait_fn, m, lhs_value, rhs_value, $result)
                    }
                    (Object(o), _) => {
                        call_object_arithmetic_op!($self, $op, $trait_fn, o, lhs_value, rhs_value, $result)
                    }
                    (_, Map(m)) if m.contains_meta_key(&[<$op Rhs>].into()) => {
                        call_metamap_binary_op_rhs!($self, [<$op Rhs>], m, lhs_value, rhs_value, $result);
                    }
                    (_, Object(o)) => {
                        call_object_binary_op!([<$op Rhs>], [<$trait_fn _rhs>], o, lhs_value, rhs_value)
                    }
                    _ => return binary_op_error(lhs_value, rhs_value, $op),
                };
                $self.set_register($result, result_value);

                Ok(())
            }
        }};
    }

    macro_rules! run_compound_assign_op {
        ($self:expr,
         $op:ident,
         $trait_fn:ident,
         $op_expr:expr,
         $lhs:expr,
         $rhs:expr) => {{
            paste::paste! {
                use BinaryOp::$op;
                use KValue::{Map, Number, Object};

                let lhs_value = $self.get_register($lhs);
                let rhs_value = $self.get_register($rhs);
                match (lhs_value, rhs_value) {
                    (Number(a), Number(b)) => {
                        $self.set_register($lhs, Number($op_expr(a, b)));
                        Ok(())
                    }
                    (Map(m), _) if m.contains_meta_key(&$op.into()) => {
                        macros::call_metamap_binary_op!($self, $op, m, lhs_value, rhs_value);
                    }
                    (Object(o), Object(o2)) if o.is_same_instance(o2) => {
                        let o2 = Object(o2.try_borrow()?.copy());
                        o.try_borrow_mut()?.$trait_fn(&o2)
                    }
                    (Object(o), _) => o.try_borrow_mut()?.$trait_fn(rhs_value),
                    _ => binary_op_error(lhs_value, rhs_value, $op),
                }
            }
        }};
    }

    pub(crate) use {
        call_metamap_arithmetic_op, call_metamap_binary_op, call_metamap_binary_op_rhs,
        call_object_arithmetic_op, call_object_binary_op, run_arithmetic_op,
        run_compound_assign_op,
    };
}
