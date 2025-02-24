use crate::{
    DefaultStderr, DefaultStdin, DefaultStdout, KFunction, Ptr, Result,
    core_lib::CoreLib,
    error::{Error, ErrorKind},
    prelude::*,
    types::{meta_id_to_key, value::RegisterSlice},
};
use instant::Instant;
use koto_bytecode::{Chunk, Instruction, InstructionReader, ModuleLoader};
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
    time::Duration,
};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone)]
pub enum ControlFlow {
    Continue,
    Return(KValue),
    Yield(KValue),
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
    imported_modules: KCell<ModuleCache>,
}

impl Default for VmContext {
    fn default() -> Self {
        Self::with_settings(KotoVmSettings::default())
    }
}

impl VmContext {
    fn with_settings(settings: KotoVmSettings) -> Self {
        let core_lib = CoreLib::default();

        Self {
            settings,
            prelude: core_lib.prelude(),
            core_lib,
            loader: ModuleLoader::default().into(),
            imported_modules: ModuleCache::default().into(),
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

    /// The runtime's `stdin`
    ///
    /// Default: [`DefaultStdin`]
    pub stdin: Ptr<dyn KotoFile>,

    /// The runtime's `stdout`
    ///
    /// Default: [`DefaultStdout`]
    pub stdout: Ptr<dyn KotoFile>,

    /// The runtime's `stderr`
    ///
    /// Default: [`DefaultStderr`]
    pub stderr: Ptr<dyn KotoFile>,
}

impl Default for KotoVmSettings {
    fn default() -> Self {
        Self {
            run_import_tests: true,
            execution_limit: None,
            module_imported_callback: None,
            stdin: make_ptr!(DefaultStdin::default()),
            stdout: make_ptr!(DefaultStdout::default()),
            stderr: make_ptr!(DefaultStderr::default()),
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
            execution_state: ExecutionState::Inactive,
        }
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

    /// Runs the provided [Chunk], returning the resulting [KValue]
    pub fn run(&mut self, chunk: Ptr<Chunk>) -> Result<KValue> {
        // Set up an execution frame to run the chunk in
        let frame_base = self.next_register();
        self.registers.push(KValue::Null); // Instance register
        self.push_frame(chunk, 0, frame_base, None);

        // Ensure that execution stops here if an error is thrown
        self.frame_mut().execution_barrier = true;

        // Run the chunk
        let result = self.execute_instructions();
        if result.is_err() {
            self.pop_frame(KValue::Null)?;
        }

        // Reset the register stack back to where it was at the start of the run
        self.truncate_registers(frame_base);
        result
    }

    /// Continues execution in a suspended VM
    ///
    /// This is currently used to support generators, which yield incremental results and then
    /// leave the VM in a suspended state.
    pub fn continue_running(&mut self) -> Result<ReturnOrYield> {
        if self.call_stack.is_empty() {
            return Ok(ReturnOrYield::Return(KValue::Null));
        }

        let result = self.execute_instructions()?;

        match self.execution_state {
            ExecutionState::Inactive => Ok(ReturnOrYield::Return(result)),
            ExecutionState::Suspended => Ok(ReturnOrYield::Yield(result)),
            ExecutionState::Active => unreachable!(),
        }
    }

    /// Calls a function with some given arguments
    pub fn call_function<'a>(
        &mut self,
        function: KValue,
        args: impl Into<CallArgs<'a>>,
    ) -> Result<KValue> {
        self.call_and_run_function(None, function, args.into())
    }

    /// Runs an instance function with some given arguments
    pub fn call_instance_function<'a>(
        &mut self,
        instance: KValue,
        function: KValue,
        args: impl Into<CallArgs<'a>>,
    ) -> Result<KValue> {
        self.call_and_run_function(Some(instance), function, args.into())
    }

    fn call_and_run_function(
        &mut self,
        instance: Option<KValue>,
        function: KValue,
        args: CallArgs,
    ) -> Result<KValue> {
        if !function.is_callable() {
            return runtime_error!("run_function: the provided value isn't a function");
        }

        let result_register = self.next_register();
        let frame_base = result_register + 1;

        self.registers.push(KValue::Null); // Result register
        self.registers.push(instance.unwrap_or_default()); // Frame base

        let (arg_count, temp_tuple_values) = match args {
            CallArgs::Single(arg) => {
                self.registers.push(arg);
                (1, None)
            }
            CallArgs::Separate(args) => {
                self.registers.extend_from_slice(args);
                (args.len() as u8, None)
            }
            CallArgs::AsTuple(args) => {
                // If the function has a single arg which is an unpacked tuple,
                // then the tuple contents can go into a temporary tuple.
                //
                // The temp tuple goes into the first arg register, the function's captures
                // follow, and then the temp tuple contents can be placed in the registers
                // following the captures. The captures and temp tuple contents are added
                // to the value stack in call_function/call_generator, here we only need to
                // add the temp tuple itself.
                //
                // At runtime the unpacking instructions will still be executed, resulting
                // in the tuple values being unpacked into the same registers that they're
                // already in. This is redundant work, but more efficient than allocating a
                // non-temporary Tuple for the values.
                match &function {
                    KValue::Function(f) if f.flags.arg_is_unpacked_tuple() => {
                        let capture_count = f
                            .captures
                            .as_ref()
                            .map(|captures| captures.len())
                            .unwrap_or(0) as u8;
                        let temp_tuple = KValue::TemporaryTuple(RegisterSlice {
                            // The unpacked tuple contents go into the registers after the
                            // function's captures, which are placed after the temp tuple and
                            // instance registers.
                            start: 2 + capture_count,
                            count: args.len() as u8,
                        });
                        self.registers.push(temp_tuple);
                        (1, Some(args))
                    }
                    _ => {
                        let tuple_contents = Vec::from(args);
                        self.registers.push(KValue::Tuple(tuple_contents.into()));
                        (1, None)
                    }
                }
            }
        };

        let old_frame_count = self.call_stack.len();

        self.call_callable(
            CallInfo {
                result_register: Some(result_register),
                frame_base,
                // The instance (or Null) has already been copied into the frame base
                instance: Some(frame_base),
                arg_count,
                packed_arg_count: 0,
            },
            function,
            temp_tuple_values,
        )?;

        let result = if self.call_stack.len() == old_frame_count {
            // If the call stack is the same size as before calling call_callable,
            // then an external function was called and the result should be in the result register.
            let result = self.clone_register(result_register);
            Ok(result)
        } else {
            // Otherwise, execute instructions until this frame is exited
            self.frame_mut().execution_barrier = true;
            let result = self.execute_instructions();
            if result.is_err() {
                self.pop_frame(KValue::Null)?;
            }
            result
        };

        self.truncate_registers(result_register);

        result
    }

    /// Returns a displayable string for the given value
    pub fn value_to_string(&mut self, value: &KValue) -> Result<String> {
        let mut display_context = DisplayContext::with_vm(self);
        value.display(&mut display_context)?;
        Ok(display_context.result())
    }

    /// Provides the result of running a unary operation on a KValue
    pub fn run_unary_op(&mut self, op: UnaryOp, value: KValue) -> Result<KValue> {
        use UnaryOp::*;

        let old_frame_count = self.call_stack.len();
        let result_register = self.next_register();
        let value_register = result_register + 1;

        self.registers.push(KValue::Null); // `result_register`
        self.registers.push(value); // `value_register`

        match op {
            Debug => self.run_debug_op(result_register, value_register)?,
            Display => self.run_display(result_register, value_register)?,
            Negate => self.run_negate(result_register, value_register)?,
            Iterator => self.run_make_iterator(result_register, value_register, false)?,
            Next => self.run_iterator_next(Some(result_register), value_register, 0, false)?,
            NextBack => match self.clone_register(value_register) {
                KValue::Map(m) if m.contains_meta_key(&NextBack.into()) => {
                    let op = m.get_meta_value(&NextBack.into()).unwrap();
                    if !op.is_callable() {
                        return unexpected_type("Callable function from @next_back", &op);
                    }
                    self.call_overridden_unary_op(Some(result_register), value_register, op)?
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

        let result = if self.call_stack.len() == old_frame_count {
            // If the call stack is the same size, then a native function was called and the result
            // will be in the result register
            Ok(self.clone_register(result_register))
        } else {
            // If the call stack size has changed, then an overridden operator in Koto has been
            // called, so continue execution until the call is complete.
            self.frame_mut().execution_barrier = true;
            let result = self.execute_instructions();
            if result.is_err() {
                self.pop_frame(KValue::Null)?;
            }
            result
        };

        self.truncate_registers(result_register);
        result
    }

    /// Provides the result of running a binary operation on a pair of Values
    pub fn run_binary_op(&mut self, op: BinaryOp, lhs: KValue, rhs: KValue) -> Result<KValue> {
        let old_frame_count = self.call_stack.len();
        let result_register = self.next_register();
        let lhs_register = result_register + 1;
        let rhs_register = result_register + 2;

        self.registers.push(KValue::Null); // Result register
        self.registers.push(lhs);
        self.registers.push(rhs);

        match op {
            BinaryOp::Add => self.run_add(result_register, lhs_register, rhs_register)?,
            BinaryOp::Subtract => self.run_subtract(result_register, lhs_register, rhs_register)?,
            BinaryOp::Multiply => self.run_multiply(result_register, lhs_register, rhs_register)?,
            BinaryOp::Divide => self.run_divide(result_register, lhs_register, rhs_register)?,
            BinaryOp::Remainder => {
                self.run_remainder(result_register, lhs_register, rhs_register)?
            }
            BinaryOp::AddAssign => {
                self.run_add_assign(lhs_register, rhs_register)?;
                self.set_register(result_register, self.clone_register(lhs_register));
            }
            BinaryOp::SubtractAssign => {
                self.run_subtract_assign(lhs_register, rhs_register)?;
                self.set_register(result_register, self.clone_register(lhs_register));
            }
            BinaryOp::MultiplyAssign => {
                self.run_multiply_assign(lhs_register, rhs_register)?;
                self.set_register(result_register, self.clone_register(lhs_register));
            }
            BinaryOp::DivideAssign => {
                self.run_divide_assign(lhs_register, rhs_register)?;
                self.set_register(result_register, self.clone_register(lhs_register));
            }
            BinaryOp::RemainderAssign => {
                self.run_remainder_assign(lhs_register, rhs_register)?;
                self.set_register(result_register, self.clone_register(lhs_register));
            }
            BinaryOp::Less => self.run_less(result_register, lhs_register, rhs_register)?,
            BinaryOp::LessOrEqual => {
                self.run_less_or_equal(result_register, lhs_register, rhs_register)?
            }
            BinaryOp::Greater => self.run_greater(result_register, lhs_register, rhs_register)?,
            BinaryOp::GreaterOrEqual => {
                self.run_greater_or_equal(result_register, lhs_register, rhs_register)?
            }
            BinaryOp::Equal => self.run_equal(result_register, lhs_register, rhs_register)?,
            BinaryOp::NotEqual => {
                self.run_not_equal(result_register, lhs_register, rhs_register)?
            }
            BinaryOp::Index => self.run_index(result_register, lhs_register, rhs_register)?,
        }

        let result = if self.call_stack.len() == old_frame_count {
            // If the call stack is the same size, then a native function was called and the result
            // will be in the result register
            Ok(self.clone_register(result_register))
        } else {
            // If the call stack size has changed, then an overridden operator in Koto has been
            // called, so continue execution until the call is complete.
            self.frame_mut().execution_barrier = true;
            let result = self.execute_instructions();
            if result.is_err() {
                self.pop_frame(KValue::Null)?;
            }
            result
        };

        self.truncate_registers(result_register);
        result
    }

    /// Makes a KIterator that iterates over the provided value's contents
    pub fn make_iterator(&mut self, value: KValue) -> Result<KIterator> {
        use KValue::*;

        match value {
            Map(ref m) if m.contains_meta_key(&UnaryOp::Next.into()) => {
                KIterator::with_meta_next(self.spawn_shared_vm(), value)
            }
            Map(ref m) if m.contains_meta_key(&UnaryOp::Iterator.into()) => {
                // If the value implements @iterator,
                // first evaluate @iterator and then make an iterator from the result
                let iterator_call_result = self.run_unary_op(UnaryOp::Iterator, value)?;
                self.make_iterator(iterator_call_result)
            }
            Iterator(i) => Ok(i),
            Range(r) => KIterator::with_range(r),
            List(l) => Ok(KIterator::with_list(l)),
            Tuple(t) => Ok(KIterator::with_tuple(t)),
            Str(s) => Ok(KIterator::with_string(s)),
            Map(m) => Ok(KIterator::with_map(m)),
            Object(o) => {
                use IsIterable::*;

                let o_inner = o.try_borrow()?;
                match o_inner.is_iterable() {
                    NotIterable => runtime_error!("{} is not iterable", o_inner.type_string()),
                    Iterable => o_inner.make_iterator(self),
                    ForwardIterator | BidirectionalIterator => {
                        KIterator::with_object(self.spawn_shared_vm(), o.clone())
                    }
                }
            }
            unexpected => {
                runtime_error!(
                    "expected iterable value, found '{}'",
                    unexpected.type_as_string(),
                )
            }
        }
    }

    /// Runs any function tagged with `@test` in the provided map
    ///
    /// Any test failure will be returned as an error.
    pub fn run_tests(&mut self, test_map: KMap) -> Result<KValue> {
        use KValue::{Map, Null};

        // It's important throughout this function to make sure we don't hang on to any references
        // to the internal test map data while calling the test functions. Otherwise we'll end up in
        // deadlocks when the map needs to be modified (e.g. in pre or post test functions).

        let (pre_test, post_test, meta_entry_count) = match test_map.meta_map() {
            Some(meta) => {
                let meta = meta.borrow();
                (
                    meta.get(&MetaKey::PreTest).cloned(),
                    meta.get(&MetaKey::PostTest).cloned(),
                    meta.len(),
                )
            }
            None => (None, None, 0),
        };

        let self_arg = Map(test_map.clone());

        for i in 0..meta_entry_count {
            let meta_entry = test_map.meta_map().and_then(|meta| {
                meta.borrow()
                    .get_index(i)
                    .map(|(key, value)| (key.clone(), value.clone()))
            });

            let Some((MetaKey::Test(test_name), test)) = meta_entry else {
                continue;
            };

            if !test.is_callable() {
                return unexpected_type(&format!("Callable for '{test_name}'"), &test);
            }

            let make_test_error = |error: Error, message: &str| {
                Err(error.with_prefix(&format!("{message} '{test_name}'")))
            };

            if let Some(pre_test) = &pre_test {
                if pre_test.is_callable() {
                    let pre_test_result =
                        self.call_instance_function(self_arg.clone(), pre_test.clone(), &[]);

                    if let Err(error) = pre_test_result {
                        return make_test_error(error, "Error while preparing to run test");
                    }
                }
            }

            let test_result = self.call_instance_function(self_arg.clone(), test, &[]);

            if let Err(error) = test_result {
                return make_test_error(error, "Error while running test");
            }

            if let Some(post_test) = &post_test {
                if post_test.is_callable() {
                    let post_test_result =
                        self.call_instance_function(self_arg.clone(), post_test.clone(), &[]);

                    if let Err(error) = post_test_result {
                        return make_test_error(error, "Error after running test");
                    }
                }
            }
        }

        Ok(Null)
    }

    fn execute_instructions(&mut self) -> Result<KValue> {
        let mut timeout = self
            .context
            .settings
            .execution_limit
            .map(ExecutionTimeout::new);

        self.instruction_ip = self.ip();

        // Every code path in this function must set the execution state to something other
        // than Active before exiting.
        self.execution_state = ExecutionState::Active;

        while let Some(instruction) = self.reader.next() {
            if let Some(timeout) = timeout.as_mut() {
                if timeout.check_for_timeout() {
                    self.execution_state = ExecutionState::Inactive;
                    return self
                        .pop_call_stack_on_error(
                            ErrorKind::Timeout(timeout.execution_limit).into(),
                            false,
                        )
                        .map(|_| KValue::Null);
                }
            }

            match self.execute_instruction(instruction) {
                Ok(ControlFlow::Continue) => {}
                Ok(ControlFlow::Return(value)) => {
                    self.execution_state = ExecutionState::Inactive;
                    return Ok(value);
                }
                Ok(ControlFlow::Yield(value)) => {
                    self.execution_state = ExecutionState::Suspended;
                    return Ok(value);
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
                    Err(error) => {
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
            ValueExport { name, value } => self.run_value_export(name, value)?,
            Import { register } => self.run_import(register)?,
            MakeTempTuple {
                register,
                start,
                count,
            } => self.set_register(
                register,
                KValue::TemporaryTuple(RegisterSlice { start, count }),
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
            } => self.run_string_push(value, &format_options)?,
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
                self.run_make_iterator(register, iterable, true)?
            }
            Function { .. } => self.run_make_function(instruction),
            Capture {
                function,
                target,
                source,
            } => self.run_capture_value(function, target, source)?,
            Negate { register, value } => self.run_negate(register, value)?,
            Not { register, value } => self.run_not(register, value)?,
            Add { register, lhs, rhs } => self.run_add(register, lhs, rhs)?,
            Subtract { register, lhs, rhs } => self.run_subtract(register, lhs, rhs)?,
            Multiply { register, lhs, rhs } => self.run_multiply(register, lhs, rhs)?,
            Divide { register, lhs, rhs } => self.run_divide(register, lhs, rhs)?,
            Remainder { register, lhs, rhs } => self.run_remainder(register, lhs, rhs)?,
            AddAssign { lhs, rhs } => self.run_add_assign(lhs, rhs)?,
            SubtractAssign { lhs, rhs } => self.run_subtract_assign(lhs, rhs)?,
            MultiplyAssign { lhs, rhs } => self.run_multiply_assign(lhs, rhs)?,
            DivideAssign { lhs, rhs } => self.run_divide_assign(lhs, rhs)?,
            RemainderAssign { lhs, rhs } => self.run_remainder_assign(lhs, rhs)?,
            Less { register, lhs, rhs } => self.run_less(register, lhs, rhs)?,
            LessOrEqual { register, lhs, rhs } => self.run_less_or_equal(register, lhs, rhs)?,
            Greater { register, lhs, rhs } => self.run_greater(register, lhs, rhs)?,
            GreaterOrEqual { register, lhs, rhs } => {
                self.run_greater_or_equal(register, lhs, rhs)?
            }
            Equal { register, lhs, rhs } => self.run_equal(register, lhs, rhs)?,
            NotEqual { register, lhs, rhs } => self.run_not_equal(register, lhs, rhs)?,
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
            } => self.call_callable(
                CallInfo {
                    result_register: Some(result),
                    frame_base,
                    instance: None,
                    arg_count,
                    packed_arg_count: unpacked_arg_count,
                },
                self.clone_register(function),
                None,
            )?,
            CallInstance {
                result,
                function,
                instance,
                frame_base,
                arg_count,
                packed_arg_count: unpacked_arg_count,
            } => self.call_callable(
                CallInfo {
                    result_register: Some(result),
                    frame_base,
                    instance: Some(instance),
                    arg_count,
                    packed_arg_count: unpacked_arg_count,
                },
                self.clone_register(function),
                None,
            )?,
            Return { register } => {
                if let Some(return_value) = self.pop_frame(self.clone_register(register))? {
                    // If pop_frame returns a new return_value, then execution should stop.
                    control_flow = ControlFlow::Return(return_value);
                }
            }
            Yield { register } => control_flow = ControlFlow::Yield(self.clone_register(register)),
            Throw { register } => {
                let thrown_value = self.clone_register(register);

                match &thrown_value {
                    KValue::Str(_) | KValue::Object(_) => {}
                    KValue::Map(m) if m.contains_meta_key(&UnaryOp::Display.into()) => {}
                    other => {
                        return unexpected_type(
                            "a String or a value that implements @display",
                            other,
                        );
                    }
                };

                return Err(crate::Error::from_koto_value(
                    thrown_value,
                    self.spawn_shared_vm(),
                ));
            }
            Size { register, value } => self.run_size(register, value, false)?,
            IterNext {
                result,
                iterator,
                jump_offset,
                temporary_output,
            } => self.run_iterator_next(result, iterator, jump_offset, temporary_output)?,
            TempIndex {
                register,
                value,
                index,
            } => self.run_temp_index(register, value, index)?,
            SliceFrom {
                register,
                value,
                index,
            } => self.run_slice(register, value, index, false)?,
            SliceTo {
                register,
                value,
                index,
            } => self.run_slice(register, value, index, true)?,
            Index {
                register,
                value,
                index,
            } => self.run_index(register, value, index)?,
            IndexMut {
                register,
                index,
                value,
            } => self.run_index_mut(register, index, value)?,
            MapInsert {
                register,
                key,
                value,
            } => self.run_map_insert(register, key, value)?,
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
            } => self.run_access(register, value, self.koto_string_from_constant(key))?,
            AccessString {
                register,
                value,
                key,
            } => {
                let key_string = match self.clone_register(key) {
                    KValue::Str(s) => s,
                    other => return unexpected_type("a String", &other),
                };
                self.run_access(register, value, key_string)?;
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
            Debug { register, constant } => self.run_debug_instruction(register, constant)?,
            CheckSizeEqual { register, size } => self.run_check_size_equal(register, size)?,
            CheckSizeMin { register, size } => self.run_check_size_min(register, size)?,
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
            .exports
            .get(name)
            .or_else(|| self.context.prelude.get(name));

        if let Some(non_local) = non_local {
            self.set_register(register, non_local);
            Ok(())
        } else {
            runtime_error!("'{name}' not found")
        }
    }

    fn run_value_export(&mut self, name_register: u8, value_register: u8) -> Result<()> {
        let name = ValueKey::try_from(self.clone_register(name_register))?;
        let value = self.clone_register(value_register);
        self.exports.data_mut().insert(name, value);
        Ok(())
    }

    fn run_temp_tuple_to_tuple(&mut self, register: u8, source_register: u8) -> Result<()> {
        match self.clone_register(source_register) {
            KValue::TemporaryTuple(temp_registers) => {
                let tuple =
                    KTuple::from(self.register_slice(temp_registers.start, temp_registers.count));
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
                    return self.call_overridden_unary_op(
                        Some(result_register),
                        iterable_register,
                        op,
                    );
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
    ) -> Result<()> {
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
                            Some(self.clone_register(start)),
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
            output
        } else {
            match self.clone_register(iterable_register) {
                Iterator(mut iterator) => {
                    match iterator.next() {
                        Some(KIteratorOutput::Value(value)) => Some(value),
                        Some(KIteratorOutput::ValuePair(first, second)) => {
                            if let Some(result) = result_register {
                                if output_is_temporary {
                                    self.set_register(result + 1, first);
                                    self.set_register(result + 2, second);
                                    Some(TemporaryTuple(RegisterSlice {
                                        start: result + 1,
                                        count: 2,
                                    }))
                                } else {
                                    Some(Tuple(vec![first, second].into()))
                                }
                            } else {
                                // The output is going to be ignored, but we use Some here to
                                // indicate that iteration should continue.
                                Some(Null)
                            }
                        }
                        Some(KIteratorOutput::Error(error)) => {
                            return runtime_error!(error.to_string());
                        }
                        None => None,
                    }
                }
                Map(m) if m.contains_meta_key(&UnaryOp::Next.into()) => {
                    let op = m.get_meta_value(&UnaryOp::Next.into()).unwrap();
                    if !op.is_callable() {
                        return unexpected_type("Callable function from @next", &op);
                    }
                    // The return value will be retrieved from execute_instructions
                    self.call_overridden_unary_op(None, iterable_register, op)?;
                    self.frame_mut().execution_barrier = true;
                    match self.execute_instructions() {
                        Ok(Null) => None,
                        Ok(output) => Some(output),
                        Err(error) => {
                            self.pop_frame(KValue::Null)?;
                            return Err(error);
                        }
                    }
                }
                unexpected => return unexpected_type("Iterator", &unexpected),
            }
        };

        match (output, result_register) {
            (Some(output), Some(register)) => {
                self.set_register(register, output);
            }
            (Some(_), None) => {
                // No result register, so the output can be discarded
            }
            (None, Some(register)) => {
                // The iterator is finished, so jump to the provided offset
                self.set_register(register, Null);
                self.jump_ip(jump_offset as u32);
            }
            (None, None) => {
                self.jump_ip(jump_offset as u32);
            }
        }

        Ok(())
    }

    fn run_temp_index(&mut self, result: u8, value: u8, index: i8) -> Result<()> {
        use KValue::*;

        let index_op = BinaryOp::Index.into();

        let result_value = match self.get_register(value) {
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
                if index.unsigned_abs() < count {
                    let index = signed_index_to_unsigned(index, count as usize);
                    self.clone_register(start + index as u8)
                } else {
                    Null
                }
            }
            Str(s) => {
                let index = signed_index_to_unsigned(index, s.len());
                s.with_bounds(index..index + 1).into()
            }
            Map(map) if map.contains_meta_key(&index_op) => {
                let op = map.get_meta_value(&index_op).unwrap();
                return self.call_overridden_binary_op(Some(result), value, index.into(), op);
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

    fn run_slice(&mut self, register: u8, value: u8, index: i8, is_slice_to: bool) -> Result<()> {
        use KValue::*;

        let index_op = BinaryOp::Index.into();

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
                let size = self.get_value_size(value)?;
                let index = signed_index_to_unsigned(index, size) as i64;
                let range = if is_slice_to {
                    0..index
                } else {
                    index..size as i64
                };
                self.run_binary_op(BinaryOp::Index, Map(m), KRange::from(range).into())?
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

        Ok(())
    }

    fn run_make_function(&mut self, function_instruction: Instruction) {
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

                let function = KFunction::new(
                    self.chunk(),
                    self.ip(),
                    arg_count,
                    optional_arg_count,
                    flags,
                    captures,
                );

                self.jump_ip(size as u32);
                self.set_register(register, KValue::Function(function));
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
                if let Some(captures) = &f.captures {
                    captures.data_mut()[capture_index as usize] = self.clone_register(value);
                }
                Ok(())
            }
            unexpected => unexpected_type("Function while capturing value", unexpected),
        }
    }

    fn run_negate(&mut self, result: u8, value: u8) -> Result<()> {
        use KValue::*;
        use UnaryOp::Negate;

        let result_value = match self.clone_register(value) {
            Number(n) => Number(-n),
            Map(m) if m.contains_meta_key(&Negate.into()) => {
                let op = m.get_meta_value(&Negate.into()).unwrap();
                return self.call_overridden_unary_op(Some(result), value, op);
            }
            Object(o) => o.try_borrow()?.negate(self)?,
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
                self.call_overridden_unary_op(Some(result), value, op)
            }
            other => {
                let mut display_context = DisplayContext::with_vm(self).enable_debug();
                match other.display(&mut display_context) {
                    Ok(_) => {
                        self.set_register(result, display_context.result().into());
                        Ok(())
                    }
                    Err(_) => runtime_error!("failed to get display value"),
                }
            }
        }
    }

    fn run_display(&mut self, result: u8, value: u8) -> Result<()> {
        use UnaryOp::Display;

        match self.clone_register(value) {
            KValue::Map(m) if m.contains_meta_key(&Display.into()) => {
                let op = m.get_meta_value(&Display.into()).unwrap();
                self.call_overridden_unary_op(Some(result), value, op)
            }
            other => {
                let mut display_context = DisplayContext::with_vm(self);
                match other.display(&mut display_context) {
                    Ok(_) => {
                        self.set_register(result, display_context.result().into());
                        Ok(())
                    }
                    Err(_) => runtime_error!("failed to get display value"),
                }
            }
        }
    }

    fn run_add(&mut self, result: u8, lhs: u8, rhs: u8) -> Result<()> {
        use BinaryOp::Add;
        use KValue::*;

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
                let op = m.get_meta_value(&Add.into()).unwrap();
                let rhs_value = rhs_value.clone();
                return self.call_overridden_binary_op(Some(result), lhs, rhs_value, op);
            }
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
            (Object(o), _) => o.try_borrow()?.add(rhs_value)?,
            _ => return binary_op_error(lhs_value, rhs_value, Add),
        };

        self.set_register(result, result_value);
        Ok(())
    }

    fn run_subtract(&mut self, result: u8, lhs: u8, rhs: u8) -> Result<()> {
        use BinaryOp::Subtract;
        use KValue::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result_value = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Number(a - b),
            (Map(m), _) if m.contains_meta_key(&Subtract.into()) => {
                let op = m.get_meta_value(&Subtract.into()).unwrap();
                let rhs_value = rhs_value.clone();
                return self.call_overridden_binary_op(Some(result), lhs, rhs_value, op);
            }
            (Object(o), _) => o.try_borrow()?.subtract(rhs_value)?,
            _ => return binary_op_error(lhs_value, rhs_value, Subtract),
        };

        self.set_register(result, result_value);
        Ok(())
    }

    fn run_multiply(&mut self, result: u8, lhs: u8, rhs: u8) -> Result<()> {
        use BinaryOp::Multiply;
        use KValue::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);

        let result_value = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Number(a * b),
            (Map(m), _) if m.contains_meta_key(&Multiply.into()) => {
                let op = m.get_meta_value(&Multiply.into()).unwrap();
                let rhs_value = rhs_value.clone();
                return self.call_overridden_binary_op(Some(result), lhs, rhs_value, op);
            }
            (Object(o), _) => o.try_borrow()?.multiply(rhs_value)?,
            _ => return binary_op_error(lhs_value, rhs_value, Multiply),
        };

        self.set_register(result, result_value);
        Ok(())
    }

    fn run_divide(&mut self, result: u8, lhs: u8, rhs: u8) -> Result<()> {
        use BinaryOp::Divide;
        use KValue::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result_value = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Number(a / b),
            (Map(m), _) if m.contains_meta_key(&Divide.into()) => {
                let op = m.get_meta_value(&Divide.into()).unwrap();
                let rhs_value = rhs_value.clone();
                return self.call_overridden_binary_op(Some(result), lhs, rhs_value, op);
            }
            (Object(o), _) => o.try_borrow()?.divide(rhs_value)?,
            _ => return binary_op_error(lhs_value, rhs_value, Divide),
        };

        self.set_register(result, result_value);
        Ok(())
    }

    fn run_remainder(&mut self, result: u8, lhs: u8, rhs: u8) -> Result<()> {
        use BinaryOp::Remainder;
        use KValue::*;

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
                let op = m.get_meta_value(&Remainder.into()).unwrap();
                let rhs_value = rhs_value.clone();
                return self.call_overridden_binary_op(Some(result), lhs, rhs_value, op);
            }
            (Object(o), _) => o.try_borrow()?.remainder(rhs_value)?,
            _ => return binary_op_error(lhs_value, rhs_value, Remainder),
        };
        self.set_register(result, result_value);

        Ok(())
    }

    fn run_add_assign(&mut self, lhs: u8, rhs: u8) -> Result<()> {
        use BinaryOp::AddAssign;
        use KValue::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => {
                self.set_register(lhs, Number(a + b));
                Ok(())
            }
            (Map(m), _) if m.contains_meta_key(&AddAssign.into()) => {
                let op = m.get_meta_value(&AddAssign.into()).unwrap();
                let rhs_value = rhs_value.clone();
                // The call result can be discarded, the result is always the modified LHS
                self.call_overridden_binary_op(None, lhs, rhs_value, op)
            }
            (Object(o), Object(o2)) if o2.is_same_instance(o2) => {
                let o2 = Object(o2.try_borrow()?.copy());
                o.try_borrow_mut()?.add_assign(&o2)
            }
            (Object(o), _) => o.try_borrow_mut()?.add_assign(rhs_value),
            _ => binary_op_error(lhs_value, rhs_value, AddAssign),
        }
    }

    fn run_subtract_assign(&mut self, lhs: u8, rhs: u8) -> Result<()> {
        use BinaryOp::SubtractAssign;
        use KValue::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => {
                self.set_register(lhs, Number(a - b));
                Ok(())
            }
            (Map(m), _) if m.contains_meta_key(&SubtractAssign.into()) => {
                let op = m.get_meta_value(&SubtractAssign.into()).unwrap();
                let rhs_value = rhs_value.clone();
                // The call result can be discarded, the result is always the modified LHS
                self.call_overridden_binary_op(None, lhs, rhs_value, op)
            }
            (Object(o), Object(o2)) if o2.is_same_instance(o2) => {
                let o2 = Object(o2.try_borrow()?.copy());
                o.try_borrow_mut()?.subtract_assign(&o2)
            }
            (Object(o), _) => o.try_borrow_mut()?.subtract_assign(rhs_value),
            _ => binary_op_error(lhs_value, rhs_value, SubtractAssign),
        }
    }

    fn run_multiply_assign(&mut self, lhs: u8, rhs: u8) -> Result<()> {
        use BinaryOp::MultiplyAssign;
        use KValue::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => {
                self.set_register(lhs, Number(a * b));
                Ok(())
            }
            (Map(m), _) if m.contains_meta_key(&MultiplyAssign.into()) => {
                let op = m.get_meta_value(&MultiplyAssign.into()).unwrap();
                let rhs_value = rhs_value.clone();
                // The call result can be discarded, the result is always the modified LHS
                self.call_overridden_binary_op(None, lhs, rhs_value, op)
            }
            (Object(o), Object(o2)) if o2.is_same_instance(o2) => {
                let o2 = Object(o2.try_borrow()?.copy());
                o.try_borrow_mut()?.multiply_assign(&o2)
            }
            (Object(o), _) => o.try_borrow_mut()?.multiply_assign(rhs_value),
            _ => binary_op_error(lhs_value, rhs_value, MultiplyAssign),
        }
    }

    fn run_divide_assign(&mut self, lhs: u8, rhs: u8) -> Result<()> {
        use BinaryOp::DivideAssign;
        use KValue::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => {
                self.set_register(lhs, Number(a / b));
                Ok(())
            }
            (Map(m), _) if m.contains_meta_key(&DivideAssign.into()) => {
                let op = m.get_meta_value(&DivideAssign.into()).unwrap();
                let rhs_value = rhs_value.clone();
                // The call result can be discarded, the result is always the modified LHS
                self.call_overridden_binary_op(None, lhs, rhs_value, op)
            }
            (Object(o), Object(o2)) if o2.is_same_instance(o2) => {
                let o2 = Object(o2.try_borrow()?.copy());
                o.try_borrow_mut()?.divide_assign(&o2)
            }
            (Object(o), _) => o.try_borrow_mut()?.divide_assign(rhs_value),
            _ => binary_op_error(lhs_value, rhs_value, DivideAssign),
        }
    }

    fn run_remainder_assign(&mut self, lhs: u8, rhs: u8) -> Result<()> {
        use BinaryOp::RemainderAssign;
        use KValue::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => {
                self.set_register(lhs, Number(a % b));
                Ok(())
            }
            (Map(m), _) if m.contains_meta_key(&RemainderAssign.into()) => {
                let op = m.get_meta_value(&RemainderAssign.into()).unwrap();
                let rhs_value = rhs_value.clone();
                // The call result can be discarded, the result is always the modified LHS
                self.call_overridden_binary_op(None, lhs, rhs_value, op)
            }
            (Object(o), Object(o2)) if o2.is_same_instance(o2) => {
                let o2 = Object(o2.try_borrow()?.copy());
                o.try_borrow_mut()?.remainder_assign(&o2)
            }
            (Object(o), _) => o.try_borrow_mut()?.remainder_assign(rhs_value),
            _ => binary_op_error(lhs_value, rhs_value, RemainderAssign),
        }
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
                let op = m.get_meta_value(&Less.into()).unwrap();
                let rhs_value = rhs_value.clone();
                return self.call_overridden_binary_op(Some(result), lhs, rhs_value, op);
            }
            (Object(o), _) => o.try_borrow()?.less(rhs_value)?.into(),
            _ => return binary_op_error(lhs_value, rhs_value, Less),
        };
        self.set_register(result, result_value);

        Ok(())
    }

    fn run_less_or_equal(&mut self, result: u8, lhs: u8, rhs: u8) -> Result<()> {
        use BinaryOp::LessOrEqual;
        use KValue::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result_value = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Bool(a <= b),
            (Str(a), Str(b)) => Bool(a.as_str() <= b.as_str()),
            (Map(m), _) if m.contains_meta_key(&LessOrEqual.into()) => {
                let op = m.get_meta_value(&LessOrEqual.into()).unwrap();
                let rhs_value = rhs_value.clone();
                return self.call_overridden_binary_op(Some(result), lhs, rhs_value, op);
            }
            (Object(o), _) => o.try_borrow()?.less_or_equal(rhs_value)?.into(),
            _ => return binary_op_error(lhs_value, rhs_value, LessOrEqual),
        };
        self.set_register(result, result_value);

        Ok(())
    }

    fn run_greater(&mut self, result: u8, lhs: u8, rhs: u8) -> Result<()> {
        use BinaryOp::Greater;
        use KValue::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result_value = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Bool(a > b),
            (Str(a), Str(b)) => Bool(a.as_str() > b.as_str()),
            (Map(m), _) if m.contains_meta_key(&Greater.into()) => {
                let op = m.get_meta_value(&Greater.into()).unwrap();
                let rhs_value = rhs_value.clone();
                return self.call_overridden_binary_op(Some(result), lhs, rhs_value, op);
            }
            (Object(o), _) => o.try_borrow()?.greater(rhs_value)?.into(),
            _ => return binary_op_error(lhs_value, rhs_value, Greater),
        };
        self.set_register(result, result_value);

        Ok(())
    }

    fn run_greater_or_equal(&mut self, result: u8, lhs: u8, rhs: u8) -> Result<()> {
        use BinaryOp::GreaterOrEqual;
        use KValue::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result_value = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Bool(a >= b),
            (Str(a), Str(b)) => Bool(a.as_str() >= b.as_str()),
            (Map(m), _) if m.contains_meta_key(&GreaterOrEqual.into()) => {
                let op = m.get_meta_value(&GreaterOrEqual.into()).unwrap();
                let rhs_value = rhs_value.clone();
                return self.call_overridden_binary_op(Some(result), lhs, rhs_value, op);
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
                self.compare_value_ranges(&data_a, &data_b)?
            }
            (Tuple(a), Tuple(b)) => {
                let a = a.clone();
                let b = b.clone();
                self.compare_value_ranges(&a, &b)?
            }
            (Map(m), _) if m.contains_meta_key(&Equal.into()) => {
                let op = m.get_meta_value(&Equal.into()).unwrap();
                let rhs_value = rhs_value.clone();
                return self.call_overridden_binary_op(Some(result), lhs, rhs_value, op);
            }
            (Map(map), _) => {
                if let Map(rhs_map) = rhs_value {
                    let a = map.clone();
                    let b = rhs_map.clone();
                    self.compare_value_maps(a, b)?
                } else {
                    false
                }
            }
            (Object(o), _) => o.try_borrow()?.equal(rhs_value)?,
            (Function(a), Function(b)) => {
                let a = a.clone();
                let b = b.clone();
                self.compare_functions(a, b)?
            }
            _ => false,
        };

        self.set_register(result, result_value.into());

        Ok(())
    }

    fn run_not_equal(&mut self, result: u8, lhs: u8, rhs: u8) -> Result<()> {
        use BinaryOp::NotEqual;
        use KValue::*;

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
                !self.compare_value_ranges(&data_a, &data_b)?
            }
            (Tuple(a), Tuple(b)) => {
                let a = a.clone();
                let b = b.clone();
                !self.compare_value_ranges(&a, &b)?
            }
            (Map(m), _) if m.contains_meta_key(&NotEqual.into()) => {
                let op = m.get_meta_value(&NotEqual.into()).unwrap();
                let rhs_value = rhs_value.clone();
                return self.call_overridden_binary_op(Some(result), lhs, rhs_value, op);
            }
            (Map(map), _) => {
                if let Map(rhs_map) = rhs_value {
                    let a = map.clone();
                    let b = rhs_map.clone();
                    !self.compare_value_maps(a, b)?
                } else {
                    true
                }
            }
            (Object(o), _) => o.try_borrow()?.not_equal(rhs_value)?,
            (Function(a), Function(b)) => {
                let a = a.clone();
                let b = b.clone();
                !self.compare_functions(a, b)?
            }
            _ => true,
        };
        self.set_register(result, result_value.into());

        Ok(())
    }

    fn compare_functions(&mut self, a: KFunction, b: KFunction) -> Result<bool> {
        if a.chunk == b.chunk && a.ip == b.ip {
            match (&a.captures, &b.captures) {
                (None, None) => Ok(true),
                (Some(captures_a), Some(captures_b)) => {
                    let captures_a = captures_a.clone();
                    let captures_b = captures_b.clone();
                    let data_a = captures_a.data();
                    let data_b = captures_b.data();
                    self.compare_value_ranges(&data_a, &data_b)
                }
                _ => Ok(false),
            }
        } else {
            Ok(false)
        }
    }

    // Called from run_equal / run_not_equal to compare the contents of lists and tuples
    fn compare_value_ranges(&mut self, range_a: &[KValue], range_b: &[KValue]) -> Result<bool> {
        if range_a.len() != range_b.len() {
            return Ok(false);
        }

        for (value_a, value_b) in range_a.iter().zip(range_b.iter()) {
            match self.run_binary_op(BinaryOp::Equal, value_a.clone(), value_b.clone())? {
                KValue::Bool(true) => {}
                KValue::Bool(false) => return Ok(false),
                other => {
                    return runtime_error!(
                        "Expected Bool from equality comparison, found '{}'",
                        other.type_as_string()
                    );
                }
            }
        }

        Ok(true)
    }

    // Called from run_equal / run_not_equal to compare the contents of maps
    fn compare_value_maps(&mut self, map_a: KMap, map_b: KMap) -> Result<bool> {
        if map_a.len() != map_b.len() {
            return Ok(false);
        }

        for (key_a, value_a) in map_a.data().iter() {
            let Some(value_b) = map_b.get(key_a) else {
                return Ok(false);
            };
            match self.run_binary_op(BinaryOp::Equal, value_a.clone(), value_b)? {
                KValue::Bool(true) => {}
                KValue::Bool(false) => return Ok(false),
                other => {
                    return runtime_error!(
                        "Expected Bool from equality comparison, found '{}'",
                        other.type_as_string()
                    );
                }
            }
        }

        Ok(true)
    }

    fn call_overridden_unary_op(
        &mut self,
        result_register: Option<u8>,
        value_register: u8,
        op: KValue,
    ) -> Result<()> {
        // Set up the call registers at the end of the stack
        let frame_base = self.new_frame_base()?;
        self.registers.push(self.clone_register(value_register)); // Frame base
        self.call_callable(
            CallInfo {
                result_register,
                frame_base,
                instance: Some(frame_base),
                arg_count: 0,
                packed_arg_count: 0,
            },
            op,
            None,
        )
    }

    fn call_overridden_binary_op(
        &mut self,
        result_register: Option<u8>,
        lhs_register: u8,
        rhs: KValue,
        op: KValue,
    ) -> Result<()> {
        // Set up the call registers at the end of the stack
        let frame_base = self.new_frame_base()?;

        self.registers.push(self.clone_register(lhs_register)); // Frame base
        self.registers.push(rhs); // The rhs goes in the first arg register
        self.call_callable(
            CallInfo {
                result_register,
                frame_base,
                instance: Some(frame_base),
                arg_count: 1, // 1 arg, the rhs value
                packed_arg_count: 0,
            },
            op,
            None,
        )
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
                return self.call_overridden_unary_op(Some(result_register), value_register, op);
            }
            Map(m) => Some(m.len()),
            Object(o) => o.try_borrow()?.size(),
            TemporaryTuple(RegisterSlice { count, .. }) => Some(*count as usize),
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

    fn run_import(&mut self, import_register: u8) -> Result<()> {
        let import_name = match self.clone_register(import_register) {
            KValue::Str(s) => s,
            value @ KValue::Map(_) => {
                self.set_register(import_register, value);
                return Ok(());
            }
            other => return unexpected_type("import id or string, or accessible value", &other),
        };

        // Is the import in the exports?
        let maybe_in_exports = self.exports.get(&import_name);
        if let Some(value) = maybe_in_exports {
            self.set_register(import_register, value);
            return Ok(());
        }

        // Is the import in the prelude?
        let maybe_in_prelude = self.context.prelude.get(&import_name);
        if let Some(value) = maybe_in_prelude {
            self.set_register(import_register, value);
            return Ok(());
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
            .imported_modules
            .borrow()
            .get(&compile_result.path)
            .cloned();
        match maybe_in_cache {
            Some(None) => {
                // If the cache contains a None placeholder entry for the module path,
                // then we're in a recursive import (see below).
                return runtime_error!("recursive import of module '{import_name}'");
            }
            Some(Some(cached_exports)) if compile_result.loaded_from_cache => {
                self.set_register(import_register, KValue::Map(cached_exports));
                return Ok(());
            }
            _ => {}
        }

        // The module needs to be loaded, which involves the following steps.
        //   - Execute the module's script.
        //   - If the module contains @tests, run them.
        //   - If the module contains a @main function, run it.
        //   - If the steps above are successful, then cache the resulting exports map.

        // Insert a placeholder for the new module, preventing recursive imports
        self.context
            .imported_modules
            .borrow_mut()
            .insert(compile_result.path.clone(), None);

        // Cache the current exports map and prepare an empty exports map for the module
        // that's being imported.
        let importer_exports = self.exports.clone();
        self.exports = KMap::default();

        // Execute the following steps in a closure to ensure that cleanup is performed afterwards
        let import_result = {
            || {
                self.run(compile_result.chunk.clone())?;

                if self.context.settings.run_import_tests {
                    self.run_tests(self.exports.clone())?;
                }

                let maybe_main = self.exports.get_meta_value(&MetaKey::Main);
                match maybe_main {
                    Some(main) if main.is_callable() => {
                        self.call_function(main, &[])?;
                    }
                    Some(unexpected) => return unexpected_type("callable function", &unexpected),
                    None => {}
                }

                Ok(())
            }
        }();

        if import_result.is_ok() {
            if let Some(callback) = &self.context.settings.module_imported_callback {
                callback(&compile_result.path);
            }

            // Cache the module's resulting exports and assign them to the import register
            let module_exports = self.exports.clone();
            self.context
                .imported_modules
                .borrow_mut()
                .insert(compile_result.path, Some(module_exports.clone()));
            self.set_register(import_register, KValue::Map(module_exports));
        } else {
            // If there was an error while importing the module then make sure that the
            // placeholder is removed from the imported modules cache.
            self.context
                .imported_modules
                .borrow_mut()
                .remove(&compile_result.path);
        }

        // Replace the VM's active exports map
        self.exports = importer_exports;
        import_result
    }

    fn run_index_mut(
        &mut self,
        indexable_register: u8,
        index_register: u8,
        value_register: u8,
    ) -> Result<()> {
        use KValue::*;

        let indexable = self.clone_register(indexable_register);
        let index_value = self.get_register(index_register);
        let value = self.get_register(value_register);

        match indexable {
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
                Ok(())
            }
            Map(map) if map.contains_meta_key(&MetaKey::IndexMut) => {
                let index_mut_fn = map.get_meta_value(&MetaKey::IndexMut).unwrap();
                let index_value = index_value.clone();
                let value = value.clone();

                // Set up the function call.
                let frame_base = self.new_frame_base()?;
                // The result of a mutable index assignment is always the RHS, so the
                // function result can be placed in the frame base where it will be
                // immediately discarded.
                let result_register = None;
                self.registers.push(map.into()); // Frame base; the map is `self` for `@index_mut`.
                self.registers.push(index_value);
                self.registers.push(value);
                self.call_callable(
                    CallInfo {
                        result_register,
                        frame_base,
                        instance: Some(frame_base),
                        arg_count: 2,
                        packed_arg_count: 0,
                    },
                    index_mut_fn,
                    None,
                )?;
                Ok(())
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
                                Ok(())
                            }
                            unexpected => unexpected_type("Tuple with 2 elements", unexpected),
                        }
                    } else {
                        runtime_error!("invalid index ({index})")
                    }
                }
                unexpected => unexpected_type("Number", unexpected),
            },
            Object(o) => o.try_borrow_mut()?.index_mut(index_value, value),
            unexpected => unexpected_type("a mutable indexable value", &unexpected),
        }
    }

    fn validate_index(&self, n: KNumber, size: Option<usize>) -> Result<usize> {
        let index = usize::from(n);

        if n < 0.0 {
            return runtime_error!("negative indices aren't allowed ('{n}')");
        } else if let Some(size) = size {
            if index >= size {
                return runtime_error!("index out of bounds - index: {n}, size: {size}");
            }
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
            (Map(m), index) if m.contains_meta_key(&BinaryOp::Index.into()) => {
                let op = m.get_meta_value(&BinaryOp::Index.into()).unwrap();
                return self.call_overridden_binary_op(
                    Some(result_register),
                    value_register,
                    index,
                    op,
                );
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

    fn run_map_insert(
        &mut self,
        map_register: u8,
        key_register: u8,
        value_register: u8,
    ) -> Result<()> {
        let key = ValueKey::try_from(self.clone_register(key_register))?;
        let value = self.clone_register(value_register);

        match self.get_register(map_register) {
            KValue::Map(map) => {
                map.data_mut().insert(key, value);
                Ok(())
            }
            KValue::Object(o) => {
                let o = o.try_borrow()?;
                if let Some(entries) = o.entries() {
                    entries.insert(key, value);
                    Ok(())
                } else {
                    runtime_error!("insertion not supported for '{}'", o.type_string())
                }
            }
            unexpected => unexpected_type("a value that supports insertion", unexpected),
        }
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
        use KValue::*;

        let accessed_value = self.clone_register(value_register);
        let key = ValueKey::from(key_string.clone());

        macro_rules! core_op {
            ($module:ident, $iterator_fallback:expr) => {{
                let op = self.get_core_op(
                    &key,
                    &self.context.core_lib.$module,
                    $iterator_fallback,
                    stringify!($module),
                )?;
                self.set_register(result_register, op);
            }};
        }

        match &accessed_value {
            List(_) => core_op!(list, true),
            Number(_) => core_op!(number, false),
            Range(_) => core_op!(range, true),
            Str(_) => core_op!(string, true),
            Tuple(_) => core_op!(tuple, true),
            Iterator(_) => core_op!(iterator, false),
            Map(map) => {
                let mut access_map = map.clone();
                let mut access_result = None;
                while access_result.is_none() {
                    let maybe_value = access_map.get(&key);
                    match maybe_value {
                        Some(value) => access_result = Some(value),
                        // Fallback to the map module when there's no metamap
                        None if access_map.meta_map().is_none() => {
                            core_op!(map, true);
                            return Ok(());
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
                    access_result = Some(self.get_core_op(
                        &key,
                        &self.context.core_lib.iterator,
                        false,
                        &accessed_value.type_as_string(),
                    )?);
                }

                let Some(value) = access_result else {
                    return runtime_error!(
                        "'{key}' not found in '{}'",
                        accessed_value.type_as_string()
                    );
                };

                self.set_register(result_register, value);
            }
            Object(o) => {
                let o = o.try_borrow()?;

                let mut result = None;
                if let Some(entries) = o.entries() {
                    result = entries.get(&key);
                }

                // Iterator fallback?
                if result.is_none() && !matches!(o.is_iterable(), IsIterable::NotIterable) {
                    result = Some(self.get_core_op(
                        &key,
                        &self.context.core_lib.iterator,
                        false,
                        &o.type_string(),
                    )?);
                }

                if let Some(result) = result {
                    self.set_register(result_register, result);
                } else {
                    return runtime_error!("'{key}' not found in '{}'", o.type_string());
                }
            }
            unexpected => return unexpected_type("Value that supports '.' access", unexpected),
        }

        Ok(())
    }

    fn get_core_op(
        &self,
        key: &ValueKey,
        module: &KMap,
        iterator_fallback: bool,
        module_name: &str,
    ) -> Result<KValue> {
        let maybe_op = match module.get(key) {
            None if iterator_fallback => self.context.core_lib.iterator.get(key),
            maybe_op => maybe_op,
        };

        if let Some(result) = maybe_op {
            Ok(result)
        } else {
            runtime_error!("'{key}' not found in '{module_name}'")
        }
    }

    fn call_native_function(
        &mut self,
        call_info: &CallInfo,
        callable: ExternalCallable,
    ) -> Result<()> {
        let mut call_context = CallContext::new(self, call_info.frame_base, call_info.arg_count);

        let result = match callable {
            ExternalCallable::Function(f) => (f.function)(&mut call_context),
            ExternalCallable::Object(o) => o.try_borrow_mut()?.call(&mut call_context),
        }?;

        if let Some(result_register) = call_info.result_register {
            self.set_register(result_register, result);
        }

        if !self.call_stack.is_empty() {
            // External function calls don't use the push/pop frame mechanism,
            // so drop the call args here now that the call has been completed,
            self.truncate_registers(call_info.frame_base);
            // Ensure that the calling frame still has the required number of registers
            let min_frame_registers = self.register_index(self.frame().required_registers);
            if self.registers.len() < min_frame_registers {
                self.registers.resize(min_frame_registers, KValue::Null);
            }
        }
        Ok(())
    }

    // Similar to `call_koto_function`, but sets up the frame in a new VM for the generator
    fn call_generator(
        &mut self,
        call_info: &CallInfo,
        f: &KFunction,
        temp_tuple_values: Option<&[KValue]>,
    ) -> Result<()> {
        // Spawn a VM for the generator
        let mut generator_vm = self.spawn_shared_vm();
        // Push a frame for running the generator function
        generator_vm.push_frame(
            f.chunk.clone(),
            f.ip,
            0, // Arguments will be copied starting in register 0
            None,
        );
        // Set the generator VM's state as suspended
        generator_vm.execution_state = ExecutionState::Suspended;

        // Place the instance in the first register of the generator vm
        let instance = self
            .get_register_safe(call_info.frame_base)
            .cloned()
            .unwrap_or(KValue::Null);
        generator_vm.registers.push(instance);

        let call_arg_base = call_info.frame_base + 1;
        let expected_arg_count = f.expected_arg_count();

        // Copy any regular (non-variadic) arguments into the generator vm
        generator_vm.registers.extend(
            self.register_slice(call_arg_base, expected_arg_count.min(call_info.arg_count))
                .iter()
                .cloned(),
        );

        // Fill in any missing arguments with default values
        apply_optional_arguments(
            &mut generator_vm.registers,
            f,
            call_info.arg_count,
            expected_arg_count,
        )?;

        // Copy any extra arguments into the generator vm,
        // they'll get extracted into a tuple in apply_variadic_arguments
        generator_vm.registers.extend(
            self.register_slice(
                call_arg_base + expected_arg_count,
                call_info.arg_count.saturating_sub(expected_arg_count),
            )
            .iter()
            .cloned(),
        );

        // Move variadic arguments into a tuple
        apply_variadic_arguments(
            &mut generator_vm.registers,
            1, // The first argument goes into register 1 in the generator vm
            call_info,
            f,
            expected_arg_count,
        )?;

        // Captures and temp tuple values are placed in the registerst following the arguments
        apply_captures_and_temp_tuple_values(&mut generator_vm.registers, f, temp_tuple_values);

        // Move the generator vm into an iterator and then place it in the result register
        if let Some(result_register) = call_info.result_register {
            self.set_register(result_register, KIterator::with_vm(generator_vm).into());
        }

        Ok(())
    }

    fn call_koto_function(
        &mut self,
        call_info: &CallInfo,
        f: &KFunction,
        temp_tuple_values: Option<&[KValue]>,
    ) -> Result<()> {
        debug_assert!(!f.flags.is_generator());

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
        apply_captures_and_temp_tuple_values(&mut self.registers, f, temp_tuple_values);

        // Set up a new frame for the called function
        self.push_frame(
            f.chunk.clone(),
            f.ip,
            call_info.frame_base,
            call_info.result_register,
        );

        Ok(())
    }

    fn call_callable(
        &mut self,
        mut info: CallInfo,
        callable: KValue,
        temp_tuple_values: Option<&[KValue]>,
    ) -> Result<()> {
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

        self.unpack_packed_arguments(&mut info)?;

        match callable {
            Function(f) => {
                if f.flags.is_generator() {
                    self.call_generator(&info, &f, temp_tuple_values)
                } else {
                    self.call_koto_function(&info, &f, temp_tuple_values)
                }
            }
            NativeFunction(f) => self.call_native_function(&info, ExternalCallable::Function(f)),
            Object(o) => self.call_native_function(&info, ExternalCallable::Object(o)),
            Map(ref m) if m.contains_meta_key(&MetaKey::Call) => {
                let f = m.get_meta_value(&MetaKey::Call).unwrap();
                // Set the callable value as the instance by placing it in the frame base,
                // and then passing the @|| function into call_callable
                self.set_register(info.frame_base, callable);
                self.call_callable(
                    CallInfo {
                        instance: Some(info.frame_base),
                        ..info
                    },
                    f,
                    temp_tuple_values,
                )
            }
            unexpected => unexpected_type("callable function", &unexpected),
        }
    }

    fn unpack_packed_arguments(&mut self, info: &mut CallInfo) -> Result<()> {
        if info.packed_arg_count == 0 {
            return Ok(());
        }

        // The indices of the registers that need to be unpacked are place in the registers
        // following the call args.
        let first_arg_index = self.register_index(info.frame_base + 1);
        let first_packed_arg_index = first_arg_index + info.arg_count as usize;
        let last_packed_arg_index = first_packed_arg_index + info.packed_arg_count as usize;
        let packed_arg_registers = self
            .registers
            .drain(first_packed_arg_index..last_packed_arg_index)
            .map(|packed_arg_register| match packed_arg_register {
                KValue::Number(n) => Ok(usize::from(n)),
                unexpected => unexpected_type("Number", &unexpected),
            })
            .collect::<Result<SmallVec<[usize; 4]>>>()?;
        let mut unpacked_values = ValueVec::new();

        let original_arg_count = info.arg_count as isize;

        // Unpack the packed arguments in reverse order
        for packed_arg_register in packed_arg_registers.iter() {
            // Get the index of the argument that needs to be unpacked,
            // taking in to account the offset resulting from unpacking previous packed arguments.
            // Packed arguments can be empty, which can result in a negative offset,
            // e.g. `f []..., x...`
            //         ^ The first argument is empty, so the second argument is shifted by -1
            let arg_offset = info.arg_count as isize - original_arg_count;
            let unpack_index =
                ((first_arg_index + packed_arg_register) as isize + arg_offset) as usize;

            // First, swap-remove the argument to be unpacked,
            // replacing the argument with null and keeping any trailing registers in place.
            self.registers.push(KValue::Null);
            let iterable = self.registers.swap_remove(unpack_index);

            // Convert the value into an iterator
            let iterator = self.make_iterator(iterable)?;

            // Process the iterator output, checking for errors and collecting `ValuePair`s
            let max_unpacked_args = (u8::MAX - info.arg_count - 1) as usize; // -1 for frame base
            for output in iterator {
                if unpacked_values.len() == max_unpacked_args {
                    return runtime_error!("Call argument limit reached during unpacking");
                }
                match output {
                    KIteratorOutput::Value(value) => unpacked_values.push(value),
                    KIteratorOutput::ValuePair(a, b) => {
                        unpacked_values.push(KTuple::from(&[a, b]).into())
                    }
                    KIteratorOutput::Error(e) => return Err(e),
                }
            }

            info.arg_count -= 1; // Subtract 1 for the arg that was unpacked
            info.arg_count += unpacked_values.len() as u8; // Add the unpacked value count

            // Splice the unpacked args into the register stack, replacing the register that
            // was occupied by the original argument.
            self.registers
                .splice(unpack_index..unpack_index + 1, unpacked_values.drain(..));
        }

        Ok(())
    }

    fn run_debug_instruction(
        &mut self,
        register: u8,
        expression_constant: ConstantIndex,
    ) -> Result<()> {
        let value = self.clone_register(register);
        let value_string = match self.run_unary_op(UnaryOp::Debug, value)? {
            KValue::Str(s) => s,
            unexpected => return unexpected_type("a displayable value", &unexpected),
        };

        let prefix = match (
            self.reader
                .chunk
                .debug_info
                .get_source_span(self.instruction_ip),
            self.reader.chunk.path.as_ref(),
        ) {
            (Some(span), Some(path)) => format!("[{}: {}] ", path, span.start.line + 1),
            (Some(span), None) => format!("[{}] ", span.start.line + 1),
            (None, Some(path)) => format!("[{}: #ERR] ", path),
            (None, None) => "[#ERR] ".to_string(),
        };

        let expression_string = self.get_constant_str(expression_constant);

        self.stdout()
            .write_line(&format!("{prefix}{expression_string}: {value_string}"))
    }

    fn run_check_size_equal(&mut self, value_register: u8, expected_size: usize) -> Result<()> {
        let size = self.get_value_size(value_register)?;
        if size == expected_size {
            Ok(())
        } else {
            runtime_error!("the container has a size of '{size}', expected '{expected_size}'")
        }
    }

    fn run_check_size_min(&mut self, value_register: u8, expected_size: usize) -> Result<()> {
        let size = self.get_value_size(value_register)?;
        if size >= expected_size {
            Ok(())
        } else {
            runtime_error!(
                "The container has a size of '{size}', expected a minimum of  '{expected_size}'"
            )
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

    fn get_value_size(&mut self, value_register: u8) -> Result<usize> {
        match self.run_unary_op(UnaryOp::Size, self.clone_register(value_register))? {
            KValue::Number(n) => Ok(n.into()),
            unexpected => unexpected_type("number for value size", &unexpected),
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
        format_options: &Option<StringFormatOptions>,
    ) -> Result<()> {
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
                        KValue::Str(rendered) => match precision {
                            Some(precision) => {
                                // `precision` acts as a maximum width for non-number values
                                let mut truncated =
                                    String::with_capacity((precision as usize).min(rendered.len()));
                                for grapheme in rendered.graphemes(true).take(precision as usize) {
                                    truncated.push_str(grapheme);
                                }
                                truncated
                            }
                            None => rendered.to_string(),
                        },
                        other => return unexpected_type("String", &other),
                    }
                }
                _ => {
                    match self.run_unary_op(UnaryOp::Display, other)? {
                        KValue::Str(rendered) => match precision {
                            Some(precision) => {
                                // `precision` acts as a maximum width for non-number values
                                let mut truncated =
                                    String::with_capacity((precision as usize).min(rendered.len()));
                                for grapheme in rendered.graphemes(true).take(precision as usize) {
                                    truncated.push_str(grapheme);
                                }
                                truncated
                            }
                            None => rendered.to_string(),
                        },
                        other => return unexpected_type("String", &other),
                    }
                }
            },
        };

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
    fn push_frame(
        &mut self,
        chunk: Ptr<Chunk>,
        ip: u32,
        frame_base: u8,
        return_register: Option<u8>,
    ) {
        let return_ip = self.ip();
        if let Some(frame) = self.call_stack.last_mut() {
            frame.return_instruction_ip = self.instruction_ip;
            frame.return_resume_ip = return_ip;
            frame.return_value_register = return_register;
        }

        let previous_frame_base = self.register_base;
        let new_frame_base = previous_frame_base + frame_base as usize;

        self.call_stack
            .push(Frame::new(chunk.clone(), new_frame_base));
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
        error.extend_trace(self.chunk(), self.instruction_ip);

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
                        error.extend_trace(self.chunk(), self.instruction_ip);
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

    pub(crate) fn register_slice(&self, register: u8, count: u8) -> &[KValue] {
        if count > 0 {
            let start = self.register_index(register);
            &self.registers[start..start + count as usize]
        } else {
            &[]
        }
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

        let Some(captures) = f.captures.as_ref() else {
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

// See [KotoVm::call_koto_function] and [KotoVm::call_generator]
fn apply_captures_and_temp_tuple_values(
    registers: &mut Vec<KValue>,
    f: &KFunction,
    temp_tuple_values: Option<&[KValue]>,
) {
    if let Some(captures) = &f.captures {
        // Copy the captures list into the registers following the args
        registers.extend(
            captures
                .data()
                .iter()
                .skip(f.optional_arg_count as usize)
                .cloned(),
        );
    }

    // Place any temp tuple values in the registers following the args and captures
    if let Some(temp_tuple_values) = temp_tuple_values {
        registers.extend_from_slice(temp_tuple_values);
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
/// implements `Into<KValue>`, or an array or slice of `KValue`s.
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
    /// then a temporary tuple will be used, which avoids the allocation of a regular KTuple.
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

// A cache of the export maps of imported modules
//
// The Map is optional to prevent recursive imports (see Vm::run_import).
type ModuleCache = HashMap<PathBuf, Option<KMap>, BuildHasherDefault<FxHasher>>;

// A frame in the VM's call stack
#[derive(Clone, Debug)]
struct Frame {
    // The chunk being interpreted in this frame
    pub chunk: Ptr<Chunk>,
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
}

impl Frame {
    pub fn new(chunk: Ptr<Chunk>, register_base: usize) -> Self {
        Self {
            chunk,
            register_base,
            required_registers: 0,
            return_resume_ip: 0,
            return_value_register: None,
            return_instruction_ip: 0,
            catch_stack: vec![],
            execution_barrier: false,
        }
    }
}

// See Vm::call_external
enum ExternalCallable {
    Function(KNativeFunction),
    Object(KObject),
}

// See Vm::call_callable
#[derive(Debug)]
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

/// An output value from [KotoVm::continue_running], either from a `return` or `yield` expression
#[allow(missing_docs)]
pub enum ReturnOrYield {
    Return(KValue),
    Yield(KValue),
}
