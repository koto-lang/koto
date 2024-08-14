use crate::{
    core_lib::CoreLib,
    error::{Error, ErrorKind},
    prelude::*,
    types::{meta_id_to_key, value::RegisterSlice},
    DefaultStderr, DefaultStdin, DefaultStdout, KCaptureFunction, KFunction, Ptr, Result,
};
use instant::Instant;
use koto_bytecode::{Chunk, Instruction, InstructionReader, Loader};
use koto_parser::{ConstantIndex, MetaKeyId, StringAlignment, StringFormatOptions};
use rustc_hash::FxHasher;
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
    loader: KCell<Loader>,
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
            loader: Loader::default().into(),
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
    pub execution_limit: Option<Duration>,

    /// An optional callback that is called whenever a module is imported by the runtime
    ///
    /// This allows you to track the runtime's dependencies, which might be useful if you want to
    /// reload the script when one of its dependencies has changed.
    pub module_imported_callback: Option<Box<dyn ModuleImportedCallback>>,

    /// The runtime's stdin
    pub stdin: Ptr<dyn KotoFile>,

    /// The runtime's stdout
    pub stdout: Ptr<dyn KotoFile>,

    /// The runtime's stderr
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
            call_stack: Vec::new(),
            sequence_builders: Vec::new(),
            string_builders: Vec::new(),
            instruction_ip: 0,
            execution_state: ExecutionState::Inactive,
        }
    }

    /// Spawn a VM that shares the same execution context
    ///
    /// e.g.
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
            call_stack: Vec::new(),
            sequence_builders: Vec::new(),
            string_builders: Vec::new(),
            instruction_ip: 0,
            execution_state: ExecutionState::Inactive,
        }
    }

    /// The loader, responsible for loading and compiling Koto scripts and modules
    pub fn loader(&self) -> &KCell<Loader> {
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

    /// The stdin wrapper used by the VM
    pub fn stdin(&self) -> &Ptr<dyn KotoFile> {
        &self.context.settings.stdin
    }

    /// The stdout wrapper used by the VM
    pub fn stdout(&self) -> &Ptr<dyn KotoFile> {
        &self.context.settings.stdout
    }

    /// The stderr wrapper used by the VM
    pub fn stderr(&self) -> &Ptr<dyn KotoFile> {
        &self.context.settings.stderr
    }

    /// Runs the provided [Chunk], returning the resulting [KValue]
    pub fn run(&mut self, chunk: Ptr<Chunk>) -> Result<KValue> {
        // Set up an execution frame to run the chunk in
        let result_register = self.next_register();
        let frame_base = result_register + 1;
        self.registers.push(KValue::Null); // result register
        self.registers.push(KValue::Null); // instance register
        self.push_frame(chunk, 0, frame_base, result_register);

        // Ensure that execution stops here if an error is thrown
        self.frame_mut().execution_barrier = true;

        // Run the chunk
        let result = self.execute_instructions();
        if result.is_err() {
            self.pop_frame(KValue::Null)?;
        }

        // Reset the value stack back to where it was at the start of the run
        self.truncate_registers(result_register);
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

        self.registers.push(KValue::Null); // result register
        self.registers.push(instance.unwrap_or_default()); // frame base
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
                    KValue::Function(f) if f.arg_is_unpacked_tuple => {
                        let temp_tuple = KValue::TemporaryTuple(RegisterSlice {
                            // The unpacked tuple contents go into the registers after the
                            // the temp tuple and instance registers.
                            start: 2,
                            count: args.len() as u8,
                        });
                        self.registers.push(temp_tuple);
                        (1, Some(args))
                    }
                    KValue::CaptureFunction(f) if f.info.arg_is_unpacked_tuple => {
                        let temp_tuple = KValue::TemporaryTuple(RegisterSlice {
                            // The unpacked tuple contents go into the registers after the
                            // captures, which are placed after the temp tuple and instance
                            // registers.
                            start: f.captures.len() as u8 + 2,
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
            &CallInfo {
                result_register,
                frame_base,
                arg_count,
            },
            function,
            temp_tuple_values,
        )?;

        let result = if self.call_stack.len() == old_frame_count {
            // If the call stack is the same size as before calling the function,
            // then an external function was called and the result should be in the frame base.
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

        self.registers.push(KValue::Null); // result_register
        self.registers.push(value); // value_register

        match op {
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
                    self.call_overridden_unary_op(result_register, value_register, op)?
                }
                unexpected => {
                    return unexpected_type(
                        "Value with an implementation of @next_back",
                        &unexpected,
                    )
                }
            },
            Size => self.run_size(result_register, value_register, true)?,
        }

        let result = if self.call_stack.len() == old_frame_count {
            // If the call stack is the same size, then the result will be in the result register
            Ok(self.clone_register(result_register))
        } else {
            // If the call stack size has changed, then an overridden operator has been called.
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

        self.registers.push(KValue::Null); // result register
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
            // If the call stack is the same size, then the result will be in the result register
            Ok(self.clone_register(result_register))
        } else {
            // If the call stack size has changed, then an overridden operator has been called.
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

    /// Runs any tests that are contained in the map's @tests meta entry
    ///
    /// Any test failure will be returned as an error.
    pub fn run_tests(&mut self, tests: KMap) -> Result<KValue> {
        use KValue::{Map, Null};

        // It's important throughout this function to make sure we don't hang on to any references
        // to the internal test map data while calling the test functions, otherwise we'll end up in
        // deadlocks when the map needs to be modified (e.g. in pre or post test functions).

        let (pre_test, post_test, meta_entry_count) = match tests.meta_map() {
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

        let self_arg = Map(tests.clone());

        for i in 0..meta_entry_count {
            let meta_entry = tests.meta_map().and_then(|meta| {
                meta.borrow()
                    .get_index(i)
                    .map(|(key, value)| (key.clone(), value.clone()))
            });

            match meta_entry {
                Some((MetaKey::Test(test_name), test)) if test.is_callable() => {
                    let make_test_error = |error: Error, message: &str| {
                        Err(error.with_prefix(&format!("{message} '{test_name}'")))
                    };

                    if let Some(pre_test) = &pre_test {
                        if pre_test.is_callable() {
                            let pre_test_result = self.call_instance_function(
                                self_arg.clone(),
                                pre_test.clone(),
                                &[],
                            );

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
                            let post_test_result = self.call_instance_function(
                                self_arg.clone(),
                                post_test.clone(),
                                &[],
                            );

                            if let Err(error) = post_test_result {
                                return make_test_error(error, "Error after running test");
                            }
                        }
                    }
                }
                _ => {}
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
            Call {
                result,
                function,
                frame_base,
                arg_count,
            } => self.call_callable(
                &CallInfo {
                    result_register: result,
                    frame_base,
                    arg_count,
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
            SetIndex {
                register,
                index,
                value,
            } => self.run_set_index(register, index, value)?,
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
            Debug { register, constant } => self.run_debug(register, constant)?,
            CheckSizeEqual { register, size } => self.run_check_size_equal(register, size)?,
            CheckSizeMin { register, size } => self.run_check_size_min(register, size)?,
            AssertType { value, type_string } => self.run_assert_type(value, type_string)?,
            CheckType {
                value,
                jump_offset,
                type_string,
            } => self.run_check_type(value, jump_offset as u32, type_string)?,
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
                return unexpected_type("a Number for the range's end", unexpected)
            }
            (Some(unexpected), _) => {
                return unexpected_type("a Number for the range's start", unexpected)
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
    // temp_iterator is used for temporary unpacking operations.
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
                    return self.call_overridden_unary_op(result_register, iterable_register, op);
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

        let output = match self.clone_register(iterable_register) {
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
                            // The output is going to be ignored, but we use Some here to indicate that
                            // iteration should continue.
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
                let old_frame_count = self.call_stack.len();
                let call_result_register = self.next_register();
                self.call_overridden_unary_op(call_result_register, iterable_register, op)?;
                if self.call_stack.len() == old_frame_count {
                    // If the call stack is the same size,
                    // then the result will be in the result register
                    Some(self.clone_register(call_result_register))
                } else {
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
            }
            other => {
                // The iterable isn't an Iterator, but might be a temporary value that's being used
                // during unpacking.
                let (output, new_iterable) = match other {
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
                    unexpected => return unexpected_type("Iterator", &unexpected),
                };

                self.set_register(iterable_register, new_iterable);
                output
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
                s.with_bounds(index..index + 1).map_or(Null, KValue::from)
            }
            Map(map) if map.contains_meta_key(&index_op) => {
                let op = map.get_meta_value(&index_op).unwrap();
                return self.call_overridden_binary_op(result, value, index.into(), op);
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
                    tuple.make_sub_tuple(0..index).map_or(Null, Tuple)
                } else {
                    tuple.make_sub_tuple(index..tuple.len()).map_or(Null, Tuple)
                }
            }
            Str(s) => {
                let index = signed_index_to_unsigned(index, s.len());
                if is_slice_to {
                    s.with_bounds(0..index).map_or(Null, KValue::from)
                } else {
                    s.with_bounds(index..s.len()).map_or(Null, KValue::from)
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
        use KValue::*;

        match function_instruction {
            Instruction::Function {
                register,
                arg_count,
                capture_count,
                variadic,
                generator,
                arg_is_unpacked_tuple,
                size,
            } => {
                let info = KFunction {
                    chunk: self.chunk(),
                    ip: self.ip(),
                    arg_count,
                    variadic,
                    arg_is_unpacked_tuple,
                    generator,
                };

                let value = if capture_count > 0 {
                    // Initialize the function's captures with Null
                    let mut captures = ValueVec::new();
                    captures.resize(capture_count as usize, Null);
                    CaptureFunction(
                        KCaptureFunction {
                            info,
                            captures: KList::with_data(captures),
                        }
                        .into(),
                    )
                } else {
                    Function(info)
                };

                self.jump_ip(size as u32);
                self.set_register(register, value);
            }
            _ => unreachable!(),
        }
    }

    fn run_capture_value(&mut self, function: u8, capture_index: u8, value: u8) -> Result<()> {
        let Some(function) = self.get_register_safe(function) else {
            // e.g. x = (1..10).find |n| n == x
            // The function was temporary and has been removed from the value stack,
            // but the capture of `x` is still attempted. It would be cleaner for the compiler to
            // detect this case but for now a runtime error will have to do.
            return runtime_error!("Function not found while attempting to capture a value");
        };

        match function {
            KValue::CaptureFunction(f) => {
                f.captures.data_mut()[capture_index as usize] = self.clone_register(value);
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
                return self.call_overridden_unary_op(result, value, op);
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

    fn run_display(&mut self, result: u8, value: u8) -> Result<()> {
        use UnaryOp::Display;

        match self.clone_register(value) {
            KValue::Map(m) if m.contains_meta_key(&Display.into()) => {
                let op = m.get_meta_value(&Display.into()).unwrap();
                self.call_overridden_unary_op(result, value, op)
            }
            other => {
                let mut display_context = DisplayContext::with_vm(self);
                match other.display(&mut display_context) {
                    Ok(_) => {
                        self.set_register(result, display_context.result().into());
                        Ok(())
                    }
                    Err(_) => runtime_error!("Failed to get display value"),
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
                return self.call_overridden_binary_op(result, lhs, rhs_value, op);
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
                return self.call_overridden_binary_op(result, lhs, rhs_value, op);
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
                return self.call_overridden_binary_op(result, lhs, rhs_value, op);
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
                return self.call_overridden_binary_op(result, lhs, rhs_value, op);
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
                return self.call_overridden_binary_op(result, lhs, rhs_value, op);
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
                let unused = self.next_register();
                self.call_overridden_binary_op(unused, lhs, rhs_value, op)
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
                let unused = self.next_register();
                self.call_overridden_binary_op(unused, lhs, rhs_value, op)
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
                let unused = self.next_register();
                self.call_overridden_binary_op(unused, lhs, rhs_value, op)
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
                let unused = self.next_register();
                self.call_overridden_binary_op(unused, lhs, rhs_value, op)
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
                let unused = self.next_register();
                self.call_overridden_binary_op(unused, lhs, rhs_value, op)
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
                return self.call_overridden_binary_op(result, lhs, rhs_value, op);
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
                return self.call_overridden_binary_op(result, lhs, rhs_value, op);
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
                return self.call_overridden_binary_op(result, lhs, rhs_value, op);
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
                return self.call_overridden_binary_op(result, lhs, rhs_value, op);
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
                return self.call_overridden_binary_op(result, lhs, rhs_value, op);
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
            (CaptureFunction(a), CaptureFunction(b)) => {
                if a.info == b.info {
                    let captures_a = a.captures.clone();
                    let captures_b = b.captures.clone();
                    let data_a = captures_a.data();
                    let data_b = captures_b.data();
                    self.compare_value_ranges(&data_a, &data_b)?
                } else {
                    false
                }
            }
            (Function(a), Function(b)) => a == b,
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
                return self.call_overridden_binary_op(result, lhs, rhs_value, op);
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
            (CaptureFunction(a), CaptureFunction(b)) => {
                if a.info == b.info {
                    let captures_a = a.captures.clone();
                    let captures_b = b.captures.clone();
                    let data_a = captures_a.data();
                    let data_b = captures_b.data();
                    !self.compare_value_ranges(&data_a, &data_b)?
                } else {
                    true
                }
            }
            _ => true,
        };
        self.set_register(result, result_value.into());

        Ok(())
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
        result_register: u8,
        value_register: u8,
        op: KValue,
    ) -> Result<()> {
        // Ensure that the result register is present in the stack, otherwise it might be lost after
        // the call to the op, which expects a frame base at or after the result register.
        if self.register_index(result_register) >= self.registers.len() {
            self.set_register(result_register, KValue::Null);
        }

        // Set up the call registers at the end of the stack
        let frame_base = self.new_frame_base()?;
        self.registers.push(self.clone_register(value_register)); // frame_base
        self.call_callable(
            &CallInfo {
                result_register,
                frame_base,
                arg_count: 0,
            },
            op,
            None,
        )
    }

    fn call_overridden_binary_op(
        &mut self,
        result_register: u8,
        lhs_register: u8,
        rhs: KValue,
        op: KValue,
    ) -> Result<()> {
        // Ensure that the result register is present in the stack, otherwise it might be lost after
        // the call to the op, which expects a frame base at or after the result register.
        if self.register_index(result_register) >= self.registers.len() {
            self.set_register(result_register, KValue::Null);
        }

        // Set up the call registers at the end of the stack
        let frame_base = self.new_frame_base()?;
        self.registers.push(self.clone_register(lhs_register)); // frame_base
        self.registers.push(rhs); // arg
        self.call_callable(
            &CallInfo {
                result_register,
                frame_base,
                arg_count: 1, // 1 arg, the rhs value
            },
            op,
            None,
        )
    }

    fn run_jump_if_true(&mut self, register: u8, offset: u32) -> Result<()> {
        match &self.get_register(register) {
            KValue::Null => {}
            KValue::Bool(b) if !b => {}
            _ => self.jump_ip(offset),
        }
        Ok(())
    }

    fn run_jump_if_false(&mut self, register: u8, offset: u32) -> Result<()> {
        match &self.get_register(register) {
            KValue::Null => self.jump_ip(offset),
            KValue::Bool(b) if !b => self.jump_ip(offset),
            _ => {}
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
                return self.call_overridden_unary_op(result_register, value_register, op);
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
        let source_path = self.reader.chunk.source_path.clone();
        let compile_result = match self
            .context
            .loader
            .borrow_mut()
            .compile_module(&import_name, source_path.as_deref())
        {
            Ok(result) => result,
            Err(error) => return runtime_error!("Failed to import '{import_name}': {error}"),
        };

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
                return runtime_error!("Recursive import of module '{import_name}'");
            }
            Some(Some(cached_exports)) if compile_result.loaded_from_cache => {
                self.set_register(import_register, KValue::Map(cached_exports));
                return Ok(());
            }
            _ => {}
        }

        // The module needs to be loaded, which involves the following steps:
        //   - Execute the module's script
        //   - If the module contains @tests, run them
        //   - If the module contains a @main function, run it
        //   - If the steps above are successful, then cache the resulting exports map

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
                    let maybe_tests = self.exports.get_meta_value(&MetaKey::Tests);
                    match maybe_tests {
                        Some(KValue::Map(tests)) => {
                            self.run_tests(tests)?;
                        }
                        Some(other) => {
                            return runtime_error!(
                                "Expected map for tests in module '{import_name}', found '{}'",
                                other.type_as_string()
                            )
                        }
                        None => {}
                    }
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

    fn run_set_index(
        &mut self,
        indexable_register: u8,
        index_register: u8,
        value_register: u8,
    ) -> Result<()> {
        use KValue::*;

        let indexable = self.clone_register(indexable_register);
        let index_value = self.clone_register(index_register);
        let value = self.clone_register(value_register);

        match indexable {
            List(list) => {
                let mut list_data = list.data_mut();
                let list_len = list_data.len();
                match index_value {
                    Number(index) => {
                        let u_index = usize::from(index);
                        if index >= 0.0 && u_index < list_len {
                            list_data[u_index] = value;
                        } else {
                            return runtime_error!("Index '{index}' not in List");
                        }
                    }
                    Range(range) => {
                        for i in range.indices(list_len) {
                            list_data[i] = value.clone();
                        }
                    }
                    unexpected => return unexpected_type("index", &unexpected),
                }
            }
            unexpected => return unexpected_type("a mutable indexable value", &unexpected),
        };

        Ok(())
    }

    fn validate_index(&self, n: KNumber, size: Option<usize>) -> Result<usize> {
        let index = usize::from(n);

        if n < 0.0 {
            return runtime_error!("Negative indices aren't allowed ('{n}')");
        } else if let Some(size) = size {
            if index >= size {
                return runtime_error!("Index out of bounds - index: {n}, size: {size}");
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
        let index_op = BinaryOp::Index.into();

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
                    // range.indices is guaranteed to return valid indices for the tuple
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
            (Map(m), index) if m.contains_meta_key(&index_op) => {
                let op = m.get_meta_value(&index_op).unwrap();
                return self.call_overridden_binary_op(result_register, value_register, index, op);
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
                )
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
                    runtime_error!("Insertion not supported for '{}'", o.type_string())
                }
            }
            unexpected => unexpected_type("a value that supports insertion", unexpected),
        }
    }

    fn run_meta_insert(&mut self, map_register: u8, value: u8, meta_id: MetaKeyId) -> Result<()> {
        let value = self.clone_register(value);
        let meta_key = match meta_id_to_key(meta_id, None) {
            Ok(meta_key) => meta_key,
            Err(error) => return runtime_error!("Error while preparing meta key: {error}"),
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
                Err(error) => return runtime_error!("Error while preparing meta key: {error}"),
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
            Err(error) => return runtime_error!("Error while preparing meta key: {error}"),
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
                Err(error) => return runtime_error!("Error while preparing meta key: {error}"),
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
                                    return unexpected_type("Map as base value", &unexpected)
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

    fn call_external(&mut self, call_info: &CallInfo, callable: ExternalCallable) -> Result<()> {
        let mut call_context = CallContext::new(self, call_info.frame_base, call_info.arg_count);

        let result = match callable {
            ExternalCallable::Function(f) => (f.function)(&mut call_context),
            ExternalCallable::Object(o) => o.try_borrow_mut()?.call(&mut call_context),
        }?;

        self.set_register(call_info.result_register, result);
        // External function calls don't use the push/pop frame mechanism,
        // so drop the call args here now that the call has been completed.
        self.truncate_registers(call_info.frame_base);

        Ok(())
    }

    fn call_generator(
        &mut self,
        call_info: &CallInfo,
        f: &KFunction,
        captures: Option<&KList>,
        temp_tuple_values: Option<&[KValue]>,
    ) -> Result<()> {
        // Spawn a VM for the generator
        let mut generator_vm = self.spawn_shared_vm();
        // Push a frame for running the generator function
        generator_vm.push_frame(
            f.chunk.clone(),
            f.ip,
            0, // arguments will be copied starting in register 0
            0,
        );
        // Set the generator VM's state as suspended
        generator_vm.execution_state = ExecutionState::Suspended;

        let expected_arg_count = if f.variadic {
            f.arg_count - 1
        } else {
            f.arg_count
        };

        // Place the instance in the first register of the generator vm
        let instance = self
            .get_register_safe(call_info.frame_base)
            .cloned()
            .unwrap_or(KValue::Null);
        generator_vm.set_register(0, instance);

        let arg_offset = 1;

        // Copy any regular (non-variadic) arguments into the generator vm
        for (arg_index, arg) in self
            .register_slice(
                call_info.frame_base + 1,
                expected_arg_count.min(call_info.arg_count),
            )
            .iter()
            .cloned()
            .enumerate()
        {
            generator_vm.set_register(arg_index as u8 + arg_offset, arg);
        }

        // Ensure that registers for missing arguments are set to Null
        if call_info.arg_count < expected_arg_count {
            for arg_index in call_info.arg_count..expected_arg_count {
                generator_vm.set_register(arg_index + arg_offset, KValue::Null);
            }
        }

        // Check for variadic arguments, and validate argument count
        if f.variadic {
            let variadic_register = expected_arg_count + arg_offset;
            if call_info.arg_count >= expected_arg_count {
                // Capture the varargs into a tuple and place them in the
                // generator vm's last arg register
                let varargs_start = call_info.frame_base + 1 + expected_arg_count;
                let varargs_count = call_info.arg_count - expected_arg_count;
                let varargs =
                    KValue::Tuple(self.register_slice(varargs_start, varargs_count).into());
                generator_vm.set_register(variadic_register, varargs);
            } else {
                generator_vm.set_register(variadic_register, KValue::Null);
            }
        }
        // Place any captures in the registers following the arguments
        if let Some(captures) = captures {
            generator_vm
                .registers
                .extend(captures.data().iter().cloned())
        }

        // Place any temp tuple values in the registers following the args and captures
        if let Some(temp_tuple_values) = temp_tuple_values {
            generator_vm.registers.extend_from_slice(temp_tuple_values);
        }

        // The args have been cloned into the generator vm, so at this point they can be removed
        self.truncate_registers(call_info.frame_base);

        // Wrap the generator vm in an iterator and place it in the result register
        self.set_register(
            call_info.result_register,
            KIterator::with_vm(generator_vm).into(),
        );

        Ok(())
    }

    fn call_koto_function(
        &mut self,
        call_info: &CallInfo,
        f: &KFunction,
        captures: Option<&KList>,
        temp_tuple_values: Option<&[KValue]>,
    ) -> Result<()> {
        if f.generator {
            return self.call_generator(call_info, f, captures, temp_tuple_values);
        }

        let expected_arg_count = if f.variadic {
            f.arg_count - 1
        } else {
            f.arg_count
        };

        if f.variadic && call_info.arg_count >= expected_arg_count {
            // The last defined arg is the start of the var_args,
            // e.g. f = |x, y, z...|
            // arg index 2 is the first vararg, and where the tuple will be placed
            let arg_base = call_info.frame_base + 1;
            let varargs_start = arg_base + expected_arg_count;
            let varargs_count = call_info.arg_count - expected_arg_count;
            let varargs = KValue::Tuple(self.register_slice(varargs_start, varargs_count).into());
            self.set_register(varargs_start, varargs);
            self.truncate_registers(varargs_start + 1);
        }

        // self is in the frame base register, arguments start from register frame_base + 1
        let arg_base_index = self.register_index(call_info.frame_base) + 1;

        // Ensure that any temporary registers used to prepare the call args have been removed
        // from the value stack.
        self.registers
            .truncate(arg_base_index + call_info.arg_count as usize);
        // Ensure that registers have been filled with Null for any missing args.
        // If there are extra args, truncating is necessary at this point. Extra args have either
        // been bundled into a variadic Tuple or they can be ignored.
        self.registers
            .resize(arg_base_index + f.arg_count as usize, KValue::Null);

        if let Some(captures) = captures {
            // Copy the captures list into the registers following the args
            self.registers.extend(captures.data().iter().cloned());
        }

        // Place any temp tuple values in the registers following the args and captures
        if let Some(temp_tuple_values) = temp_tuple_values {
            self.registers.extend_from_slice(temp_tuple_values);
        }

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
        info: &CallInfo,
        callable: KValue,
        temp_tuple_values: Option<&[KValue]>,
    ) -> Result<()> {
        use KValue::*;

        match callable {
            Function(f) => self.call_koto_function(info, &f, None, temp_tuple_values),
            CaptureFunction(f) => {
                self.call_koto_function(info, &f.info, Some(&f.captures), temp_tuple_values)
            }
            NativeFunction(f) => self.call_external(info, ExternalCallable::Function(f)),
            Object(o) => self.call_external(info, ExternalCallable::Object(o)),
            Map(ref m) if m.contains_meta_key(&MetaKey::Call) => {
                let f = m.get_meta_value(&MetaKey::Call).unwrap();
                // Set the callable value as the instance by placing it in the frame base,
                // and then passing the @|| function into call_callable
                self.set_register(info.frame_base, callable);
                self.call_callable(info, f, temp_tuple_values)
            }
            unexpected => unexpected_type("callable function", &unexpected),
        }
    }

    fn run_debug(&mut self, register: u8, expression_constant: ConstantIndex) -> Result<()> {
        let value = self.clone_register(register);
        let value_string = match self.run_unary_op(UnaryOp::Display, value)? {
            KValue::Str(s) => s,
            unexpected => return unexpected_type("a displayable value", &unexpected),
        };

        let prefix = match (
            self.reader
                .chunk
                .debug_info
                .get_source_span(self.instruction_ip),
            self.reader.chunk.source_path.as_ref(),
        ) {
            (Some(span), Some(path)) => format!("[{}: {}] ", path.display(), span.start.line + 1),
            (Some(span), None) => format!("[{}] ", span.start.line + 1),
            (None, Some(path)) => format!("[{}: #ERR] ", path.display()),
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
            runtime_error!("The container has a size of '{size}', expected '{expected_size}'")
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

    fn run_assert_type(&self, value_register: u8, type_index: ConstantIndex) -> Result<()> {
        if self.compare_value_type(value_register, type_index) {
            Ok(())
        } else {
            unexpected_type(
                self.get_constant_str(type_index),
                self.get_register(value_register),
            )
        }
    }

    fn run_check_type(
        &mut self,
        value_register: u8,
        jump_offset: u32,
        type_index: ConstantIndex,
    ) -> Result<()> {
        if !self.compare_value_type(value_register, type_index) {
            self.jump_ip(jump_offset);
        }
        Ok(())
    }

    fn compare_value_type(&self, value_register: u8, type_index: ConstantIndex) -> bool {
        let value = self.get_register(value_register);
        match self.get_constant_str(type_index) {
            "Any" => true,
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
        let rendered = match value {
            KValue::Number(n) => match precision {
                Some(precision) if n.is_f64() || n.is_i64_in_f64_range() => {
                    format!("{:.*}", precision as usize, f64::from(n))
                }
                _ => n.to_string(),
            },
            other => match self.run_unary_op(UnaryOp::Display, other)? {
                KValue::Str(rendered) => match precision {
                    Some(precision) => {
                        // precision acts as a maximum width for non-number values
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

    fn push_frame(&mut self, chunk: Ptr<Chunk>, ip: u32, frame_base: u8, return_register: u8) {
        let return_ip = self.ip();
        let previous_frame_base = if let Some(frame) = self.call_stack.last_mut() {
            frame.return_register_and_ip = Some((return_register, return_ip));
            frame.return_instruction_ip = self.instruction_ip;
            frame.register_base
        } else {
            0
        };
        let new_frame_base = previous_frame_base + frame_base as usize;

        self.call_stack
            .push(Frame::new(chunk.clone(), new_frame_base));
        self.set_chunk_and_ip(chunk, ip);
    }

    fn pop_frame(&mut self, return_value: KValue) -> Result<Option<KValue>> {
        self.truncate_registers(0);

        match self.call_stack.pop() {
            Some(popped_frame) => {
                if self.call_stack.is_empty() {
                    Ok(Some(return_value))
                } else {
                    let (return_register, return_ip) = self.frame().return_register_and_ip.unwrap();

                    self.set_register(return_register, return_value.clone());
                    self.set_chunk_and_ip(self.frame().chunk.clone(), return_ip);
                    self.instruction_ip = self.frame().return_instruction_ip;

                    if popped_frame.execution_barrier {
                        Ok(Some(return_value))
                    } else {
                        Ok(None)
                    }
                }
            }
            None => {
                runtime_error!(ErrorKind::EmptyCallStack)
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
                    return Ok((*error_register, *catch_ip))
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
        u8::try_from(self.registers.len() - self.register_base())
            .map_err(|_| "Overflow of Koto's stack".into())
    }

    fn register_base(&self) -> usize {
        match self.call_stack.last() {
            Some(frame) => frame.register_base,
            None => 0,
        }
    }

    fn register_index(&self, register: u8) -> usize {
        self.register_base() + register as usize
    }

    // Returns the register id that corresponds to the next push to the value stack
    fn next_register(&self) -> u8 {
        (self.registers.len() - self.register_base()) as u8
    }

    fn set_register(&mut self, register: u8, value: KValue) {
        let index = self.register_index(register);

        if index >= self.registers.len() {
            self.registers.resize(index + 1, KValue::Null);
        }

        self.registers[index] = value;
    }

    #[track_caller]
    fn clone_register(&self, register: u8) -> KValue {
        self.get_register(register).clone()
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
        self.registers.truncate(self.register_base() + len as usize);
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

// Used when calling iterator.copy on a generator
//
// The idea here is to clone the VM, and then scan through the value stack to make copies of
// any iterators that it finds. This makes simple generators copyable, although any captured or
// contained iterators in the generator VM will have shared state. This behaviour is noted in the
// documentation for iterator.copy and should hopefully be sufficient.
pub(crate) fn clone_generator_vm(vm: &KotoVm) -> Result<KotoVm> {
    let mut result = vm.clone();
    for value in result.registers.iter_mut() {
        if let KValue::Iterator(ref mut i) = value {
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
    // When returning to this frame, the ip that produced the most recently read instruction
    pub return_instruction_ip: u32,
    // When returning to this frame, the register for the return value and the ip to resume from.
    pub return_register_and_ip: Option<(u8, u32)>,
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
            return_register_and_ip: None,
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
    result_register: u8,
    frame_base: u8,
    arg_count: u8,
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
