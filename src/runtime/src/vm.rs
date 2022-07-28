use {
    crate::{
        core::CoreLib,
        error::{unexpected_type_error, RuntimeErrorType},
        external::{self, Args, ExternalFunction},
        frame::Frame,
        meta_map::meta_id_to_key,
        runtime_error,
        value::{self, FunctionInfo, RegisterSlice, SimpleFunctionInfo},
        value_iterator::{ValueIterator, ValueIteratorOutput},
        BinaryOp, DefaultStderr, DefaultStdin, DefaultStdout, IntRange, KotoFile, Loader, MetaKey,
        RuntimeError, RuntimeResult, UnaryOp, Value, ValueKey, ValueList, ValueMap, ValueNumber,
        ValueString, ValueTuple, ValueVec,
    },
    koto_bytecode::{Chunk, Instruction, InstructionReader, TypeId},
    koto_parser::{ConstantIndex, MetaKeyId},
    rustc_hash::FxHasher,
    std::{
        cell::RefCell,
        collections::HashMap,
        fmt,
        hash::BuildHasherDefault,
        path::{Path, PathBuf},
        rc::Rc,
    },
};

macro_rules! call_binary_op_or_else {
    ($vm:expr,
     $result_register:expr,
     $lhs_register:expr,
     $rhs_value: expr,
     $overloaded_value:expr,
     $op:tt,
     $else:tt) => {{
        let maybe_op = $overloaded_value.get_meta_value(&MetaKey::BinaryOp($op));
        if let Some(op) = maybe_op {
            let rhs_value = $rhs_value.clone();
            return $vm.call_overloaded_binary_op($result_register, $lhs_register, rhs_value, op);
        } else {
            $else
        }
    }};
}

#[derive(Clone, Debug)]
pub enum ControlFlow {
    Continue,
    Return(Value),
    Yield(Value),
}

// Instructions will place their results in registers, there's no Ok type
pub type InstructionResult = Result<(), RuntimeError>;

fn setup_core_lib_and_prelude() -> (CoreLib, ValueMap) {
    let core_lib = CoreLib::default();

    let prelude = ValueMap::default();
    prelude.add_map("io", core_lib.io.clone());
    prelude.add_map("iterator", core_lib.iterator.clone());
    prelude.add_map("koto", core_lib.koto.clone());
    prelude.add_map("list", core_lib.list.clone());
    prelude.add_map("map", core_lib.map.clone());
    prelude.add_map("os", core_lib.os.clone());
    prelude.add_map("number", core_lib.number.clone());
    prelude.add_map("num2", core_lib.num2.clone());
    prelude.add_map("num4", core_lib.num4.clone());
    prelude.add_map("range", core_lib.range.clone());
    prelude.add_map("string", core_lib.string.clone());
    prelude.add_map("test", core_lib.test.clone());
    prelude.add_map("tuple", core_lib.tuple.clone());

    macro_rules! default_import {
        ($name:expr, $module:ident) => {{
            prelude.add_value(
                $name,
                core_lib
                    .$module
                    .data()
                    .get_with_string($name)
                    .unwrap()
                    .clone(),
            );
        }};
    }

    default_import!("assert", test);
    default_import!("assert_eq", test);
    default_import!("assert_ne", test);
    default_import!("assert_near", test);
    default_import!("make_num2", num2);
    default_import!("make_num4", num4);
    default_import!("print", io);
    default_import!("type", koto);

    (core_lib, prelude)
}

/// State shared between concurrent VMs
struct VmContext {
    // The settings that were used to initialize the runtime
    settings: VmSettings,
    // The runtime's prelude
    prelude: ValueMap,
    // The runtime's core library
    core_lib: CoreLib,
    // The module loader used to compile imported modules
    loader: RefCell<Loader>,
    // The cached export maps of imported modules
    imported_modules: RefCell<ModuleCache>,
}

impl Default for VmContext {
    fn default() -> Self {
        Self::with_settings(VmSettings::default())
    }
}

impl VmContext {
    fn with_settings(settings: VmSettings) -> Self {
        let (core_lib, prelude) = setup_core_lib_and_prelude();

        Self {
            settings,
            prelude,
            core_lib,
            loader: RefCell::new(Loader::default()),
            imported_modules: RefCell::new(ModuleCache::default()),
        }
    }
}

/// The trait used by the 'module imported' callback mechanism
pub trait ModuleImportedCallback: Fn(&Path) {}

// Implement the trait for any matching function
impl<T> ModuleImportedCallback for T where T: Fn(&Path) {}

/// The configurable settings that should be used by the Koto runtime
pub struct VmSettings {
    /// Whether or not tests should be run when importing modules
    pub run_import_tests: bool,
    /// An optional callback that is called whenever a module is imported by the runtime
    ///
    /// This allows you to track the runtime's dependencies, which might be useful if you want to
    /// reload the script when one of its dependencies has changed.
    pub module_imported_callback: Option<Box<dyn ModuleImportedCallback>>,
    /// The runtime's stdin
    pub stdin: Rc<dyn KotoFile>,
    /// The runtime's stdout
    pub stdout: Rc<dyn KotoFile>,
    /// The runtime's stderr
    pub stderr: Rc<dyn KotoFile>,
}

impl Default for VmSettings {
    fn default() -> Self {
        Self {
            run_import_tests: true,
            module_imported_callback: None,
            stdin: Rc::new(DefaultStdin::default()),
            stdout: Rc::new(DefaultStdout::default()),
            stderr: Rc::new(DefaultStderr::default()),
        }
    }
}

#[derive(Clone)]
pub struct Vm {
    exports: ValueMap,
    context: Rc<VmContext>,
    reader: InstructionReader,
    value_stack: Vec<Value>,
    call_stack: Vec<Frame>,
    // The ip that produced the most recently read instruction, used for debug and error traces
    instruction_ip: usize,
}

impl Default for Vm {
    fn default() -> Self {
        Self::with_settings(VmSettings::default())
    }
}

impl Vm {
    /// Initializes a Koto VM with the provided settings
    pub fn with_settings(settings: VmSettings) -> Self {
        Self {
            exports: ValueMap::default(),
            context: Rc::new(VmContext::with_settings(settings)),
            reader: InstructionReader::default(),
            value_stack: Vec::with_capacity(32),
            call_stack: vec![],
            instruction_ip: 0,
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
            value_stack: Vec::with_capacity(8),
            call_stack: vec![],
            instruction_ip: 0,
        }
    }

    /// The loader, responsible for loading and compiling Koto scripts and modules
    pub fn loader(&self) -> &RefCell<Loader> {
        &self.context.loader
    }

    /// The prelude, containing items that can be imported within all modules
    pub fn prelude(&self) -> &ValueMap {
        &self.context.prelude
    }

    /// The active module's exports map
    ///
    /// Note that this is the exports map of the active module, so during execution the returned
    /// map will be of the module that's currently being executed.
    pub fn exports(&self) -> &ValueMap {
        &self.exports
    }

    /// The stdin wrapper used by the VM
    pub fn stdin(&self) -> &Rc<dyn KotoFile> {
        &self.context.settings.stdin
    }

    /// The stdout wrapper used by the VM
    pub fn stdout(&self) -> &Rc<dyn KotoFile> {
        &self.context.settings.stdout
    }

    /// The stderr wrapper used by the VM
    pub fn stderr(&self) -> &Rc<dyn KotoFile> {
        &self.context.settings.stderr
    }

    /// Returns the named value from the exports map, or None if no matching value is found
    pub fn get_exported_value(&self, id: &str) -> Option<Value> {
        self.exports.data().get_with_string(id).cloned()
    }

    /// Returns the named function from the exports map
    ///
    /// None is returned if no matching value is found, or if a matching value is found which isn't
    /// a callable function.
    pub fn get_exported_function(&self, id: &str) -> Option<Value> {
        match self.get_exported_value(id) {
            Some(function) if function.is_callable() => Some(function),
            _ => None,
        }
    }

    /// Runs the provided [Chunk], returning the resulting [Value]
    pub fn run(&mut self, chunk: Rc<Chunk>) -> RuntimeResult {
        // Set up an execution frame to run the chunk in
        let result_register = self.next_register();
        let frame_base = result_register + 1;
        self.value_stack.push(Value::Null); // result register
        self.value_stack.push(Value::Null); // instance register
        self.push_frame(chunk, 0, frame_base, result_register);

        // Ensure that execution stops here if an error is thrown
        self.frame_mut().execution_barrier = true;

        // Run the chunk
        let result = self.execute_instructions();
        // Reset the value stack back to where it was at the start of the run
        self.truncate_registers(result_register);
        result
    }

    /// Continues execution in a suspended VM
    ///
    /// This is currently used to support generators, which yield incremental results and then
    /// leave the VM in a suspended state.
    pub fn continue_running(&mut self) -> RuntimeResult {
        if self.call_stack.is_empty() {
            Ok(Value::Null)
        } else {
            self.execute_instructions()
        }
    }

    /// Runs a function with some given arguments
    pub fn run_function(&mut self, function: Value, args: CallArgs) -> RuntimeResult {
        self.call_and_run_function(None, function, args)
    }

    /// Runs an instance function with some given arguments
    pub fn run_instance_function(
        &mut self,
        instance: Value,
        function: Value,
        args: CallArgs,
    ) -> RuntimeResult {
        self.call_and_run_function(Some(instance), function, args)
    }

    fn call_and_run_function(
        &mut self,
        instance: Option<Value>,
        function: Value,
        args: CallArgs,
    ) -> RuntimeResult {
        if !function.is_callable() {
            return runtime_error!("run_function: the provided value isn't a function");
        }

        let result_register = self.next_register();
        let frame_base = result_register + 1;
        // If there's an instance value then it goes in the frame base
        let instance_register = if instance.is_some() {
            Some(frame_base)
        } else {
            None
        };

        self.value_stack.push(Value::Null); // result register
        self.value_stack.push(instance.unwrap_or_default()); // frame base
        let (args_count, temp_tuple_values) = match args {
            CallArgs::None => (0, None),
            CallArgs::Single(arg) => {
                self.value_stack.push(arg);
                (1, None)
            }
            CallArgs::Separate(args) => {
                self.value_stack.extend_from_slice(args);
                (args.len() as u8, None)
            }
            CallArgs::AsTuple(args) => {
                match &function {
                    Value::Function(FunctionInfo {
                        arg_is_unpacked_tuple,
                        captures,
                        ..
                    }) if *arg_is_unpacked_tuple && (args.len() as u8) < u8::MAX => {
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
                        // already in. This is redundant work, but still more efficient than
                        // allocating a non-temporary Tuple for the values.

                        let capture_count =
                            captures.as_ref().map_or(0, |captures| captures.len() as u8);

                        let temp_tuple = Value::TemporaryTuple(RegisterSlice {
                            // The unpacked tuple contents go into the registers after the
                            // captures, which are placed after the temp tuple register
                            start: capture_count + 1,
                            count: args.len() as u8,
                        });

                        self.value_stack.push(temp_tuple);
                        (1, Some(args))
                    }
                    _ => {
                        let tuple_contents = Vec::from(args);
                        self.value_stack.push(Value::Tuple(tuple_contents.into()));
                        (1, None)
                    }
                }
            }
        };

        let old_frame_count = self.call_stack.len();

        self.call_callable(
            result_register,
            function,
            frame_base,
            args_count,
            instance_register,
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
                self.pop_frame(Value::Null)?;
            }
            result
        };

        self.truncate_registers(result_register);
        result
    }

    pub fn run_unary_op(&mut self, op: UnaryOp, value: Value) -> RuntimeResult {
        let old_frame_count = self.call_stack.len();
        let result_register = self.next_register();
        let value_register = result_register + 1;

        self.value_stack.push(Value::Null); // result_register
        self.value_stack.push(value); // value_register

        match op {
            UnaryOp::Display => self.run_display(result_register, value_register)?,
            UnaryOp::Iterator => self.run_make_iterator(result_register, value_register)?,
            UnaryOp::Negate => self.run_negate(result_register, value_register)?,
            UnaryOp::Not => self.run_not(result_register, value_register)?,
        }

        let result = if self.call_stack.len() == old_frame_count {
            // If the call stack is the same size, then the result will be in the result register
            Ok(self.clone_register(result_register))
        } else {
            // If the call stack size has changed, then an overloaded operator has been called.
            self.frame_mut().execution_barrier = true;
            let result = self.execute_instructions();
            if result.is_err() {
                self.pop_frame(Value::Null)?;
            }
            result
        };

        self.truncate_registers(result_register);
        result
    }

    pub fn run_binary_op(&mut self, op: BinaryOp, lhs: Value, rhs: Value) -> RuntimeResult {
        let old_frame_count = self.call_stack.len();
        let result_register = self.next_register();
        let lhs_register = result_register + 1;
        let rhs_register = result_register + 2;

        self.value_stack.push(Value::Null); // result register
        self.value_stack.push(lhs);
        self.value_stack.push(rhs);

        match op {
            BinaryOp::Add => self.run_add(result_register, lhs_register, rhs_register)?,
            BinaryOp::Subtract => self.run_subtract(result_register, lhs_register, rhs_register)?,
            BinaryOp::Multiply => self.run_multiply(result_register, lhs_register, rhs_register)?,
            BinaryOp::Divide => self.run_divide(result_register, lhs_register, rhs_register)?,
            BinaryOp::Remainder => {
                self.run_remainder(result_register, lhs_register, rhs_register)?
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
            // If the call stack size has changed, then an overloaded operator has been called.
            self.frame_mut().execution_barrier = true;
            let result = self.execute_instructions();
            if result.is_err() {
                self.pop_frame(Value::Null)?;
            }
            result
        };

        self.truncate_registers(result_register);
        result
    }

    /// Makes a ValueIterator that iterates over the provided value's contents
    pub fn make_iterator(&mut self, value: Value) -> Result<ValueIterator, RuntimeError> {
        use Value::*;
        let value = match value {
            Map(m) if m.contains_meta_key(&MetaKey::UnaryOp(UnaryOp::Iterator)) => {
                self.run_unary_op(UnaryOp::Iterator, Map(m))?
            }
            _ => value,
        };

        let result = match value {
            Range(r) => ValueIterator::with_range(r),
            Num2(n) => ValueIterator::with_num2(n),
            Num4(n) => ValueIterator::with_num4(n),
            List(l) => ValueIterator::with_list(l),
            Tuple(t) => ValueIterator::with_tuple(t),
            Str(s) => ValueIterator::with_string(s),
            Map(m) => ValueIterator::with_map(m),
            Iterator(i) => i,
            unexpected => {
                return runtime_error!(
                    "expected iterable value, found '{}'",
                    unexpected.type_as_string()
                )
            }
        };
        Ok(result)
    }

    pub fn run_tests(&mut self, tests: ValueMap) -> RuntimeResult {
        use Value::{Function, Map, Null};

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
        let pass_self_to_pre_test = match &pre_test {
            Some(Function(f)) => f.instance_function,
            _ => false,
        };
        let pass_self_to_post_test = match &post_test {
            Some(Function(f)) => f.instance_function,
            _ => false,
        };

        for i in 0..meta_entry_count {
            let meta_entry = tests.meta_map().and_then(|meta| {
                meta.borrow()
                    .get_index(i)
                    .map(|(key, value)| (key.clone(), value.clone()))
            });

            match meta_entry {
                Some((MetaKey::Test(test_name), test)) if test.is_callable() => {
                    let make_test_error = |error: RuntimeError, message: &str| {
                        Err(error.with_prefix(&format!("{message} '{test_name}'")))
                    };

                    if let Some(pre_test) = &pre_test {
                        if pre_test.is_callable() {
                            let pre_test_result = if pass_self_to_pre_test {
                                self.run_instance_function(
                                    self_arg.clone(),
                                    pre_test.clone(),
                                    CallArgs::None,
                                )
                            } else {
                                self.run_function(pre_test.clone(), CallArgs::None)
                            };

                            if let Err(error) = pre_test_result {
                                return make_test_error(error, "Error while preparing to run test");
                            }
                        }
                    }

                    let pass_self_to_test = match &test {
                        Function(f) => f.arg_count == 1,
                        _ => false,
                    };

                    let test_result = if pass_self_to_test {
                        self.run_instance_function(self_arg.clone(), test, CallArgs::None)
                    } else {
                        self.run_function(test, CallArgs::None)
                    };

                    if let Err(error) = test_result {
                        return make_test_error(error, "Error while running test");
                    }

                    if let Some(post_test) = &post_test {
                        if post_test.is_callable() {
                            let post_test_result = if pass_self_to_post_test {
                                self.run_instance_function(
                                    self_arg.clone(),
                                    post_test.clone(),
                                    CallArgs::None,
                                )
                            } else {
                                self.run_function(post_test.clone(), CallArgs::None)
                            };

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

    fn execute_instructions(&mut self) -> RuntimeResult {
        let mut result = Value::Null;

        self.instruction_ip = self.ip();

        while let Some(instruction) = self.reader.next() {
            match self.execute_instruction(instruction) {
                Ok(ControlFlow::Continue) => {}
                Ok(ControlFlow::Return(value)) => {
                    result = value;
                    break;
                }
                Ok(ControlFlow::Yield(value)) => {
                    result = value;
                    break;
                }
                Err(mut error) => {
                    let mut recover_register_and_ip = None;

                    error.extend_trace(self.chunk(), self.instruction_ip);

                    while let Some(frame) = self.call_stack.last() {
                        if let Some((error_register, catch_ip)) = frame.catch_stack.last() {
                            recover_register_and_ip = Some((*error_register, *catch_ip));
                            break;
                        } else {
                            if frame.execution_barrier {
                                return Err(error);
                            }

                            self.pop_frame(Value::Null)?;

                            if !self.call_stack.is_empty() {
                                error.extend_trace(self.chunk(), self.instruction_ip);
                            }
                        }
                    }

                    if let Some((register, ip)) = recover_register_and_ip {
                        let catch_value = match error.error {
                            RuntimeErrorType::KotoError { thrown_value, .. } => thrown_value,
                            _ => Value::Str(error.to_string().into()),
                        };
                        self.set_register(register, catch_value);
                        self.set_ip(ip);
                    } else {
                        return Err(error);
                    }
                }
            }

            self.instruction_ip = self.ip();
        }

        Ok(result)
    }

    fn execute_instruction(
        &mut self,
        instruction: Instruction,
    ) -> Result<ControlFlow, RuntimeError> {
        use Value::*;

        let mut control_flow = ControlFlow::Continue;

        match instruction {
            Instruction::Error { message } => runtime_error!(message),
            Instruction::Copy { target, source } => {
                self.set_register(target, self.clone_register(source));
                Ok(())
            }
            Instruction::SetNull { register } => {
                self.set_register(register, Null);
                Ok(())
            }
            Instruction::SetBool { register, value } => {
                self.set_register(register, Bool(value));
                Ok(())
            }
            Instruction::SetNumber { register, value } => {
                self.set_register(register, Number(value.into()));
                Ok(())
            }
            Instruction::LoadFloat { register, constant } => {
                let n = self.reader.chunk.constants.get_f64(constant);
                self.set_register(register, Number(n.into()));
                Ok(())
            }
            Instruction::LoadInt { register, constant } => {
                let n = self.reader.chunk.constants.get_i64(constant);
                self.set_register(register, Number(n.into()));
                Ok(())
            }
            Instruction::LoadString { register, constant } => {
                let string = self.value_string_from_constant(constant);
                self.set_register(register, Str(string));
                Ok(())
            }
            Instruction::LoadNonLocal { register, constant } => {
                self.run_load_non_local(register, constant)
            }
            Instruction::ValueExport { name, value } => {
                self.run_value_export(name, value);
                Ok(())
            }
            Instruction::Import { register } => self.run_import(register),
            Instruction::MakeTempTuple {
                register,
                start,
                count,
            } => {
                self.set_register(register, TemporaryTuple(RegisterSlice { start, count }));
                Ok(())
            }
            Instruction::TempTupleToTuple { register, source } => {
                self.run_temp_tuple_to_tuple(register, source)
            }
            Instruction::MakeMap {
                register,
                size_hint,
            } => {
                self.set_register(register, Map(ValueMap::with_capacity(size_hint)));
                Ok(())
            }
            Instruction::SequenceStart {
                register,
                size_hint,
            } => {
                self.set_register(register, SequenceBuilder(Vec::with_capacity(size_hint)));
                Ok(())
            }
            Instruction::SequencePush { sequence, value } => {
                self.run_sequence_push(sequence, value)
            }
            Instruction::SequencePushN {
                sequence,
                start,
                count,
            } => {
                for value_register in start..(start + count) {
                    self.run_sequence_push(sequence, value_register)?;
                }
                Ok(())
            }
            Instruction::SequenceToList { sequence } => self.run_sequence_to_list(sequence),
            Instruction::SequenceToTuple { sequence } => self.run_sequence_to_tuple(sequence),
            Instruction::StringStart {
                register,
                size_hint,
            } => {
                self.set_register(register, StringBuilder(String::with_capacity(size_hint)));
                Ok(())
            }
            Instruction::StringPush { register, value } => self.run_string_push(register, value),
            Instruction::StringFinish { register } => self.run_string_finish(register),
            Instruction::Range {
                register,
                start,
                end,
            } => self.run_make_range(register, Some(start), Some(end), false),
            Instruction::RangeInclusive {
                register,
                start,
                end,
            } => self.run_make_range(register, Some(start), Some(end), true),
            Instruction::RangeTo { register, end } => {
                self.run_make_range(register, None, Some(end), false)
            }
            Instruction::RangeToInclusive { register, end } => {
                self.run_make_range(register, None, Some(end), true)
            }
            Instruction::RangeFrom { register, start } => {
                self.run_make_range(register, Some(start), None, false)
            }
            Instruction::RangeFull { register } => self.run_make_range(register, None, None, false),
            Instruction::MakeIterator { register, iterable } => {
                self.run_make_iterator(register, iterable)
            }
            Instruction::SimpleFunction {
                register,
                arg_count,
                size,
            } => {
                let result = Value::SimpleFunction(SimpleFunctionInfo {
                    chunk: self.chunk(),
                    ip: self.ip(),
                    arg_count,
                });
                self.set_register(register, result);
                self.jump_ip(size);
                Ok(())
            }
            Instruction::Function { .. } => {
                self.run_make_function(instruction);
                Ok(())
            }
            Instruction::Capture {
                function,
                target,
                source,
            } => self.run_capture_value(function, target, source),
            Instruction::Negate { register, value } => self.run_negate(register, value),
            Instruction::Not { register, value } => self.run_not(register, value),
            Instruction::Add { register, lhs, rhs } => self.run_add(register, lhs, rhs),
            Instruction::Subtract { register, lhs, rhs } => self.run_subtract(register, lhs, rhs),
            Instruction::Multiply { register, lhs, rhs } => self.run_multiply(register, lhs, rhs),
            Instruction::Divide { register, lhs, rhs } => self.run_divide(register, lhs, rhs),
            Instruction::Remainder { register, lhs, rhs } => self.run_remainder(register, lhs, rhs),
            Instruction::Less { register, lhs, rhs } => self.run_less(register, lhs, rhs),
            Instruction::LessOrEqual { register, lhs, rhs } => {
                self.run_less_or_equal(register, lhs, rhs)
            }
            Instruction::Greater { register, lhs, rhs } => self.run_greater(register, lhs, rhs),
            Instruction::GreaterOrEqual { register, lhs, rhs } => {
                self.run_greater_or_equal(register, lhs, rhs)
            }
            Instruction::Equal { register, lhs, rhs } => self.run_equal(register, lhs, rhs),
            Instruction::NotEqual { register, lhs, rhs } => self.run_not_equal(register, lhs, rhs),
            Instruction::Jump { offset } => {
                self.jump_ip(offset);
                Ok(())
            }
            Instruction::JumpBack { offset } => {
                self.jump_ip_back(offset);
                Ok(())
            }
            Instruction::JumpIfTrue { register, offset } => self.run_jump_if_true(register, offset),
            Instruction::JumpIfFalse { register, offset } => {
                self.run_jump_if_false(register, offset)
            }
            Instruction::Call {
                result,
                function,
                frame_base,
                arg_count,
            } => self.call_callable(
                result,
                self.clone_register(function),
                frame_base,
                arg_count,
                None,
                None,
            ),
            Instruction::CallInstance {
                result,
                function,
                frame_base,
                arg_count,
                instance,
            } => self.call_callable(
                result,
                self.clone_register(function),
                frame_base,
                arg_count,
                Some(instance),
                None,
            ),
            Instruction::Return { register } => {
                if let Some(return_value) = self.pop_frame(self.clone_register(register))? {
                    // If pop_frame returns a new return_value, then execution should stop.
                    control_flow = ControlFlow::Return(return_value);
                }
                Ok(())
            }
            Instruction::Yield { register } => {
                control_flow = ControlFlow::Yield(self.clone_register(register));
                Ok(())
            }
            Instruction::Throw { register } => {
                let thrown_value = self.clone_register(register);

                let display_op = MetaKey::UnaryOp(UnaryOp::Display);
                use RuntimeErrorType::KotoError;
                match &thrown_value {
                    Str(_) => Err(RuntimeError::new(KotoError {
                        thrown_value,
                        vm: None,
                    })),
                    Map(m) if m.contains_meta_key(&display_op) => Err(
                        RuntimeError::from_koto_value(thrown_value, self.spawn_shared_vm()),
                    ),
                    other => {
                        runtime_error!(
                            "throw: expected string or map with @display function, found '{}'",
                            other.type_as_string()
                        )
                    }
                }
            }
            Instruction::Size { register, value } => {
                self.run_size(register, value);
                Ok(())
            }
            Instruction::IsTuple { register, value } => {
                let result = matches!(self.get_register(value), Tuple(_));
                self.set_register(register, Bool(result));
                Ok(())
            }
            Instruction::IsList { register, value } => {
                let result = matches!(self.get_register(value), List(_));
                self.set_register(register, Bool(result));
                Ok(())
            }
            Instruction::IterNext {
                register,
                iterator,
                jump_offset,
            } => self.run_iterator_next(Some(register), iterator, jump_offset, false),
            Instruction::IterNextTemp {
                register,
                iterator,
                jump_offset,
            } => self.run_iterator_next(Some(register), iterator, jump_offset, true),
            Instruction::IterNextQuiet {
                iterator,
                jump_offset,
            } => self.run_iterator_next(None, iterator, jump_offset, false),
            Instruction::TempIndex {
                register,
                value,
                index,
            } => self.run_temp_index(register, value, index),
            Instruction::SliceFrom {
                register,
                value,
                index,
            } => self.run_slice(register, value, index, false),
            Instruction::SliceTo {
                register,
                value,
                index,
            } => self.run_slice(register, value, index, true),
            Instruction::Index {
                register,
                value,
                index,
            } => self.run_index(register, value, index),
            Instruction::SetIndex {
                register,
                index,
                value,
            } => self.run_set_index(register, index, value),
            Instruction::MapInsert {
                register,
                key,
                value,
            } => self.run_map_insert(register, key, value),
            Instruction::MetaInsert {
                register,
                value,
                id,
            } => self.run_meta_insert(register, value, id),
            Instruction::MetaInsertNamed {
                register,
                value,
                id,
                name,
            } => self.run_meta_insert_named(register, value, id, name),
            Instruction::MetaExport { value, id } => self.run_meta_export(value, id),
            Instruction::MetaExportNamed { id, name, value } => {
                self.run_meta_export_named(id, name, value)
            }
            Instruction::Access {
                register,
                value,
                key,
            } => self.run_access(register, value, self.value_string_from_constant(key)),
            Instruction::AccessString {
                register,
                value,
                key,
            } => {
                let key_string = match self.clone_register(key) {
                    Str(s) => s,
                    other => return unexpected_type_error("String", &other),
                };
                self.run_access(register, value, key_string)
            }
            Instruction::TryStart {
                arg_register,
                catch_offset,
            } => {
                let catch_ip = self.ip() + catch_offset;
                self.frame_mut().catch_stack.push((arg_register, catch_ip));
                Ok(())
            }
            Instruction::TryEnd => {
                self.frame_mut().catch_stack.pop();
                Ok(())
            }
            Instruction::Debug { register, constant } => self.run_debug(register, constant),
            Instruction::CheckType { register, type_id } => self.run_check_type(register, type_id),
            Instruction::CheckSizeEqual { register, size } => {
                self.run_check_size_equal(register, size)
            }
            Instruction::CheckSizeMin { register, size } => self.run_check_size_min(register, size),
        }?;

        Ok(control_flow)
    }

    fn run_load_non_local(
        &mut self,
        register: u8,
        constant_index: ConstantIndex,
    ) -> InstructionResult {
        let name = self.get_constant_str(constant_index);

        let non_local = self
            .exports
            .data()
            .get_with_string(name)
            .cloned()
            .or_else(|| self.context.prelude.data().get_with_string(name).cloned());

        if let Some(non_local) = non_local {
            self.set_register(register, non_local);
            Ok(())
        } else {
            runtime_error!("'{name}' not found")
        }
    }

    fn run_value_export(&mut self, name_register: u8, value_register: u8) {
        let name = ValueKey::from(self.clone_register(name_register));
        let value = self.clone_register(value_register);
        self.exports.data_mut().insert(name, value);
    }

    fn run_temp_tuple_to_tuple(&mut self, register: u8, source_register: u8) -> InstructionResult {
        match self.clone_register(source_register) {
            Value::TemporaryTuple(temp_registers) => {
                let tuple = ValueTuple::from(
                    self.register_slice(temp_registers.start, temp_registers.count),
                );
                self.set_register(register, Value::Tuple(tuple));
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
    ) -> InstructionResult {
        use Value::{IndexRange, Number, Range};

        let start = start_register.map(|register| self.get_register(register));
        let end = end_register.map(|register| self.get_register(register));

        let range = match (start, end) {
            (Some(Number(start)), Some(Number(end))) => {
                let istart = isize::from(start);
                let iend = isize::from(end);

                let (start, end) = if inclusive {
                    if istart <= iend {
                        (istart, iend + 1)
                    } else {
                        (istart, iend - 1)
                    }
                } else {
                    (istart, iend)
                };

                Range(IntRange { start, end })
            }
            (None, Some(Number(end))) => {
                if *end < 0.0 {
                    return runtime_error!("RangeTo: negative numbers not allowed, found '{end}'");
                }
                let end = if inclusive {
                    usize::from(end) + 1
                } else {
                    usize::from(end)
                };
                IndexRange(value::IndexRange {
                    start: 0,
                    end: Some(end),
                })
            }
            (Some(Number(start)), None) => {
                if *start < 0.0 {
                    return runtime_error!(
                        "RangeFrom: negative numbers not allowed, found '{start}'"
                    );
                }
                IndexRange(value::IndexRange {
                    start: usize::from(start),
                    end: None,
                })
            }
            (None, None) => {
                // RangeFull
                IndexRange(value::IndexRange {
                    start: 0,
                    end: None,
                })
            }
            (Some(Number(_)), Some(unexpected)) | (None, Some(unexpected)) => {
                return unexpected_type_error("Number for range end", unexpected);
            }
            (Some(unexpected), _) => {
                return unexpected_type_error("Number for range start", unexpected);
            }
        };

        self.set_register(register, range);
        Ok(())
    }

    // Runs the MakeIterator instruction
    //
    // Distinct from the public `make_iterator` function, which will defer to this function when
    // the input value is a map that overloads @iterator.
    fn run_make_iterator(&mut self, result: u8, iterable_register: u8) -> InstructionResult {
        use Value::*;

        let iterable = self.clone_register(iterable_register);

        if matches!(iterable, Iterator(_)) {
            self.set_register(result, iterable);
        } else {
            let iterator = match iterable {
                Range(int_range) => ValueIterator::with_range(int_range),
                List(list) => ValueIterator::with_list(list),
                Tuple(tuple) => ValueIterator::with_tuple(tuple),
                Str(s) => ValueIterator::with_string(s),
                Num2(n) => ValueIterator::with_num2(n),
                Num4(n) => ValueIterator::with_num4(n),
                Map(map) if map.contains_meta_key(&MetaKey::UnaryOp(UnaryOp::Iterator)) => {
                    let op = map
                        .get_meta_value(&MetaKey::UnaryOp(UnaryOp::Iterator))
                        .unwrap();
                    return self.call_overloaded_unary_op(result, iterable_register, op);
                }
                Map(map) => ValueIterator::with_map(map),
                unexpected => {
                    return unexpected_type_error("Iterable while making iterator", &unexpected);
                }
            };

            self.set_register(result, iterator.into());
        }

        Ok(())
    }

    fn run_iterator_next(
        &mut self,
        result_register: Option<u8>,
        iterator: u8,
        jump_offset: usize,
        output_is_temporary: bool,
    ) -> InstructionResult {
        use Value::{Iterator, TemporaryTuple, Tuple};

        let result = match self.get_register_mut(iterator) {
            Iterator(iterator) => iterator.next(),
            unexpected => return unexpected_type_error("Iterator", unexpected),
        };

        match (result, result_register) {
            (Some(ValueIteratorOutput::Value(value)), Some(register)) => {
                self.set_register(register, value)
            }
            (Some(ValueIteratorOutput::ValuePair(first, second)), Some(register)) => {
                if output_is_temporary {
                    self.set_register(
                        register,
                        TemporaryTuple(RegisterSlice {
                            start: register + 1,
                            count: 2,
                        }),
                    );
                    self.set_register(register + 1, first);
                    self.set_register(register + 2, second);
                } else {
                    self.set_register(register, Tuple(vec![first, second].into()));
                }
            }
            (Some(ValueIteratorOutput::Error(error)), _) => {
                return runtime_error!(error.to_string())
            }
            (Some(_), None) => {
                // No result register, so the output can be discarded
            }
            (None, _) => self.jump_ip(jump_offset),
        };

        Ok(())
    }

    fn run_temp_index(&mut self, register: u8, value: u8, index: i8) -> InstructionResult {
        use Value::*;

        let result = match self.get_register(value) {
            List(list) => {
                let index = signed_index_to_unsigned(index, list.data().len());
                list.data().get(index).cloned().unwrap_or(Null)
            }
            Tuple(tuple) => {
                let index = signed_index_to_unsigned(index, tuple.data().len());
                tuple.data().get(index).cloned().unwrap_or(Null)
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
            Num2(n) => {
                let index = signed_index_to_unsigned(index, 2);
                if index < 2 {
                    Number(n[index].into())
                } else {
                    Null
                }
            }
            Num4(n) => {
                let index = signed_index_to_unsigned(index, 4);
                if index < 4 {
                    Number(n[index].into())
                } else {
                    Null
                }
            }
            unexpected => return unexpected_type_error("indexable value", unexpected),
        };

        self.set_register(register, result);

        Ok(())
    }

    fn run_slice(
        &mut self,
        register: u8,
        value: u8,
        index: i8,
        is_slice_to: bool,
    ) -> InstructionResult {
        use Value::*;

        let result = match self.get_register(value) {
            List(list) => {
                let index = signed_index_to_unsigned(index, list.data().len());
                if is_slice_to {
                    list.data()
                        .get(..index)
                        .map_or(Null, |entries| List(ValueList::from_slice(entries)))
                } else {
                    list.data()
                        .get(index..)
                        .map_or(Null, |entries| List(ValueList::from_slice(entries)))
                }
            }
            Tuple(tuple) => {
                let index = signed_index_to_unsigned(index, tuple.data().len());
                if is_slice_to {
                    tuple
                        .data()
                        .get(..index)
                        .map_or(Null, |entries| Tuple(entries.into()))
                } else {
                    tuple
                        .data()
                        .get(index..)
                        .map_or(Null, |entries| Tuple(entries.into()))
                }
            }
            unexpected => return unexpected_type_error("List or Tuple", unexpected),
        };

        self.set_register(register, result);

        Ok(())
    }

    fn run_make_function(&mut self, function_instruction: Instruction) {
        use Value::*;

        match function_instruction {
            Instruction::Function {
                register,
                arg_count,
                capture_count,
                instance_function,
                variadic,
                generator,
                arg_is_unpacked_tuple,
                size,
            } => {
                // Initialize the function's captures with Null
                let captures = if capture_count > 0 {
                    let mut captures = ValueVec::new();
                    captures.resize(capture_count as usize, Null);
                    Some(ValueList::with_data(captures))
                } else {
                    None
                };

                let function = FunctionInfo {
                    chunk: self.chunk(),
                    ip: self.ip(),
                    arg_count,
                    instance_function,
                    variadic,
                    captures,
                    arg_is_unpacked_tuple,
                };

                let value = if generator {
                    Generator(function)
                } else {
                    Function(function)
                };

                self.jump_ip(size);
                self.set_register(register, value);
            }
            _ => unreachable!(),
        }
    }

    fn run_capture_value(
        &mut self,
        function: u8,
        capture_index: u8,
        value: u8,
    ) -> InstructionResult {
        if let Some(function) = self.get_register_safe(function) {
            let capture_list = match function {
                Value::Function(f) => &f.captures,
                Value::Generator(g) => &g.captures,
                unexpected => return unexpected_type_error("Function", unexpected),
            };

            match capture_list {
                Some(capture_list) => {
                    capture_list.data_mut()[capture_index as usize] = self.clone_register(value);
                    Ok(())
                }
                None => runtime_error!("Missing capture list for function"),
            }
        } else {
            // e.g. x = (1..10).find |n| n == x
            // The function was temporary and has been removed from the value stack,
            // but the capture of `x` is still attempted. It would be cleaner for the compiler to
            // detect this case but for now a runtime error will have to do.
            runtime_error!("Attempting to capture a reserved value in a temporary function")
        }
    }

    fn run_negate(&mut self, result: u8, value: u8) -> InstructionResult {
        use {UnaryOp::Negate, Value::*};

        let result_value = match &self.get_register(value) {
            Number(n) => Number(-n),
            Num2(v) => Num2(-v),
            Num4(v) => Num4(-v),
            Map(map) if map.contains_meta_key(&MetaKey::UnaryOp(Negate)) => {
                let op = map.get_meta_value(&MetaKey::UnaryOp(Negate)).unwrap();
                return self.call_overloaded_unary_op(result, value, op);
            }
            ExternalValue(v) if v.contains_meta_key(&MetaKey::UnaryOp(Negate)) => {
                let op = v.get_meta_value(&MetaKey::UnaryOp(Negate)).unwrap();
                return self.call_overloaded_unary_op(result, value, op);
            }
            unexpected => return unexpected_type_error("negatable value", unexpected),
        };
        self.set_register(result, result_value);

        Ok(())
    }

    fn run_not(&mut self, result: u8, value: u8) -> InstructionResult {
        use {UnaryOp::Not, Value::*};

        let result_value = match &self.get_register(value) {
            Null => Bool(true),
            Bool(b) if !b => Bool(true),
            Map(map) if map.contains_meta_key(&MetaKey::UnaryOp(Not)) => {
                let op = map.get_meta_value(&MetaKey::UnaryOp(Not)).unwrap();
                return self.call_overloaded_unary_op(result, value, op);
            }
            ExternalValue(v) if v.contains_meta_key(&MetaKey::UnaryOp(Not)) => {
                let op = v.get_meta_value(&MetaKey::UnaryOp(Not)).unwrap();
                return self.call_overloaded_unary_op(result, value, op);
            }
            _ => Bool(false), // All other values coerce to true, so return false
        };
        self.set_register(result, result_value);

        Ok(())
    }

    fn run_display(&mut self, result: u8, value: u8) -> InstructionResult {
        use {UnaryOp::Display, Value::*};

        let result_value = match &self.get_register(value) {
            Map(map) if map.contains_meta_key(&MetaKey::UnaryOp(Display)) => {
                let op = map.get_meta_value(&MetaKey::UnaryOp(Display)).unwrap();
                return self.call_overloaded_unary_op(result, value, op);
            }
            ExternalValue(v) if v.contains_meta_key(&MetaKey::UnaryOp(Display)) => {
                let op = v.get_meta_value(&MetaKey::UnaryOp(Display)).unwrap();
                return self.call_overloaded_unary_op(result, value, op);
            }
            other => Str(other.to_string().into()),
        };
        self.set_register(result, result_value);

        Ok(())
    }

    fn run_add(&mut self, result: u8, lhs: u8, rhs: u8) -> InstructionResult {
        use {BinaryOp::Add, Value::*};

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result_value = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Number(a + b),
            (Number(a), Num2(b)) => Num2(a + b),
            (Num2(a), Num2(b)) => Num2(a + b),
            (Num2(a), Number(b)) => Num2(a + b),
            (Number(a), Num4(b)) => Num4(a + b),
            (Num4(a), Num4(b)) => Num4(a + b),
            (Num4(a), Number(b)) => Num4(a + b),
            (Str(a), Str(b)) => {
                let result = a.to_string() + b.as_ref();
                Str(result.into())
            }
            (Map(map), _) => {
                call_binary_op_or_else!(self, result, lhs, rhs_value, map, Add, {
                    return self.binary_op_error(lhs_value, rhs_value, "+");
                })
            }
            (ExternalValue(ev), _) => {
                call_binary_op_or_else!(self, result, lhs, rhs_value, ev, Add, {
                    return self.binary_op_error(lhs_value, rhs_value, "+");
                })
            }
            _ => return self.binary_op_error(lhs_value, rhs_value, "+"),
        };

        self.set_register(result, result_value);
        Ok(())
    }

    fn run_subtract(&mut self, result: u8, lhs: u8, rhs: u8) -> InstructionResult {
        use {BinaryOp::Subtract, Value::*};

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result_value = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Number(a - b),
            (Number(a), Num2(b)) => Num2(a - b),
            (Num2(a), Num2(b)) => Num2(a - b),
            (Num2(a), Number(b)) => Num2(a - b),
            (Number(a), Num4(b)) => Num4(a - b),
            (Num4(a), Num4(b)) => Num4(a - b),
            (Num4(a), Number(b)) => Num4(a - b),
            (Map(map), _) => {
                call_binary_op_or_else!(self, result, lhs, rhs_value, map, Subtract, {
                    return self.binary_op_error(lhs_value, rhs_value, "-");
                })
            }
            (ExternalValue(ev), _) => {
                call_binary_op_or_else!(self, result, lhs, rhs_value, ev, Subtract, {
                    return self.binary_op_error(lhs_value, rhs_value, "-");
                })
            }
            _ => return self.binary_op_error(lhs_value, rhs_value, "-"),
        };

        self.set_register(result, result_value);
        Ok(())
    }

    fn run_multiply(&mut self, result: u8, lhs: u8, rhs: u8) -> InstructionResult {
        use {BinaryOp::Multiply, Value::*};

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);

        let result_value = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Number(a * b),
            (Number(a), Num2(b)) => Num2(a * b),
            (Num2(a), Num2(b)) => Num2(a * b),
            (Num2(a), Number(b)) => Num2(a * b),
            (Number(a), Num4(b)) => Num4(a * b),
            (Num4(a), Num4(b)) => Num4(a * b),
            (Num4(a), Number(b)) => Num4(a * b),
            (Map(map), _) => {
                call_binary_op_or_else!(self, result, lhs, rhs_value, map, Multiply, {
                    return self.binary_op_error(lhs_value, rhs_value, "*");
                })
            }
            (ExternalValue(ev), _) => {
                call_binary_op_or_else!(self, result, lhs, rhs_value, ev, Multiply, {
                    return self.binary_op_error(lhs_value, rhs_value, "*");
                })
            }
            _ => return self.binary_op_error(lhs_value, rhs_value, "*"),
        };

        self.set_register(result, result_value);
        Ok(())
    }

    fn run_divide(&mut self, result: u8, lhs: u8, rhs: u8) -> InstructionResult {
        use {BinaryOp::Divide, Value::*};

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result_value = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Number(a / b),
            (Number(a), Num2(b)) => Num2(a / b),
            (Num2(a), Num2(b)) => Num2(a / b),
            (Num2(a), Number(b)) => Num2(a / b),
            (Number(a), Num4(b)) => Num4(a / b),
            (Num4(a), Num4(b)) => Num4(a / b),
            (Num4(a), Number(b)) => Num4(a / b),
            (Map(map), _) => {
                call_binary_op_or_else!(self, result, lhs, rhs_value, map, Divide, {
                    return self.binary_op_error(lhs_value, rhs_value, "/");
                })
            }
            (ExternalValue(ev), _) => {
                call_binary_op_or_else!(self, result, lhs, rhs_value, ev, Divide, {
                    return self.binary_op_error(lhs_value, rhs_value, "/");
                })
            }
            _ => return self.binary_op_error(lhs_value, rhs_value, "/"),
        };

        self.set_register(result, result_value);
        Ok(())
    }

    fn run_remainder(&mut self, result: u8, lhs: u8, rhs: u8) -> InstructionResult {
        use {BinaryOp::Remainder, Value::*};

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result_value = match (lhs_value, rhs_value) {
            (Number(_), Number(ValueNumber::I64(b))) if *b == 0 => {
                // Special case for integer remainder when the divisor is zero,
                // avoid a panic and return NaN instead.
                Number(f64::NAN.into())
            }
            (Number(a), Number(b)) => Number(a % b),
            (Number(a), Num2(b)) => Num2(a % b),
            (Num2(a), Num2(b)) => Num2(a % b),
            (Num2(a), Number(b)) => Num2(a % b),
            (Number(a), Num4(b)) => Num4(a % b),
            (Num4(a), Num4(b)) => Num4(a % b),
            (Num4(a), Number(b)) => Num4(a % b),
            (Map(map), _) => {
                call_binary_op_or_else!(self, result, lhs, rhs_value, map, Remainder, {
                    return self.binary_op_error(lhs_value, rhs_value, "%");
                })
            }
            (ExternalValue(ev), _) => {
                call_binary_op_or_else!(self, result, lhs, rhs_value, ev, Remainder, {
                    return self.binary_op_error(lhs_value, rhs_value, "%");
                })
            }
            _ => return self.binary_op_error(lhs_value, rhs_value, "%"),
        };
        self.set_register(result, result_value);

        Ok(())
    }

    fn run_less(&mut self, result: u8, lhs: u8, rhs: u8) -> InstructionResult {
        use {BinaryOp::Less, Value::*};

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result_value = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Bool(a < b),
            (Str(a), Str(b)) => Bool(a.as_str() < b.as_str()),
            (Map(map), _) => {
                call_binary_op_or_else!(self, result, lhs, rhs_value, map, Less, {
                    return self.binary_op_error(lhs_value, rhs_value, "<");
                })
            }
            (ExternalValue(ev), _) => {
                call_binary_op_or_else!(self, result, lhs, rhs_value, ev, Less, {
                    return self.binary_op_error(lhs_value, rhs_value, "<");
                })
            }
            _ => return self.binary_op_error(lhs_value, rhs_value, "<"),
        };
        self.set_register(result, result_value);

        Ok(())
    }

    fn run_less_or_equal(&mut self, result: u8, lhs: u8, rhs: u8) -> InstructionResult {
        use {BinaryOp::LessOrEqual, Value::*};

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result_value = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Bool(a <= b),
            (Str(a), Str(b)) => Bool(a.as_str() <= b.as_str()),
            (Map(map), _) => {
                call_binary_op_or_else!(self, result, lhs, rhs_value, map, LessOrEqual, {
                    return self.binary_op_error(lhs_value, rhs_value, "<=");
                })
            }
            (ExternalValue(ev), _) => {
                call_binary_op_or_else!(self, result, lhs, rhs_value, ev, LessOrEqual, {
                    return self.binary_op_error(lhs_value, rhs_value, "<=");
                })
            }
            _ => return self.binary_op_error(lhs_value, rhs_value, "<="),
        };
        self.set_register(result, result_value);

        Ok(())
    }

    fn run_greater(&mut self, result: u8, lhs: u8, rhs: u8) -> InstructionResult {
        use {BinaryOp::Greater, Value::*};

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result_value = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Bool(a > b),
            (Str(a), Str(b)) => Bool(a.as_str() > b.as_str()),
            (Map(map), _) => {
                call_binary_op_or_else!(self, result, lhs, rhs_value, map, Greater, {
                    return self.binary_op_error(lhs_value, rhs_value, ">");
                })
            }
            (ExternalValue(ev), _) => {
                call_binary_op_or_else!(self, result, lhs, rhs_value, ev, Greater, {
                    return self.binary_op_error(lhs_value, rhs_value, ">");
                })
            }
            _ => return self.binary_op_error(lhs_value, rhs_value, ">"),
        };
        self.set_register(result, result_value);

        Ok(())
    }

    fn run_greater_or_equal(&mut self, result: u8, lhs: u8, rhs: u8) -> InstructionResult {
        use {BinaryOp::GreaterOrEqual, Value::*};

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result_value = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Bool(a >= b),
            (Str(a), Str(b)) => Bool(a.as_str() >= b.as_str()),
            (Map(map), _) => {
                call_binary_op_or_else!(self, result, lhs, rhs_value, map, GreaterOrEqual, {
                    return self.binary_op_error(lhs_value, rhs_value, ">=");
                })
            }
            (ExternalValue(ev), _) => {
                call_binary_op_or_else!(self, result, lhs, rhs_value, ev, GreaterOrEqual, {
                    return self.binary_op_error(lhs_value, rhs_value, ">=");
                })
            }
            _ => return self.binary_op_error(lhs_value, rhs_value, ">="),
        };
        self.set_register(result, result_value);

        Ok(())
    }

    fn run_equal(&mut self, result: u8, lhs: u8, rhs: u8) -> InstructionResult {
        use {BinaryOp::Equal, Value::*};

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result_value = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => a == b,
            (Num2(a), Num2(b)) => a == b,
            (Num4(a), Num4(b)) => a == b,
            (Bool(a), Bool(b)) => a == b,
            (Str(a), Str(b)) => a == b,
            (Range(a), Range(b)) => a == b,
            (IndexRange(a), IndexRange(b)) => a == b,
            (Null, Null) => true,
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
                let data_a = a.data();
                let data_b = b.data();
                self.compare_value_ranges(data_a, data_b)?
            }
            (Map(map), _) => {
                call_binary_op_or_else!(self, result, lhs, rhs_value, map, Equal, {
                    if let Map(rhs_map) = rhs_value {
                        let a = map.clone();
                        let b = rhs_map.clone();
                        self.compare_value_maps(a, b)?
                    } else {
                        false
                    }
                })
            }
            (Function(a), Function(b)) => {
                if a.chunk == b.chunk && a.ip == b.ip && a.arg_count == b.arg_count {
                    match (&a.captures, &b.captures) {
                        (None, None) => true,
                        (Some(captures_a), Some(captures_b)) => {
                            let captures_a = captures_a.clone();
                            let captures_b = captures_b.clone();
                            let data_a = captures_a.data();
                            let data_b = captures_b.data();
                            self.compare_value_ranges(&data_a, &data_b)?
                        }
                        _ => false,
                    }
                } else {
                    false
                }
            }
            (SimpleFunction(a), SimpleFunction(b)) => {
                a.chunk == b.chunk && a.ip == b.ip && a.arg_count == b.arg_count
            }
            (ExternalValue(ev), _) => {
                call_binary_op_or_else!(self, result, lhs, rhs_value, ev, Equal, false)
            }
            _ => false,
        };

        self.set_register(result, result_value.into());

        Ok(())
    }

    fn run_not_equal(&mut self, result: u8, lhs: u8, rhs: u8) -> InstructionResult {
        use {BinaryOp::NotEqual, Value::*};

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result_value = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => a != b,
            (Num2(a), Num2(b)) => a != b,
            (Num4(a), Num4(b)) => a != b,
            (Bool(a), Bool(b)) => a != b,
            (Str(a), Str(b)) => a != b,
            (Range(a), Range(b)) => a != b,
            (IndexRange(a), IndexRange(b)) => a != b,
            (Null, Null) => false,
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
                let data_a = a.data();
                let data_b = b.data();
                !self.compare_value_ranges(data_a, data_b)?
            }
            (Map(map), _) => {
                call_binary_op_or_else!(self, result, lhs, rhs_value, map, NotEqual, {
                    if let Map(rhs_map) = rhs_value {
                        let a = map.clone();
                        let b = rhs_map.clone();
                        !self.compare_value_maps(a, b)?
                    } else {
                        true
                    }
                })
            }
            (Function(a), Function(b)) => {
                if a.chunk == b.chunk && a.ip == b.ip && a.arg_count == b.arg_count {
                    match (&a.captures, &b.captures) {
                        (None, None) => false,
                        (Some(captures_a), Some(captures_b)) => {
                            let captures_a = captures_a.clone();
                            let captures_b = captures_b.clone();
                            let data_a = captures_a.data();
                            let data_b = captures_b.data();
                            !self.compare_value_ranges(&data_a, &data_b)?
                        }
                        _ => true,
                    }
                } else {
                    true
                }
            }
            (ExternalValue(ev), _) => {
                call_binary_op_or_else!(self, result, lhs, rhs_value, ev, NotEqual, true)
            }
            _ => true,
        };
        self.set_register(result, result_value.into());

        Ok(())
    }

    // Called from run_equal / run_not_equal to compare the contents of lists and tuples
    fn compare_value_ranges(
        &mut self,
        range_a: &[Value],
        range_b: &[Value],
    ) -> Result<bool, RuntimeError> {
        if range_a.len() != range_b.len() {
            return Ok(false);
        }

        for (value_a, value_b) in range_a.iter().zip(range_b.iter()) {
            match self.run_binary_op(BinaryOp::Equal, value_a.clone(), value_b.clone())? {
                Value::Bool(true) => {}
                Value::Bool(false) => return Ok(false),
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
    fn compare_value_maps(
        &mut self,
        map_a: ValueMap,
        map_b: ValueMap,
    ) -> Result<bool, RuntimeError> {
        if map_a.len() != map_b.len() {
            return Ok(false);
        }

        for (key_a, value_a) in map_a.data().iter() {
            if let Some(value_b) = map_b.data().get(key_a) {
                match self.run_binary_op(BinaryOp::Equal, value_a.clone(), value_b.clone())? {
                    Value::Bool(true) => {}
                    Value::Bool(false) => return Ok(false),
                    other => {
                        return runtime_error!(
                            "Expected Bool from equality comparison, found '{}'",
                            other.type_as_string()
                        );
                    }
                }
            } else {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn call_overloaded_unary_op(
        &mut self,
        result_register: u8,
        value_register: u8,
        op: Value,
    ) -> InstructionResult {
        // Ensure that the result register is present in the stack, otherwise it might be lost after
        // the call to the op, which expects a frame base at or after the result register.
        if self.register_index(result_register) >= self.value_stack.len() {
            self.set_register(result_register, Value::Null);
        }

        // Set up the call registers at the end of the stack
        let stack_len = self.value_stack.len();
        let frame_base = (stack_len - self.register_base()) as u8;
        self.value_stack.push(Value::Null); // frame_base
        self.call_callable(
            result_register,
            op,
            frame_base,
            0, // 0 args
            Some(value_register),
            None,
        )
    }

    fn call_overloaded_binary_op(
        &mut self,
        result_register: u8,
        lhs_register: u8,
        rhs: Value,
        op: Value,
    ) -> InstructionResult {
        // Ensure that the result register is present in the stack, otherwise it might be lost after
        // the call to the op, which expects a frame base at or after the result register.
        if self.register_index(result_register) >= self.value_stack.len() {
            self.set_register(result_register, Value::Null);
        }

        // Set up the call registers at the end of the stack
        let stack_len = self.value_stack.len();
        let frame_base = (stack_len - self.register_base()) as u8;
        self.value_stack.push(Value::Null); // frame_base
        self.value_stack.push(rhs); // arg
        self.call_callable(
            result_register,
            op,
            frame_base,
            1, // 1 arg, the rhs value
            Some(lhs_register),
            None,
        )
    }

    fn run_jump_if_true(&mut self, register: u8, offset: usize) -> InstructionResult {
        match &self.get_register(register) {
            Value::Null => {}
            Value::Bool(b) if !b => {}
            _ => self.jump_ip(offset),
        }
        Ok(())
    }

    fn run_jump_if_false(&mut self, register: u8, offset: usize) -> InstructionResult {
        match &self.get_register(register) {
            Value::Null => self.jump_ip(offset),
            Value::Bool(b) if !b => self.jump_ip(offset),
            _ => {}
        }
        Ok(())
    }

    fn run_size(&mut self, register: u8, value: u8) {
        let result = self.get_register(value).size();
        self.set_register(register, Value::Number(result.into()));
    }

    fn run_import(&mut self, import_register: u8) -> InstructionResult {
        let import_name = match self.clone_register(import_register) {
            Value::Str(s) => s,
            value @ Value::Map(_) | value @ Value::ExternalValue(_) => {
                self.set_register(import_register, value);
                return Ok(());
            }
            other => {
                return unexpected_type_error("import id or string, or accessible value", &other)
            }
        };

        // Is the import in the exports?
        let maybe_in_exports = self.exports.data().get_with_string(&import_name).cloned();
        if let Some(value) = maybe_in_exports {
            self.set_register(import_register, value);
            return Ok(());
        }

        // Is the import in the prelude?
        let maybe_in_prelude = self
            .context
            .prelude
            .data()
            .get_with_string(&import_name)
            .cloned();
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
            .compile_module(&import_name, source_path)
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
                self.set_register(import_register, Value::Map(cached_exports));
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
        self.exports = ValueMap::default();

        // Execute the following steps in a closure to ensure that cleanup is performed afterwards
        let import_result = {
            || {
                self.run(compile_result.chunk.clone())?;

                if self.context.settings.run_import_tests {
                    let maybe_tests = self.exports.get_meta_value(&MetaKey::Tests);
                    match maybe_tests {
                        Some(Value::Map(tests)) => {
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
                        self.run_function(main, CallArgs::None)?;
                    }
                    Some(unexpected) => {
                        return unexpected_type_error("callable function", &unexpected)
                    }
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
            self.set_register(import_register, Value::Map(module_exports));
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
    ) -> InstructionResult {
        use Value::*;

        let indexable = self.clone_register(indexable_register);
        let index_value = self.clone_register(index_register);
        let value = self.clone_register(value_register);

        match indexable {
            List(list) => {
                let list_len = list.len();
                match index_value {
                    Number(index) => {
                        let u_index = usize::from(index);
                        if index >= 0.0 && u_index < list_len {
                            list.data_mut()[u_index] = value;
                        } else {
                            return runtime_error!("Index '{index}' not in List");
                        }
                    }
                    Range(IntRange { start, end }) => {
                        let (ustart, uend) = self.validate_int_range(start, end, Some(list_len))?;

                        let mut list_data = list.data_mut();
                        for i in ustart..uend {
                            list_data[i] = value.clone();
                        }
                    }
                    IndexRange(value::IndexRange { start, end }) => {
                        let end = end.unwrap_or(list_len);
                        self.validate_index_range(start, end, list_len)?;

                        let mut list_data = list.data_mut();
                        for i in start..end {
                            list_data[i] = value.clone();
                        }
                    }
                    unexpected => return unexpected_type_error("index", &unexpected),
                }
            }
            unexpected => return unexpected_type_error("a mutable indexable value", &unexpected),
        };

        Ok(())
    }

    fn validate_index(&self, n: ValueNumber, size: Option<usize>) -> Result<usize, RuntimeError> {
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

    fn validate_int_range(
        &self,
        start: isize,
        end: isize,
        size: Option<usize>,
    ) -> Result<(usize, usize), RuntimeError> {
        let ustart = start as usize;
        let uend = end as usize;

        if start < 0 || end < 0 {
            return runtime_error!(
                "Indexing with negative indices isn't supported, start: {start}, end: {end}"
            );
        } else if start > end {
            return runtime_error!(
                "Indexing with a descending range isn't supported, start: {start}, end: {end}"
            );
        } else if let Some(size) = size {
            if ustart > size || uend > size {
                return runtime_error!(
                    "Index out of bounds, start: {start}, end: {end}, size: {size}"
                );
            }
        }

        Ok((ustart, uend))
    }

    fn validate_index_range(&self, start: usize, end: usize, size: usize) -> InstructionResult {
        if start > end {
            runtime_error!(
                "Indexing with a descending range isn't supported, start: {start}, end: {end}"
            )
        } else if start > size || end > size {
            runtime_error!("Index out of bounds, start: {start}, end: {end}, size: {size}")
        } else {
            Ok(())
        }
    }

    fn run_index(
        &mut self,
        result_register: u8,
        value_register: u8,
        index_register: u8,
    ) -> InstructionResult {
        use {BinaryOp::Index, Value::*};

        let value = self.clone_register(value_register);
        let index = self.clone_register(index_register);

        match (&value, index) {
            (List(l), Number(n)) => {
                let index = self.validate_index(n, Some(l.len()))?;
                self.set_register(result_register, l.data()[index].clone());
            }
            (List(l), Range(IntRange { start, end })) => {
                let (start, end) = self.validate_int_range(start, end, Some(l.len()))?;
                self.set_register(
                    result_register,
                    List(ValueList::from_slice(&l.data()[start..end])),
                )
            }
            (List(l), IndexRange(value::IndexRange { start, end })) => {
                let end = end.unwrap_or_else(|| l.len());
                self.validate_index_range(start, end, l.len())?;
                self.set_register(
                    result_register,
                    List(ValueList::from_slice(&l.data()[start..end])),
                )
            }
            (Tuple(t), Number(n)) => {
                let index = self.validate_index(n, Some(t.data().len()))?;
                self.set_register(result_register, t.data()[index].clone());
            }
            (Tuple(t), Range(IntRange { start, end })) => {
                let (start, end) = self.validate_int_range(start, end, Some(t.data().len()))?;
                self.set_register(result_register, Tuple(t.data()[start..end].into()))
            }
            (Tuple(t), IndexRange(value::IndexRange { start, end })) => {
                let end = end.unwrap_or_else(|| t.data().len());
                self.validate_index_range(start, end, t.data().len())?;
                self.set_register(result_register, Tuple(t.data()[start..end].into()))
            }
            (Str(s), Number(n)) => {
                let index = self.validate_index(n, None)?;

                if let Some(result) = s.with_grapheme_indices(index, Some(index + 1)) {
                    self.set_register(result_register, Str(result));
                } else {
                    return runtime_error!(
                        "Index out of bounds - index: {index}, size: {}",
                        s.grapheme_count()
                    );
                }
            }
            (Str(s), Range(IntRange { start, end })) => {
                let (start, end) = self.validate_int_range(start, end, None)?;

                if let Some(result) = s.with_grapheme_indices(start, Some(end)) {
                    self.set_register(result_register, Str(result));
                } else {
                    return runtime_error!(
                        "Index out of bounds for string - start: {start}, end {end}, size: {}",
                        s.grapheme_count()
                    );
                }
            }
            (Str(s), IndexRange(value::IndexRange { start, end })) => {
                if let Some(end_unwrapped) = end {
                    self.validate_int_range(start as isize, end_unwrapped as isize, None)?;
                }

                if let Some(result) = s.with_grapheme_indices(start, end) {
                    self.set_register(result_register, Str(result));
                } else {
                    return runtime_error!(
                        "Index out of bounds for string - start: {start}{}, size: {}",
                        if let Some(end_unwrapped) = end {
                            format!(", {end_unwrapped}")
                        } else {
                            "".to_string()
                        },
                        s.grapheme_count()
                    );
                }
            }
            (Num2(n), Number(i)) => {
                let i = usize::from(i);
                match i {
                    0 | 1 => self.set_register(result_register, Number(n[i].into())),
                    other => return runtime_error!("Index out of bounds for Num2, {other}"),
                }
            }
            (Num4(n), Number(i)) => {
                let i = usize::from(i);
                match i {
                    0 | 1 | 2 | 3 => self.set_register(result_register, Number(n[i].into())),
                    other => return runtime_error!("Index out of bounds for Num4, {other}"),
                }
            }
            (Map(m), index) => {
                call_binary_op_or_else!(self, result_register, value_register, index, m, Index, {
                    return runtime_error!("Unable to index {}", value.type_as_string());
                });
            }
            (ExternalValue(ev), index) => {
                call_binary_op_or_else!(self, result_register, value_register, index, ev, Index, {
                    return runtime_error!("Unable to index {}", value.type_as_string());
                });
            }
            (unexpected_value, unexpected_index) => {
                return runtime_error!(
                    "Unable to index '{}' with '{}'",
                    unexpected_value.type_as_string(),
                    unexpected_index.type_as_string(),
                )
            }
        };

        Ok(())
    }

    fn run_map_insert(
        &mut self,
        map_register: u8,
        key_register: u8,
        value_register: u8,
    ) -> InstructionResult {
        let key = self.clone_register(key_register);
        let value = self.clone_register(value_register);

        match self.get_register_mut(map_register) {
            Value::Map(map) => {
                map.data_mut().insert(key.into(), value);
                Ok(())
            }
            unexpected => unexpected_type_error("Map", unexpected),
        }
    }

    fn run_meta_insert(
        &mut self,
        map_register: u8,
        value: u8,
        meta_id: MetaKeyId,
    ) -> InstructionResult {
        let value = self.clone_register(value);
        let meta_key = match meta_id_to_key(meta_id, None) {
            Ok(meta_key) => meta_key,
            Err(error) => return runtime_error!("Error while preparing meta key: {error}"),
        };

        match self.get_register_mut(map_register) {
            Value::Map(map) => {
                map.insert_meta(meta_key, value);
                Ok(())
            }
            unexpected => unexpected_type_error("Map", unexpected),
        }
    }

    fn run_meta_insert_named(
        &mut self,
        map_register: u8,
        value_register: u8,
        meta_id: MetaKeyId,
        name_register: u8,
    ) -> InstructionResult {
        let value = self.clone_register(value_register);

        let meta_key = match self.clone_register(name_register) {
            Value::Str(name) => match meta_id_to_key(meta_id, Some(name)) {
                Ok(key) => key,
                Err(error) => return runtime_error!("Error while preparing meta key: {error}"),
            },
            other => return unexpected_type_error("String", &other),
        };

        match self.get_register_mut(map_register) {
            Value::Map(map) => {
                map.insert_meta(meta_key, value);
                Ok(())
            }
            unexpected => unexpected_type_error("Map", unexpected),
        }
    }

    fn run_meta_export(&mut self, value: u8, meta_id: MetaKeyId) -> InstructionResult {
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
    ) -> InstructionResult {
        let value = self.clone_register(value_register);

        let meta_key = match self.clone_register(name_register) {
            Value::Str(name) => match meta_id_to_key(meta_id, Some(name)) {
                Ok(key) => key,
                Err(error) => return runtime_error!("Error while preparing meta key: {error}"),
            },
            other => return unexpected_type_error("String", &other),
        };

        self.exports.insert_meta(meta_key, value);
        Ok(())
    }

    fn run_access(
        &mut self,
        result_register: u8,
        value_register: u8,
        key_string: ValueString,
    ) -> InstructionResult {
        use Value::*;

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
            Map(map) => match map.data().get(&key) {
                Some(value) => {
                    self.set_register(result_register, value.clone());
                }
                None => match map.get_meta_value(&MetaKey::Named(key_string)) {
                    Some(value) => self.set_register(result_register, value),
                    None => core_op!(map, true),
                },
            },
            List(_) => core_op!(list, true),
            Num2(_) => core_op!(num2, true),
            Num4(_) => core_op!(num4, true),
            Number(_) => core_op!(number, false),
            Range(_) => core_op!(range, true),
            Str(_) => core_op!(string, true),
            Tuple(_) => core_op!(tuple, true),
            Iterator(_) => core_op!(iterator, false),
            ExternalValue(ev) => match ev.get_meta_value(&MetaKey::Named(key_string.clone())) {
                Some(value) => self.set_register(result_register, value),
                None => {
                    return runtime_error!(
                        "'{key_string}' not found in '{}'",
                        accessed_value.type_as_string()
                    );
                }
            },
            unexpected => {
                return unexpected_type_error("Value that supports '.' access", unexpected)
            }
        }

        Ok(())
    }

    fn get_core_op(
        &self,
        key: &ValueKey,
        module: &ValueMap,
        iterator_fallback: bool,
        module_name: &str,
    ) -> RuntimeResult {
        use Value::*;

        let maybe_op = match module.data().get(key).cloned() {
            None if iterator_fallback => self.context.core_lib.iterator.data().get(key).cloned(),
            maybe_op => maybe_op,
        };

        let result = match maybe_op {
            Some(op) => match op {
                // Core module functions accessed in a lookup need to be invoked as
                // if they were declared as instance functions, so that they can receive
                // the parent instance as a self argument.
                // e.g.
                // A function in string that can be called as:
                //   string.lines my_string
                // can also be called as:
                //   my_string.lines()
                // ...where it needs to behave as an instance function.
                // There's surely a cleaner way to achieve this, but this will do for now...
                ExternalFunction(f) => {
                    let f_as_instance_function = external::ExternalFunction {
                        is_instance_function: true,
                        ..f
                    };
                    ExternalFunction(f_as_instance_function)
                }
                SimpleFunction(f) => {
                    let f_as_instance_function = FunctionInfo {
                        chunk: f.chunk,
                        ip: f.ip,
                        arg_count: f.arg_count,
                        instance_function: true,
                        variadic: false,
                        captures: None,
                        arg_is_unpacked_tuple: false,
                    };
                    Function(f_as_instance_function)
                }
                Function(f) => {
                    let f_as_instance_function = FunctionInfo {
                        instance_function: true,
                        ..f
                    };
                    Function(f_as_instance_function)
                }
                Generator(f) => {
                    let f_as_instance_function = FunctionInfo {
                        instance_function: true,
                        ..f
                    };
                    Generator(f_as_instance_function)
                }
                other => other,
            },
            None => {
                use std::ops::Deref;
                return runtime_error!("'{}' not found in '{module_name}'", key.deref());
            }
        };

        Ok(result)
    }

    fn call_external_function(
        &mut self,
        result_register: u8,
        external_function: ExternalFunction,
        frame_base: u8,
        call_arg_count: u8,
        instance_register: Option<u8>,
    ) -> InstructionResult {
        let function = external_function.function.as_ref();

        let mut call_arg_count = call_arg_count;

        let adjusted_frame_base = if external_function.is_instance_function {
            if let Some(instance_register) = instance_register {
                if instance_register != frame_base {
                    let instance = self.clone_register(instance_register);
                    self.set_register(frame_base, instance);
                }
                call_arg_count += 1;
                frame_base
            } else {
                return runtime_error!("Expected self for external instance function");
            }
        } else {
            frame_base + 1
        };

        let result = (*function)(
            self,
            &Args {
                register: adjusted_frame_base,
                count: call_arg_count,
            },
        );

        match result {
            Ok(value) => {
                self.set_register(result_register, value);
                // External function calls don't use the push/pop frame mechanism,
                // so drop the function args here now that the call has been completed.
                self.truncate_registers(frame_base);
            }
            Err(error) => return Err(error),
        }

        Ok(())
    }

    fn call_generator(
        &mut self,
        result_register: u8,
        function: FunctionInfo,
        frame_base: u8,
        call_arg_count: u8,
        instance_register: Option<u8>,
        temp_tuple_values: Option<&[Value]>,
    ) -> InstructionResult {
        let FunctionInfo {
            chunk,
            ip: function_ip,
            arg_count: function_arg_count,
            instance_function,
            variadic,
            captures,
            arg_is_unpacked_tuple: _unused,
        } = function;

        // Spawn a VM for the generator
        let mut generator_vm = self.spawn_shared_vm();
        // Push a frame for running the generator function
        generator_vm.push_frame(
            chunk,
            function_ip,
            0, // arguments will be copied starting in register 0
            0,
        );

        let expected_arg_count = match (instance_function, variadic) {
            (true, true) => function_arg_count - 2,
            (true, false) | (false, true) => function_arg_count - 1,
            (false, false) => function_arg_count,
        };

        // Copy the instance value into the generator vm
        let arg_offset = if instance_function {
            if let Some(instance_register) = instance_register {
                let instance = self.clone_register(instance_register);
                // Place the instance in the first register of the generator vm
                generator_vm.set_register(0, instance);
                1
            } else {
                return runtime_error!("Missing instance for call to instance function");
            }
        } else {
            0
        };

        // Copy any regular (non-instance, non-variadic) arguments into the generator vm
        for (arg_index, arg) in self
            .register_slice(frame_base + 1, expected_arg_count.min(call_arg_count))
            .iter()
            .cloned()
            .enumerate()
        {
            generator_vm.set_register(arg_index as u8 + arg_offset, arg);
        }

        // Ensure that registers for missing arguments are set to Null
        if call_arg_count < expected_arg_count {
            for arg_index in call_arg_count..expected_arg_count {
                generator_vm.set_register(arg_index as u8 + arg_offset, Value::Null);
            }
        }

        // Check for variadic arguments, and validate argument count
        if variadic {
            let variadic_register = expected_arg_count + arg_offset;
            if call_arg_count >= expected_arg_count {
                // Capture the varargs into a tuple and place them in the
                // generator vm's last arg register
                let varargs_start = frame_base + 1 + expected_arg_count;
                let varargs_count = call_arg_count - expected_arg_count;
                let varargs =
                    Value::Tuple(self.register_slice(varargs_start, varargs_count).into());
                generator_vm.set_register(variadic_register, varargs);
            } else {
                generator_vm.set_register(variadic_register, Value::Null);
            }
        }
        // Place any captures in the registers following the arguments
        if let Some(captures) = captures {
            generator_vm
                .value_stack
                .extend(captures.data().iter().cloned())
        }

        // Place any temp tuple values in the registers following the args and captures
        if let Some(temp_tuple_values) = temp_tuple_values {
            generator_vm
                .value_stack
                .extend_from_slice(temp_tuple_values);
        }

        // The args have been cloned into the generator vm, so at this point they can be removed
        self.truncate_registers(frame_base);

        // Wrap the generator vm in an iterator and place it in the result register
        self.set_register(result_register, ValueIterator::with_vm(generator_vm).into());

        Ok(())
    }

    fn call_function(
        &mut self,
        result_register: u8,
        function: FunctionInfo,
        frame_base: u8,
        call_arg_count: u8,
        instance_register: Option<u8>,
        temp_tuple_values: Option<&[Value]>,
    ) -> InstructionResult {
        let FunctionInfo {
            chunk,
            ip: function_ip,
            arg_count: function_arg_count,
            instance_function,
            variadic,
            captures,
            arg_is_unpacked_tuple: _unused,
        } = function;

        let expected_arg_count = match (instance_function, variadic) {
            (true, true) => function_arg_count - 2,
            (true, false) | (false, true) => function_arg_count - 1,
            (false, false) => function_arg_count,
        };

        // Clone the instance register into the first register of the frame
        let adjusted_frame_base = if instance_function {
            if let Some(instance_register) = instance_register {
                if instance_register != frame_base {
                    let instance = self.clone_register(instance_register);
                    self.set_register(frame_base, instance);
                }
                frame_base
            } else {
                return runtime_error!("Missing instance for call to instance function");
            }
        } else {
            // If there's no self arg, then the frame's instance register is unused,
            // so the new function's frame base is offset by 1
            frame_base + 1
        };

        if variadic && call_arg_count >= expected_arg_count {
            // The last defined arg is the start of the var_args,
            // e.g. f = |x, y, z...|
            // arg index 2 is the first vararg, and where the tuple will be placed
            let arg_base = frame_base + 1;
            let varargs_start = arg_base + expected_arg_count;
            let varargs_count = call_arg_count - expected_arg_count;
            let varargs = Value::Tuple(self.register_slice(varargs_start, varargs_count).into());
            self.set_register(varargs_start, varargs);
            self.truncate_registers(varargs_start + 1);
        }

        let frame_base_index = self.register_index(adjusted_frame_base);
        if expected_arg_count > call_arg_count {
            // Ensure that temporary registers used to prepare the call args have been removed from
            // the value stack.
            let missing_args = expected_arg_count - call_arg_count;
            self.value_stack
                .truncate(frame_base_index + missing_args as usize);
        }
        // Ensure that registers have been filled with Null for any missing args.
        // If there are extra args, truncating is necessary at this point. Extra args have either
        // been bundled into a variadic Tuple or they can be ignored.
        self.value_stack
            .resize(frame_base_index + function_arg_count as usize, Value::Null);

        if let Some(captures) = captures {
            // Copy the captures list into the registers following the args
            self.value_stack.extend(captures.data().iter().cloned());
        }

        // Place any temp tuple values in the registers following the args and captures
        if let Some(temp_tuple_values) = temp_tuple_values {
            self.value_stack.extend_from_slice(temp_tuple_values);
        }

        // Set up a new frame for the called function
        self.push_frame(chunk, function_ip, adjusted_frame_base, result_register);

        Ok(())
    }

    fn call_callable(
        &mut self,
        result_register: u8,
        function: Value,
        frame_base: u8,
        call_arg_count: u8,
        instance_register: Option<u8>,
        temp_tuple_values: Option<&[Value]>,
    ) -> InstructionResult {
        use Value::*;

        match function {
            SimpleFunction(SimpleFunctionInfo {
                chunk,
                ip: function_ip,
                arg_count: function_arg_count,
            }) => {
                // The frame base is offset by one since the frame's instance register is unused.
                let frame_base = frame_base + 1;

                let frame_base_index = self.register_index(frame_base);
                // Remove any temporary registers used to prepare the call args
                self.value_stack
                    .truncate(frame_base_index + call_arg_count as usize);
                // Ensure that registers have been filled with Null for any missing args.
                // If there are extra args, truncating is OK at this point (variadic calls aren't
                // available for SimpleFunction).
                self.value_stack
                    .resize(frame_base_index + function_arg_count as usize, Value::Null);

                // Set up a new frame for the called function
                self.push_frame(chunk, function_ip, frame_base, result_register);

                Ok(())
            }
            Function(function_info) => self.call_function(
                result_register,
                function_info,
                frame_base,
                call_arg_count,
                instance_register,
                temp_tuple_values,
            ),
            Generator(function_info) => self.call_generator(
                result_register,
                function_info,
                frame_base,
                call_arg_count,
                instance_register,
                temp_tuple_values,
            ),
            ExternalFunction(external_function) => self.call_external_function(
                result_register,
                external_function,
                frame_base,
                call_arg_count,
                instance_register,
            ),
            unexpected => unexpected_type_error("callable function", &unexpected),
        }
    }

    fn run_debug(&mut self, register: u8, expression_constant: ConstantIndex) -> InstructionResult {
        let value = self.clone_register(register);
        let value_string = match self.run_unary_op(UnaryOp::Display, value)? {
            result @ Value::Str(_) => result,
            other => {
                return runtime_error!(
                    "debug: Expected string to display, found '{}'",
                    other.type_as_string()
                )
            }
        };

        let prefix = match (
            self.reader
                .chunk
                .debug_info
                .get_source_span(self.instruction_ip),
            self.reader.chunk.source_path.as_ref(),
        ) {
            (Some(span), Some(path)) => format!("[{}: {}] ", path.display(), span.start.line),
            (Some(span), None) => format!("[{}] ", span.start.line),
            (None, Some(path)) => format!("[{}: #ERR] ", path.display()),
            (None, None) => "[#ERR] ".to_string(),
        };

        let expression_string = self.get_constant_str(expression_constant);

        self.stdout()
            .write_line(&format!("{prefix}{expression_string}: {value_string}",))
    }

    fn run_check_type(&self, register: u8, type_id: TypeId) -> InstructionResult {
        let value = self.get_register(register);
        match type_id {
            TypeId::List => {
                if !matches!(value, Value::List(_)) {
                    return unexpected_type_error("List", value);
                }
            }
            TypeId::Tuple => {
                if !matches!(value, Value::Tuple(_) | Value::TemporaryTuple(_)) {
                    return unexpected_type_error("Tuple", value);
                }
            }
        }
        Ok(())
    }

    fn run_check_size_equal(&self, register: u8, expected_size: usize) -> InstructionResult {
        let value_size = self.get_register(register).size();

        if value_size == expected_size {
            Ok(())
        } else {
            runtime_error!("Container has a size of '{value_size}', expected '{expected_size}'")
        }
    }

    fn run_check_size_min(&self, register: u8, expected_size: usize) -> InstructionResult {
        let value_size = self.get_register(register).size();

        if value_size >= expected_size {
            Ok(())
        } else {
            runtime_error!(
                "Container has a size of '{value_size}', expected a minimum of '{expected_size}'"
            )
        }
    }

    fn run_sequence_push(
        &mut self,
        sequence_register: u8,
        value_register: u8,
    ) -> InstructionResult {
        let value = self.clone_register(value_register);
        match self.get_register_mut(sequence_register) {
            Value::SequenceBuilder(builder) => {
                builder.push(value);
                Ok(())
            }
            other => {
                runtime_error!(
                    "SequencePush: Expected SequenceBuilder, found '{}'",
                    other.type_as_string()
                )
            }
        }
    }

    fn run_sequence_to_list(&mut self, register: u8) -> InstructionResult {
        // Move the sequence builder out of its register to avoid cloning the Vec
        match self.remove_register(register) {
            Value::SequenceBuilder(result) => {
                let list = ValueList::with_data(ValueVec::from_vec(result));
                self.set_register(register, Value::List(list));
                Ok(())
            }
            other => unexpected_type_error("SequenceBuilder", &other),
        }
    }

    fn run_sequence_to_tuple(&mut self, register: u8) -> InstructionResult {
        // Move the sequence builder out of its register to avoid cloning the Vec
        match self.remove_register(register) {
            Value::SequenceBuilder(result) => {
                self.set_register(register, Value::Tuple(ValueTuple::from(result)));
                Ok(())
            }
            other => unexpected_type_error("SequenceBuilder", &other),
        }
    }

    fn run_string_push(&mut self, register: u8, value_register: u8) -> InstructionResult {
        let value = self.clone_register(value_register);
        let display_result = self.run_unary_op(UnaryOp::Display, value)?;

        // Add the resulting string to the string builder
        match display_result {
            Value::Str(string) => match self.get_register_mut(register) {
                Value::StringBuilder(builder) => {
                    builder.push_str(&string);
                    Ok(())
                }
                other => unexpected_type_error("StringBuilder", other),
            },
            other => unexpected_type_error("String", &other),
        }
    }

    fn run_string_finish(&mut self, register: u8) -> InstructionResult {
        // Move the string builder out of its register to avoid cloning the string data
        match self.remove_register(register) {
            Value::StringBuilder(result) => {
                // Make a ValueString out of the string builder's contents
                self.set_register(register, Value::Str(ValueString::from(result)));
                Ok(())
            }
            other => unexpected_type_error("StringBuilder", &other),
        }
    }

    pub fn chunk(&self) -> Rc<Chunk> {
        self.reader.chunk.clone()
    }

    fn set_chunk_and_ip(&mut self, chunk: Rc<Chunk>, ip: usize) {
        self.reader = InstructionReader { chunk, ip };
    }

    fn ip(&self) -> usize {
        self.reader.ip
    }

    fn set_ip(&mut self, ip: usize) {
        self.reader.ip = ip;
    }

    fn jump_ip(&mut self, offset: usize) {
        self.reader.ip += offset;
    }

    fn jump_ip_back(&mut self, offset: usize) {
        self.reader.ip -= offset;
    }

    fn frame(&self) -> &Frame {
        self.call_stack.last().expect("Empty call stack")
    }

    fn frame_mut(&mut self) -> &mut Frame {
        self.call_stack.last_mut().expect("Empty call stack")
    }

    fn push_frame(&mut self, chunk: Rc<Chunk>, ip: usize, frame_base: u8, return_register: u8) {
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

    fn pop_frame(&mut self, return_value: Value) -> Result<Option<Value>, RuntimeError> {
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
                runtime_error!("pop_frame: Empty call stack")
            }
        }
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
        (self.value_stack.len() - self.register_base()) as u8
    }

    fn set_register(&mut self, register: u8, value: Value) {
        let index = self.register_index(register);

        if index >= self.value_stack.len() {
            self.value_stack.resize(index + 1, Value::Null);
        }

        self.value_stack[index] = value;
    }

    fn clone_register(&self, register: u8) -> Value {
        self.get_register(register).clone()
    }

    // Moves a value out of the stack, replacing it with Null
    fn remove_register(&mut self, register: u8) -> Value {
        let index = self.register_index(register);
        self.value_stack.push(Value::Null);
        self.value_stack.swap_remove(index)
    }

    fn get_register(&self, register: u8) -> &Value {
        let index = self.register_index(register);
        match self.value_stack.get(index) {
            Some(value) => value,
            None => {
                panic!(
                    "Out of bounds access, index: {}, register: {}, ip: {}",
                    index, register, self.instruction_ip
                );
            }
        }
    }

    fn get_register_safe(&self, register: u8) -> Option<&Value> {
        let index = self.register_index(register);
        self.value_stack.get(index)
    }

    fn get_register_mut(&mut self, register: u8) -> &mut Value {
        let index = self.register_index(register);
        &mut self.value_stack[index]
    }

    pub fn register_slice(&self, register: u8, count: u8) -> &[Value] {
        if count > 0 {
            let start = self.register_index(register);
            &self.value_stack[start..start + count as usize]
        } else {
            &[]
        }
    }

    fn truncate_registers(&mut self, len: u8) {
        self.value_stack
            .truncate(self.register_base() + len as usize);
    }

    pub fn get_args(&self, args: &Args) -> &[Value] {
        self.register_slice(args.register, args.count)
    }

    fn get_constant_str(&self, constant_index: ConstantIndex) -> &str {
        self.reader.chunk.constants.get_str(constant_index)
    }

    fn value_string_from_constant(&self, constant_index: ConstantIndex) -> ValueString {
        let constants = &self.reader.chunk.constants;
        let bounds = constants.get_str_bounds(constant_index);

        ValueString::new_with_bounds(constants.string_data().clone(), bounds)
            // The bounds have been already checked in the constant pool
            .unwrap()
    }

    fn binary_op_error(&self, lhs: &Value, rhs: &Value, op: &str) -> InstructionResult {
        runtime_error!(
            "Unable to perform operation '{op}' with '{}' and '{}'",
            lhs.type_as_string(),
            rhs.type_as_string(),
        )
    }
}

impl fmt::Debug for Vm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Vm")
    }
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
pub(crate) fn clone_generator_vm(vm: &Vm) -> Vm {
    let mut result = vm.clone();
    for value in result.value_stack.iter_mut() {
        if let Value::Iterator(ref mut i) = value {
            *i = i.make_copy()
        }
    }
    result
}

/// The ways in which a function's arguments will be treated when called externally
pub enum CallArgs<'a> {
    /// No args to be passed to the function
    None,
    /// The function will be called with a single argument
    Single(Value),
    /// The arguments will be passed to the function separately
    Separate(&'a [Value]),
    /// The arguments will be collected into a tuple before being passed to the function
    AsTuple(&'a [Value]),
}

// A cache of the export maps of imported modules
//
// The ValueMap is optional to prevent recursive imports (see Vm::run_import).
type ModuleCache = HashMap<PathBuf, Option<ValueMap>, BuildHasherDefault<FxHasher>>;
