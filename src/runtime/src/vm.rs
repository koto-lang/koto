use {
    crate::{
        core::CoreLib,
        external::{self, Args, ExternalFunction},
        frame::Frame,
        meta_map::meta_id_to_key,
        num2, num4, runtime_error,
        value::{self, RegisterSlice, RuntimeFunction},
        value_iterator::{IntRange, Iterable, ValueIterator, ValueIteratorOutput},
        BinaryOp, DefaultLogger, KotoLogger, Loader, MetaKey, RuntimeError, RuntimeErrorType,
        RuntimeResult, UnaryOp, Value, ValueList, ValueMap, ValueNumber, ValueString, ValueVec,
    },
    koto_bytecode::{Chunk, Instruction, InstructionReader, TypeId},
    koto_parser::{ConstantIndex, MetaId},
    parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard},
    std::{
        collections::HashMap,
        fmt,
        path::PathBuf,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
    },
    unicode_segmentation::UnicodeSegmentation,
};

#[derive(Clone, Debug)]
pub enum ControlFlow {
    Continue,
    Return(Value),
    Yield(Value),
}

// Instructions will place their results in registers, there's no Ok type
pub type InstructionResult = Result<(), RuntimeError>;

/// Context shared by all VMs across modules
struct SharedContext {
    pub prelude: ValueMap,
    core_lib: CoreLib,
    logger: Arc<dyn KotoLogger>,
}

impl Default for SharedContext {
    fn default() -> Self {
        Self::with_logger(Arc::new(DefaultLogger {}))
    }
}

impl SharedContext {
    fn with_logger(logger: Arc<dyn KotoLogger>) -> Self {
        let core_lib = CoreLib::default();

        let mut prelude = ValueMap::default();
        prelude.add_map("io", core_lib.io.clone());
        prelude.add_map("iterator", core_lib.iterator.clone());
        prelude.add_map("koto", core_lib.koto.clone());
        prelude.add_map("list", core_lib.list.clone());
        prelude.add_map("map", core_lib.map.clone());
        prelude.add_map("os", core_lib.os.clone());
        prelude.add_map("number", core_lib.number.clone());
        prelude.add_map("range", core_lib.range.clone());
        prelude.add_map("string", core_lib.string.clone());
        prelude.add_map("test", core_lib.test.clone());
        prelude.add_map("thread", core_lib.thread.clone());
        prelude.add_map("tuple", core_lib.tuple.clone());

        Self {
            prelude,
            core_lib,
            logger,
        }
    }
}

/// VM Context shared by VMs running in the same module
#[derive(Default)]
pub struct ModuleContext {
    /// The module's exported values
    pub exports: ValueMap,
    loader: Loader,
    modules: HashMap<PathBuf, Option<ValueMap>>,
    spawned_stop_flags: Vec<Arc<AtomicBool>>,
}

impl ModuleContext {
    fn spawn_new_context(&self) -> Self {
        Self {
            loader: self.loader.clone(),
            modules: self.modules.clone(),
            exports: Default::default(),
            spawned_stop_flags: Default::default(),
        }
    }

    fn reset(&mut self) {
        self.loader = Default::default();
        self.stop_spawned_vms();
    }

    fn stop_spawned_vms(&mut self) {
        for stop_flag in self.spawned_stop_flags.iter() {
            stop_flag.store(true, Ordering::Relaxed);
        }
        self.spawned_stop_flags.clear();
    }
}

impl Drop for ModuleContext {
    fn drop(&mut self) {
        self.stop_spawned_vms();
    }
}

pub struct VmSettings {
    pub logger: Arc<dyn KotoLogger>,
}

impl Default for VmSettings {
    fn default() -> Self {
        Self {
            logger: Arc::new(DefaultLogger {}),
        }
    }
}

pub struct Vm {
    context: Arc<RwLock<ModuleContext>>,
    context_shared: Arc<SharedContext>,
    reader: InstructionReader,
    value_stack: Vec<Value>,
    call_stack: Vec<Frame>,
    stop_flag: Option<Arc<AtomicBool>>,
    child_vm: Option<Box<Vm>>,
}

impl Default for Vm {
    fn default() -> Self {
        Self::with_settings(VmSettings::default())
    }
}

impl Vm {
    pub fn with_settings(settings: VmSettings) -> Self {
        Self {
            context: Arc::new(RwLock::new(ModuleContext::default())),
            context_shared: Arc::new(SharedContext::with_logger(settings.logger)),
            reader: InstructionReader::default(),
            value_stack: Vec::with_capacity(32),
            call_stack: vec![],
            stop_flag: None,
            child_vm: None,
        }
    }

    pub fn spawn_new_vm(&mut self) -> Self {
        Self {
            context: Arc::new(RwLock::new(self.context().spawn_new_context())),
            context_shared: self.context_shared.clone(),
            reader: InstructionReader::default(),
            value_stack: Vec::with_capacity(32),
            call_stack: vec![],
            stop_flag: None,
            child_vm: None,
        }
    }

    pub fn spawn_shared_vm(&mut self) -> Self {
        Self {
            context: self.context.clone(),
            context_shared: self.context_shared.clone(),
            reader: self.reader.clone(),
            value_stack: Vec::with_capacity(8),
            call_stack: vec![],
            stop_flag: None,
            child_vm: None,
        }
    }

    pub fn spawn_shared_concurrent_vm(&mut self) -> Self {
        let stop_flag = Arc::new(AtomicBool::new(false));
        self.context_mut()
            .spawned_stop_flags
            .push(stop_flag.clone());

        Self {
            context: self.context.clone(),
            context_shared: self.context_shared.clone(),
            reader: self.reader.clone(),
            value_stack: Vec::with_capacity(8),
            call_stack: vec![],
            stop_flag: Some(stop_flag),
            child_vm: None,
        }
    }

    pub fn child_vm(&mut self) -> &mut Vm {
        if self.child_vm.is_none() {
            self.child_vm = Some(Box::new(self.spawn_shared_vm()))
        }
        self.child_vm.as_mut().unwrap()
    }

    pub fn prelude(&self) -> ValueMap {
        self.context_shared.prelude.clone()
    }

    fn context(&self) -> RwLockReadGuard<ModuleContext> {
        self.context.read()
    }

    /// Access module context.
    pub fn context_mut(&mut self) -> RwLockWriteGuard<ModuleContext> {
        self.context.write()
    }

    pub fn logger(&self) -> &Arc<dyn KotoLogger> {
        &self.context_shared.logger
    }

    pub fn get_exported_value(&self, id: &str) -> Option<Value> {
        self.context()
            .exports
            .contents()
            .data
            .get_with_string(id)
            .cloned()
    }

    pub fn get_exported_function(&self, id: &str) -> Option<Value> {
        match self.get_exported_value(id) {
            Some(function) if function.is_callable() => Some(function),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.context_mut().reset();
        self.value_stack = Default::default();
        self.call_stack = Default::default();
    }

    pub fn run(&mut self, chunk: Arc<Chunk>) -> RuntimeResult {
        self.push_frame(chunk, 0, 0);
        self.execute_instructions()
    }

    pub fn continue_running(&mut self) -> RuntimeResult {
        if self.call_stack.is_empty() {
            Ok(Value::Empty)
        } else {
            self.execute_instructions()
        }
    }

    pub fn run_function(&mut self, function: Value, args: &[Value]) -> RuntimeResult {
        self.call_and_run_function(None, function, args)
    }

    pub fn run_instance_function(
        &mut self,
        instance: Value,
        function: Value,
        args: &[Value],
    ) -> RuntimeResult {
        self.call_and_run_function(Some(instance), function, args)
    }

    fn call_and_run_function(
        &mut self,
        instance: Option<Value>,
        function: Value,
        args: &[Value],
    ) -> RuntimeResult {
        if !self.call_stack.is_empty() {
            return runtime_error!(
                "run_function: the call stack must be empty, \
                 are you calling run_function on an active VM?"
            );
        }

        if !function.is_callable() {
            return runtime_error!("run_function: the provided value isn't a function");
        }

        let result_register = 0;
        let frame_base = 1;
        // If there's an instance value then it goes in the frame base
        let instance_register = if instance.is_some() {
            Some(frame_base)
        } else {
            None
        };

        self.value_stack.clear();
        self.value_stack.push(Value::Empty); // result register
        self.value_stack.push(instance.unwrap_or_default()); // frame base
        self.value_stack.extend_from_slice(args);

        self.call_function(
            result_register,
            function,
            frame_base,
            args.len() as u8,
            instance_register,
        )?;

        if self.call_stack.is_empty() {
            // If the call stack is empty, then an external function was called and the result
            // should be in the frame base.
            match self.value_stack.first() {
                Some(value) => Ok(value.clone()),
                None => runtime_error!("run_function: missing return register"),
            }
        } else {
            self.frame_mut().catch_barrier = true;
            let result = self.execute_instructions();
            if result.is_err() {
                self.pop_frame(Value::Empty)?;
            }
            result
        }
    }

    pub fn run_unary_op(&mut self, op: UnaryOp, value: Value) -> RuntimeResult {
        if !self.call_stack.is_empty() {
            return runtime_error!(
                "run_unary_op: the call stack must be empty,
                 are you calling run_unary_op on an active VM?"
            );
        }

        self.value_stack.clear();
        self.value_stack.push(Value::Empty); // result register
        self.value_stack.push(value);

        match op {
            UnaryOp::Negate => self.run_negate(0, 1)?,
            UnaryOp::Display => self.run_display(0, 1)?,
        }

        if self.call_stack.is_empty() {
            // If the call stack is empty, then the result will be in the result register
            Ok(self.clone_register(0))
        } else {
            // If the call stack isn't empty, then an overloaded operator has been called.
            self.execute_instructions()
        }
    }

    pub fn run_binary_op(&mut self, op: BinaryOp, lhs: Value, rhs: Value) -> RuntimeResult {
        if !self.call_stack.is_empty() {
            return runtime_error!(
                "run_binary_op: the call stack must be empty,
                 are you calling run_binary_op on an active VM?"
            );
        }

        self.value_stack.clear();
        self.value_stack.push(Value::Empty); // result register
        self.value_stack.push(lhs);
        self.value_stack.push(rhs);

        match op {
            BinaryOp::Add => self.run_add(0, 1, 2)?,
            BinaryOp::Subtract => self.run_subtract(0, 1, 2)?,
            BinaryOp::Multiply => self.run_multiply(0, 1, 2)?,
            BinaryOp::Divide => self.run_divide(0, 1, 2)?,
            BinaryOp::Modulo => self.run_modulo(0, 1, 2)?,
            BinaryOp::Less => self.run_less(0, 1, 2)?,
            BinaryOp::LessOrEqual => self.run_less_or_equal(0, 1, 2)?,
            BinaryOp::Greater => self.run_greater(0, 1, 2)?,
            BinaryOp::GreaterOrEqual => self.run_greater_or_equal(0, 1, 2)?,
            BinaryOp::Equal => self.run_equal(0, 1, 2)?,
            BinaryOp::NotEqual => self.run_not_equal(0, 1, 2)?,
            BinaryOp::Index => self.run_index(0, 1, 2)?,
        }

        if self.call_stack.is_empty() {
            // If the call stack is empty, then the result will be in the result register
            Ok(self.clone_register(0))
        } else {
            // If the call stack isn't empty, then an overloaded operator has been called.
            self.execute_instructions()
        }
    }

    pub fn run_tests(&mut self, tests: ValueMap) -> RuntimeResult {
        use Value::{Empty, Function, Map};

        // It's important throughout this function to make sure we don't hang on to any references
        // to the internal test map data while calling the test functions, otherwise we'll end up in
        // deadlocks when the map needs to be modified (e.g. in pre or post test functions).

        let self_arg = Map(tests.clone());

        let (pre_test, post_test, meta_entry_count) = {
            let meta = &tests.contents().meta;
            (
                meta.get(&MetaKey::PreTest).cloned(),
                meta.get(&MetaKey::PostTest).cloned(),
                meta.len(),
            )
        };
        let pass_self_to_pre_test = match &pre_test {
            Some(Function(f)) => f.instance_function,
            _ => false,
        };
        let pass_self_to_post_test = match &post_test {
            Some(Function(f)) => f.instance_function,
            _ => false,
        };

        for i in 0..meta_entry_count {
            let meta_entry = tests
                .contents()
                .meta
                .get_index(i)
                .map(|(key, value)| (key.clone(), value.clone()));

            match meta_entry {
                Some((MetaKey::Test(test_name), test)) if test.is_callable() => {
                    let make_test_error = |error: RuntimeError, message: &str| {
                        Err(error.with_prefix(&format!("{} '{}'", message, test_name)))
                    };

                    if let Some(pre_test) = &pre_test {
                        if pre_test.is_callable() {
                            let pre_test_result = if pass_self_to_pre_test {
                                self.run_instance_function(self_arg.clone(), pre_test.clone(), &[])
                            } else {
                                self.run_function(pre_test.clone(), &[])
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
                        self.run_instance_function(self_arg.clone(), test, &[])
                    } else {
                        self.run_function(test, &[])
                    };

                    if let Err(error) = test_result {
                        return make_test_error(error, "Error while running test");
                    }

                    if let Some(post_test) = &post_test {
                        if post_test.is_callable() {
                            let post_test_result = if pass_self_to_post_test {
                                self.run_instance_function(self_arg.clone(), post_test.clone(), &[])
                            } else {
                                self.run_function(post_test.clone(), &[])
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

        Ok(Empty)
    }

    fn execute_instructions(&mut self) -> RuntimeResult {
        let mut result = Value::Empty;

        let mut instruction_ip = self.ip();

        while let Some(instruction) = self.reader.next() {
            if let Some(stop_flag) = &self.stop_flag {
                if stop_flag.load(Ordering::Relaxed) {
                    break;
                }
            }
            match self.execute_instruction(instruction, instruction_ip) {
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

                    error.extend_trace(self.chunk(), instruction_ip);

                    while let Some(frame) = self.call_stack.last() {
                        if let Some((error_register, catch_ip)) = frame.catch_stack.last() {
                            recover_register_and_ip = Some((*error_register, *catch_ip));
                            break;
                        } else {
                            if frame.catch_barrier {
                                return Err(error);
                            }

                            self.pop_frame(Value::Empty)?;

                            if !self.call_stack.is_empty() {
                                error.extend_trace(self.chunk(), self.ip());
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

            instruction_ip = self.ip();
        }

        Ok(result)
    }

    fn execute_instruction(
        &mut self,
        instruction: Instruction,
        instruction_ip: usize,
    ) -> Result<ControlFlow, RuntimeError> {
        use Value::*;

        let mut control_flow = ControlFlow::Continue;

        match instruction {
            Instruction::Error { message } => {
                runtime_error!("{}", message)
            }
            Instruction::Copy { target, source } => {
                self.run_copy(target, source);
                Ok(())
            }
            Instruction::SetEmpty { register } => {
                self.set_register(register, Empty);
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
            Instruction::SetExport { export, source } => {
                self.run_set_export(export, source);
                Ok(())
            }
            Instruction::Import { register, constant } => self.run_import(register, constant),
            Instruction::MakeTuple {
                register,
                start,
                count,
            } => {
                self.run_make_tuple(register, start, count);
                Ok(())
            }
            Instruction::MakeTempTuple {
                register,
                start,
                count,
            } => {
                self.set_register(register, TemporaryTuple(RegisterSlice { start, count }));
                Ok(())
            }
            Instruction::MakeList {
                register,
                size_hint,
            } => {
                self.set_register(register, List(ValueList::with_capacity(size_hint)));
                Ok(())
            }
            Instruction::MakeMap {
                register,
                size_hint,
            } => {
                self.set_register(register, Map(ValueMap::with_capacity(size_hint)));
                Ok(())
            }
            Instruction::MakeNum2 {
                register,
                count,
                element_register,
            } => self.run_make_num2(register, count, element_register),
            Instruction::MakeNum4 {
                register,
                count,
                element_register,
            } => self.run_make_num4(register, count, element_register),
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
            Instruction::Function { .. } => {
                self.run_make_function(instruction);
                Ok(())
            }
            Instruction::Capture {
                function,
                target,
                source,
            } => self.run_capture_value(function, target, source),
            Instruction::Negate { register, source } => self.run_negate(register, source),
            Instruction::Add { register, lhs, rhs } => self.run_add(register, lhs, rhs),
            Instruction::Subtract { register, lhs, rhs } => self.run_subtract(register, lhs, rhs),
            Instruction::Multiply { register, lhs, rhs } => self.run_multiply(register, lhs, rhs),
            Instruction::Divide { register, lhs, rhs } => self.run_divide(register, lhs, rhs),
            Instruction::Modulo { register, lhs, rhs } => self.run_modulo(register, lhs, rhs),
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
            Instruction::JumpIf {
                register,
                offset,
                jump_condition,
            } => self.run_jump_if(register, offset, jump_condition),
            Instruction::JumpBack { offset } => {
                self.jump_ip_back(offset);
                Ok(())
            }
            Instruction::JumpBackIf {
                register,
                offset,
                jump_condition,
            } => self.run_jump_back_if(register, offset, jump_condition),
            Instruction::Call {
                result,
                function,
                frame_base,
                arg_count,
            } => self.call_function(
                result,
                self.clone_register(function),
                frame_base,
                arg_count,
                None,
            ),
            Instruction::CallChild {
                result,
                function,
                frame_base,
                arg_count,
                parent,
            } => self.call_function(
                result,
                self.clone_register(function),
                frame_base,
                arg_count,
                Some(parent),
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
                    Map(m) if m.contents().meta.contains_key(&display_op) => Err(
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
            Instruction::ValueIndex {
                register,
                value,
                index,
            } => self.run_value_index(register, value, index),
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
            Instruction::ListPushValue { list, value } => self.run_list_push(list, value),
            Instruction::ListPushValues {
                list,
                values_start,
                count,
            } => {
                for value_register in values_start..(values_start + count) {
                    self.run_list_push(list, value_register)?;
                }
                Ok(())
            }
            Instruction::ListUpdate { list, index, value } => {
                self.run_list_update(list, index, value)
            }
            Instruction::Index {
                register,
                value,
                index,
            } => self.run_index(register, value, index),
            Instruction::MapInsert {
                register,
                value,
                key,
            } => self.run_map_insert(register, value, key),
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
            Instruction::Access { register, map, key } => self.run_access(register, map, key),
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
            Instruction::Debug { register, constant } => {
                self.run_debug(register, constant, instruction_ip)
            }
            Instruction::CheckType { register, type_id } => self.run_check_type(register, type_id),
            Instruction::CheckSize { register, size } => self.run_check_size(register, size),
        }?;

        Ok(control_flow)
    }

    fn run_copy(&mut self, target: u8, source: u8) {
        let value = match self.clone_register(source) {
            Value::TemporaryTuple(RegisterSlice { start, count }) => {
                // A temporary tuple shouldn't make it into a named value,
                // so here it gets converted into a regular tuple.
                Value::Tuple(self.register_slice(start, count).into())
            }
            other => other,
        };
        self.set_register(target, value);
    }

    fn run_load_non_local(
        &mut self,
        register: u8,
        constant_index: ConstantIndex,
    ) -> InstructionResult {
        let name = self.get_constant_str(constant_index);

        let non_local = self
            .context()
            .exports
            .contents()
            .data
            .get_with_string(name)
            .cloned()
            .or_else(|| {
                self.context_shared
                    .prelude
                    .contents()
                    .data
                    .get_with_string(name)
                    .cloned()
            });

        if let Some(non_local) = non_local {
            self.set_register(register, non_local);
            Ok(())
        } else {
            runtime_error!("'{}' not found", name)
        }
    }

    fn run_set_export(&mut self, constant_index: ConstantIndex, source_register: u8) {
        let export_name = Value::Str(self.value_string_from_constant(constant_index));
        let value = self.clone_register(source_register);
        self.context_mut()
            .exports
            .contents_mut()
            .data
            .insert(export_name.into(), value);
    }

    fn run_make_tuple(&mut self, register: u8, start: u8, count: u8) {
        let mut copied = Vec::with_capacity(count as usize);

        for register in start..start + count {
            copied.push(self.clone_register(register));
        }

        self.set_register(register, Value::Tuple(copied.into()));
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
                    return runtime_error!(
                        "RangeTo: negative numbers not allowed, found '{}'",
                        end
                    );
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
                        "RangeFrom: negative numbers not allowed, found '{}'",
                        start
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
            (Some(unexpected), _) => {
                return self.unexpected_type_error("Expected Number for range start", unexpected);
            }
            (_, Some(unexpected)) => {
                return self.unexpected_type_error("Expected Number for range end", unexpected);
            }
        };

        self.set_register(register, range);
        Ok(())
    }

    fn run_make_iterator(&mut self, register: u8, iterable_register: u8) -> InstructionResult {
        use Value::*;

        let iterable = self.clone_register(iterable_register);

        if matches!(iterable, Iterator(_)) {
            self.set_register(register, iterable);
        } else {
            let iterator = match iterable {
                Range(int_range) => ValueIterator::with_range(int_range),
                List(list) => ValueIterator::with_list(list),
                Map(map) => ValueIterator::with_map(map),
                Tuple(tuple) => ValueIterator::with_tuple(tuple),
                Str(s) => ValueIterator::with_string(s),
                unexpected => {
                    return self.unexpected_type_error(
                        "Expected iterable value while making iterator",
                        &unexpected,
                    );
                }
            };

            self.set_register(register, iterator.into());
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
            unexpected => {
                return runtime_error!(
                    "Expected Iterator, found '{}'",
                    unexpected.type_as_string()
                );
            }
        };

        match (result, result_register) {
            (Some(Ok(_)), None) => {}
            (Some(Ok(ValueIteratorOutput::Value(value))), Some(register)) => {
                self.set_register(register, value)
            }
            (Some(Ok(ValueIteratorOutput::ValuePair(first, second))), Some(register)) => {
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
            (Some(Err(error)), _) => return runtime_error!(error.to_string()),
            (None, _) => self.jump_ip(jump_offset),
        };

        Ok(())
    }

    fn run_value_index(&mut self, register: u8, value: u8, index: i8) -> InstructionResult {
        use Value::*;

        let result = match self.get_register(value) {
            List(list) => {
                let index = signed_index_to_unsigned(index, list.data().len());
                list.data().get(index).cloned().unwrap_or(Empty)
            }
            Tuple(tuple) => {
                let index = signed_index_to_unsigned(index, tuple.data().len());
                tuple.data().get(index).cloned().unwrap_or(Empty)
            }
            TemporaryTuple(RegisterSlice { start, count }) => {
                let count = *count;
                if (index.abs() as u8) < count {
                    let index = signed_index_to_unsigned(index, count as usize);
                    self.clone_register(start + index as u8)
                } else {
                    Empty
                }
            }
            Num2(n) => {
                let index = signed_index_to_unsigned(index, 2);
                if index < 2 {
                    Number(n[index].into())
                } else {
                    Empty
                }
            }
            Num4(n) => {
                let index = signed_index_to_unsigned(index, 4);
                if index < 4 {
                    Number(n[index].into())
                } else {
                    Empty
                }
            }
            unexpected => {
                return self
                    .unexpected_type_error("ValueIndex: Expected indexable value", unexpected);
            }
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
                        .map_or(Empty, |entries| List(ValueList::from_slice(entries)))
                } else {
                    list.data()
                        .get(index..)
                        .map_or(Empty, |entries| List(ValueList::from_slice(entries)))
                }
            }
            Tuple(tuple) => {
                let index = signed_index_to_unsigned(index, tuple.data().len());
                if is_slice_to {
                    tuple
                        .data()
                        .get(..index)
                        .map_or(Empty, |entries| Tuple(entries.into()))
                } else {
                    tuple
                        .data()
                        .get(index..)
                        .map_or(Empty, |entries| Tuple(entries.into()))
                }
            }
            unexpected => {
                return self.unexpected_type_error("SliceFrom: expected List or Tuple", unexpected);
            }
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
                size,
            } => {
                // Initialize the function's captures with Empty
                let captures = if capture_count > 0 {
                    let mut captures = ValueVec::new();
                    captures.resize(capture_count as usize, Empty);
                    Some(ValueList::with_data(captures))
                } else {
                    None
                };

                let function = RuntimeFunction {
                    chunk: self.chunk(),
                    ip: self.ip(),
                    arg_count,
                    instance_function,
                    variadic,
                    captures,
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
        let capture_list = match self.get_register(function) {
            Value::Function(f) => &f.captures,
            Value::Generator(g) => &g.captures,
            unexpected => {
                return self.unexpected_type_error("Capture: expected Function", unexpected);
            }
        };

        match capture_list {
            Some(capture_list) => {
                capture_list.data_mut()[capture_index as usize] = self.clone_register(value)
            }
            None => return runtime_error!("Capture: missing capture list for function"),
        }

        Ok(())
    }

    fn run_negate(&mut self, result: u8, value: u8) -> InstructionResult {
        use {UnaryOp::Negate, Value::*};

        let result_value = match &self.get_register(value) {
            Bool(b) => Bool(!b),
            Number(n) => Number(-n),
            Num2(v) => Num2(-v),
            Num4(v) => Num4(-v),
            Map(map) if map.contents().meta.contains_key(&MetaKey::UnaryOp(Negate)) => {
                let map = map.clone();
                return self.call_overloaded_unary_op(result, value, map, Negate);
            }
            unexpected => {
                return self.unexpected_type_error("Negate: expected negatable value", unexpected);
            }
        };
        self.set_register(result, result_value);

        Ok(())
    }

    fn run_display(&mut self, result: u8, value: u8) -> InstructionResult {
        use {UnaryOp::Display, Value::*};

        let result_value = match &self.get_register(value) {
            Map(map) if map.contents().meta.contains_key(&MetaKey::UnaryOp(Display)) => {
                let map = map.clone();
                return self.call_overloaded_unary_op(result, value, map, Display);
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
            (List(a), List(b)) => {
                let mut result = ValueVec::new();
                result.extend(a.data().iter().chain(b.data().iter()).cloned());
                List(ValueList::with_data(result))
            }
            (List(a), Tuple(b)) => {
                let mut result = ValueVec::new();
                result.extend(a.data().iter().chain(b.data().iter()).cloned());
                List(ValueList::with_data(result))
            }
            (Str(a), Str(b)) => {
                let result = a.to_string() + b.as_ref();
                Str(result.into())
            }
            (Map(map), value) if map.contents().meta.contains_key(&MetaKey::BinaryOp(Add)) => {
                let map = map.clone();
                let value = value.clone();
                return self.call_overloaded_binary_op(result, lhs, map, value, Add);
            }
            (Map(a), Map(b)) => {
                let mut result = a.contents().clone();
                result.extend(&b.contents());
                Map(ValueMap::with_contents(result))
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
            (Map(map), value)
                if map
                    .contents()
                    .meta
                    .contains_key(&MetaKey::BinaryOp(Subtract)) =>
            {
                let map = map.clone();
                let value = value.clone();
                return self.call_overloaded_binary_op(result, lhs, map, value, Subtract);
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
            (Map(map), value)
                if map
                    .contents()
                    .meta
                    .contains_key(&MetaKey::BinaryOp(Multiply)) =>
            {
                let map = map.clone();
                let value = value.clone();
                return self.call_overloaded_binary_op(result, lhs, map, value, Multiply);
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
            (Map(map), value) if map.contents().meta.contains_key(&MetaKey::BinaryOp(Divide)) => {
                let map = map.clone();
                let value = value.clone();
                return self.call_overloaded_binary_op(result, lhs, map, value, Divide);
            }
            _ => return self.binary_op_error(lhs_value, rhs_value, "/"),
        };

        self.set_register(result, result_value);
        Ok(())
    }

    fn run_modulo(&mut self, result: u8, lhs: u8, rhs: u8) -> InstructionResult {
        use {BinaryOp::Modulo, Value::*};

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result_value = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Number(a % b),
            (Number(a), Num2(b)) => Num2(a % b),
            (Num2(a), Num2(b)) => Num2(a % b),
            (Num2(a), Number(b)) => Num2(a % b),
            (Number(a), Num4(b)) => Num4(a % b),
            (Num4(a), Num4(b)) => Num4(a % b),
            (Num4(a), Number(b)) => Num4(a % b),
            (Map(map), value) if map.contents().meta.contains_key(&MetaKey::BinaryOp(Modulo)) => {
                let map = map.clone();
                let value = value.clone();
                return self.call_overloaded_binary_op(result, lhs, map, value, Modulo);
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
            (Map(map), value) if map.contents().meta.contains_key(&MetaKey::BinaryOp(Less)) => {
                let map = map.clone();
                let value = value.clone();
                return self.call_overloaded_binary_op(result, lhs, map, value, Less);
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
            (Map(map), value)
                if map
                    .contents()
                    .meta
                    .contains_key(&MetaKey::BinaryOp(LessOrEqual)) =>
            {
                let map = map.clone();
                let value = value.clone();
                return self.call_overloaded_binary_op(result, lhs, map, value, LessOrEqual);
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
            (Map(map), value)
                if map
                    .contents()
                    .meta
                    .contains_key(&MetaKey::BinaryOp(Greater)) =>
            {
                let map = map.clone();
                let value = value.clone();
                return self.call_overloaded_binary_op(result, lhs, map, value, Greater);
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
            (Map(map), value)
                if map
                    .contents()
                    .meta
                    .contains_key(&MetaKey::BinaryOp(GreaterOrEqual)) =>
            {
                let map = map.clone();
                let value = value.clone();
                return self.call_overloaded_binary_op(result, lhs, map, value, GreaterOrEqual);
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
            (Empty, Empty) => true,
            (ExternalDataId, ExternalDataId) => true,
            (List(a), List(b)) => {
                let a = a.clone();
                let b = b.clone();
                let data_a = a.data();
                let data_b = b.data();
                self.child_vm().compare_value_ranges(&data_a, &data_b)?
            }
            (Tuple(a), Tuple(b)) => {
                let a = a.clone();
                let b = b.clone();
                let data_a = a.data();
                let data_b = b.data();
                self.child_vm().compare_value_ranges(&data_a, &data_b)?
            }
            (Map(map), value) if map.contents().meta.contains_key(&MetaKey::BinaryOp(Equal)) => {
                let map = map.clone();
                let value = value.clone();
                return self.call_overloaded_binary_op(result, lhs, map, value, Equal);
            }
            (Map(a), Map(b)) => {
                let a = a.clone();
                let b = b.clone();
                self.child_vm().compare_value_maps(a, b)?
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
                            self.child_vm().compare_value_ranges(&data_a, &data_b)?
                        }
                        _ => false,
                    }
                } else {
                    false
                }
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
            (Empty, Empty) => false,
            (ExternalDataId, ExternalDataId) => false,
            (List(a), List(b)) => {
                let a = a.clone();
                let b = b.clone();
                let data_a = a.data();
                let data_b = b.data();
                !self.child_vm().compare_value_ranges(&data_a, &data_b)?
            }
            (Tuple(a), Tuple(b)) => {
                let a = a.clone();
                let b = b.clone();
                let data_a = a.data();
                let data_b = b.data();
                !self.child_vm().compare_value_ranges(&data_a, &data_b)?
            }
            (Map(map), value)
                if map
                    .contents()
                    .meta
                    .contains_key(&MetaKey::BinaryOp(NotEqual)) =>
            {
                let map = map.clone();
                let value = value.clone();
                return self.call_overloaded_binary_op(result, lhs, map, value, NotEqual);
            }
            (Map(a), Map(b)) => {
                let a = a.clone();
                let b = b.clone();
                !self.child_vm().compare_value_maps(a, b)?
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
                            !self.child_vm().compare_value_ranges(&data_a, &data_b)?
                        }
                        _ => true,
                    }
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

        for ((key_a, value_a), (key_b, value_b)) in map_a
            .contents()
            .data
            .iter()
            .zip(map_b.contents().data.iter())
        {
            if key_a != key_b {
                return Ok(false);
            }
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

    fn call_overloaded_unary_op(
        &mut self,
        result_register: u8,
        map_register: u8,
        map: ValueMap,
        op: UnaryOp,
    ) -> InstructionResult {
        let op = match map.contents().meta.get(&MetaKey::UnaryOp(op)) {
            Some(op) => op.clone(),
            None => return runtime_error!("Missing overloaded {:?} key", op),
        };

        // Set up the call registers at the end of the stack
        let stack_len = self.value_stack.len();
        let frame_base = (stack_len - self.register_base()) as u8;
        self.value_stack.push(Value::Empty); // frame_base
        self.call_function(
            result_register,
            op,
            frame_base,
            0, // 0 args
            Some(map_register),
        )?;

        Ok(())
    }

    fn call_overloaded_binary_op(
        &mut self,
        result_register: u8,
        map_register: u8,
        map: ValueMap,
        rhs: Value,
        op: BinaryOp,
    ) -> InstructionResult {
        let op = match map.contents().meta.get(&MetaKey::BinaryOp(op)) {
            Some(op) => op.clone(),
            None => return runtime_error!("Missing overloaded {:?} operation", op),
        };

        // Set up the call registers at the end of the stack
        let stack_len = self.value_stack.len();
        let frame_base = (stack_len - self.register_base()) as u8;
        self.value_stack.push(Value::Empty); // frame_base
        self.value_stack.push(rhs); // arg
        self.call_function(
            result_register,
            op,
            frame_base,
            1, // 1 arg, the rhs value
            Some(map_register),
        )?;

        Ok(())
    }

    fn run_jump_if(
        &mut self,
        register: u8,
        offset: usize,
        jump_condition: bool,
    ) -> InstructionResult {
        match self.get_register(register) {
            Value::Bool(b) => {
                if *b == jump_condition {
                    self.jump_ip(offset);
                }
            }
            unexpected => {
                return self.unexpected_type_error("JumpIf: expected Bool", unexpected);
            }
        }
        Ok(())
    }

    fn run_jump_back_if(
        &mut self,
        register: u8,
        offset: usize,
        jump_condition: bool,
    ) -> InstructionResult {
        match self.get_register(register) {
            Value::Bool(b) => {
                if *b == jump_condition {
                    self.jump_ip_back(offset);
                }
            }
            unexpected => {
                return self.unexpected_type_error("JumpIf: expected Bool", unexpected);
            }
        }
        Ok(())
    }

    fn run_size(&mut self, register: u8, value: u8) {
        let result = self.get_register(value).size();
        self.set_register(register, Value::Number(result.into()));
    }

    fn run_import(
        &mut self,
        result_register: u8,
        import_constant: ConstantIndex,
    ) -> InstructionResult {
        let import_name = self.value_string_from_constant(import_constant);

        let maybe_export = self
            .context()
            .exports
            .contents()
            .data
            .get_with_string(&import_name)
            .cloned();
        if let Some(value) = maybe_export {
            self.set_register(result_register, value);
        } else {
            let maybe_in_prelude = self
                .context_shared
                .prelude
                .contents()
                .data
                .get_with_string(&import_name)
                .cloned();
            if let Some(value) = maybe_in_prelude {
                self.set_register(result_register, value);
            } else {
                let source_path = self.reader.chunk.source_path.clone();
                let compile_result = self
                    .context_mut()
                    .loader
                    .compile_module(&import_name, source_path);
                let (module_chunk, module_path) = match compile_result {
                    Ok(chunk) => chunk,
                    Err(e) => return runtime_error!("Failed to import '{}': {}", import_name, e),
                };
                let maybe_module = self.context().modules.get(&module_path).cloned();
                match maybe_module {
                    Some(Some(module)) => self.set_register(result_register, Value::Map(module)),
                    Some(None) => {
                        return runtime_error!("Recursive import of module '{}'", import_name)
                    }
                    None => {
                        // Insert a placeholder for the new module, preventing recursive imports
                        self.context_mut().modules.insert(module_path.clone(), None);

                        // Run the module chunk
                        let mut vm = self.spawn_new_vm();
                        match vm.run(module_chunk) {
                            Ok(_) => {
                                if let Some(main) = vm.get_exported_function("main") {
                                    if let Err(error) = vm.run_function(main, &[]) {
                                        self.context_mut().modules.remove(&module_path);
                                        return Err(error);
                                    }
                                }
                            }
                            Err(error) => {
                                self.context_mut().modules.remove(&module_path);
                                return Err(error);
                            }
                        }

                        // Cache the resulting module's exports map
                        let module_exports = vm.context().exports.clone();
                        self.context_mut()
                            .modules
                            .insert(module_path, Some(module_exports.clone()));

                        self.set_register(result_register, Value::Map(module_exports));
                    }
                }
            }
        }

        Ok(())
    }

    fn run_make_num2(
        &mut self,
        result_register: u8,
        element_count: u8,
        element_register: u8,
    ) -> InstructionResult {
        use Value::*;

        let result = if element_count == 1 {
            match self.get_register(element_register) {
                Number(n) => num2::Num2(n.into(), n.into()),
                Num2(n) => *n,
                List(list) => {
                    let mut result = num2::Num2::default();
                    for (i, value) in list.data().iter().take(2).enumerate() {
                        match value {
                            Number(n) => result[i] = n.into(),
                            unexpected => {
                                return self
                                    .unexpected_type_error("num2: Expected Number", unexpected);
                            }
                        }
                    }
                    result
                }
                unexpected => {
                    return self
                        .unexpected_type_error("num2: Expected Number, Num2, or List", unexpected);
                }
            }
        } else {
            let mut result = num2::Num2::default();
            for i in 0..element_count {
                match self.get_register(element_register + i) {
                    Number(n) => result[i as usize] = n.into(),
                    unexpected => {
                        return self.unexpected_type_error(
                            "num2: Expected Number, Num2, or List",
                            unexpected,
                        );
                    }
                }
            }
            result
        };

        self.set_register(result_register, Num2(result));
        Ok(())
    }

    fn run_make_num4(
        &mut self,
        result_register: u8,
        element_count: u8,
        element_register: u8,
    ) -> InstructionResult {
        use Value::*;
        let result = if element_count == 1 {
            match self.get_register(element_register) {
                Number(n) => {
                    let n = n.into();
                    num4::Num4(n, n, n, n)
                }
                Num2(n) => num4::Num4(n[0] as f32, n[1] as f32, 0.0, 0.0),
                Num4(n) => *n,
                List(list) => {
                    let mut result = num4::Num4::default();
                    for (i, value) in list.data().iter().take(4).enumerate() {
                        match value {
                            Number(n) => result[i] = n.into(),
                            unexpected => {
                                return self
                                    .unexpected_type_error("num4: Expected Number", unexpected);
                            }
                        }
                    }
                    result
                }
                unexpected => {
                    return self
                        .unexpected_type_error("num4: Expected Number, Num4, or List", unexpected);
                }
            }
        } else {
            let mut result = num4::Num4::default();
            for i in 0..element_count {
                match self.get_register(element_register + i) {
                    Number(n) => result[i as usize] = n.into(),
                    unexpected => {
                        return self.unexpected_type_error(
                            "num4: Expected Number, Num4, or List",
                            unexpected,
                        );
                    }
                }
            }
            result
        };

        self.set_register(result_register, Num4(result));
        Ok(())
    }

    fn run_list_push(&mut self, list_register: u8, value_register: u8) -> InstructionResult {
        use Value::*;

        let value = self.clone_register(value_register);

        match self.get_register_mut(list_register) {
            List(list) => match value {
                Range(range) => {
                    list.data_mut()
                        .extend(ValueIterator::new(Iterable::Range(range)).map(
                            |iterator_output| match iterator_output {
                                Ok(ValueIteratorOutput::Value(value)) => value,
                                _ => unreachable!(),
                            },
                        ));
                }
                _ => list.data_mut().push(value),
            },
            unexpected => {
                return runtime_error!("Expected List, found '{}'", unexpected,);
            }
        };
        Ok(())
    }

    fn run_list_update(
        &mut self,
        list_register: u8,
        index_register: u8,
        value_register: u8,
    ) -> InstructionResult {
        use Value::*;

        let index_value = self.clone_register(index_register);
        let value = self.clone_register(value_register);

        match self.get_register_mut(list_register) {
            List(list) => {
                let list_len = list.len();
                match index_value {
                    Number(index) => {
                        let u_index = usize::from(index);
                        if index >= 0.0 && u_index < list_len {
                            list.data_mut()[u_index] = value;
                        } else {
                            return runtime_error!("Index '{}' not in List", index);
                        }
                    }
                    Range(IntRange { start, end }) => {
                        let ustart = start as usize;
                        let uend = end as usize;

                        if start < 0 || end < 0 {
                            return runtime_error!(
                                "Indexing with negative indices isn't supported, \
                                                start: {}, end: {}",
                                start,
                                end
                            );
                        } else if start > end {
                            return runtime_error!(
                                "Indexing with a descending range isn't supported, \
                                                start: {}, end: {}",
                                start,
                                end
                            );
                        } else if ustart > list_len || uend > list_len {
                            return runtime_error!(
                                "Index out of bounds, \
                                                List has a length of {} - start: {}, end: {}",
                                list_len,
                                start,
                                end
                            );
                        } else {
                            let mut list_data = list.data_mut();
                            for i in ustart..uend {
                                list_data[i] = value.clone();
                            }
                        }
                    }
                    IndexRange(value::IndexRange { start, end }) => {
                        let end = end.unwrap_or(list_len);
                        if start > end {
                            return runtime_error!(
                                "Indexing with a descending range isn't supported, \
                                                start: {}, end: {}",
                                start,
                                end
                            );
                        } else if start > list_len || end > list_len {
                            return runtime_error!(
                                "Index out of bounds, \
                                                List has a length of {} - start: {}, end: {}",
                                list_len,
                                start,
                                end
                            );
                        } else {
                            let mut list_data = list.data_mut();
                            for i in start..end {
                                list_data[i] = value.clone();
                            }
                        }
                    }
                    unexpected => {
                        return self.unexpected_type_error("Expected List", &unexpected);
                    }
                }
            }
            unexpected => {
                return runtime_error!("Expected List, found '{}'", unexpected);
            }
        };

        Ok(())
    }

    fn validate_index(&self, n: ValueNumber, size: Option<usize>) -> Result<usize, RuntimeError> {
        let index = usize::from(n);

        if n < 0.0 {
            return runtime_error!("Negative indices aren't allowed ('{}')", n);
        } else if let Some(size) = size {
            if index >= size {
                return runtime_error!("Index out of bounds - index: {}, size: {}", n, size);
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
                "Indexing with negative indices isn't supported, start: {}, end: {}",
                start,
                end
            );
        } else if start > end {
            return runtime_error!(
                "Indexing with a descending range isn't supported, start: {}, end: {}",
                start,
                end
            );
        } else if let Some(size) = size {
            if ustart > size || uend > size {
                return runtime_error!(
                    "Index out of bounds, start: {}, end: {}, size: {}",
                    start,
                    end,
                    size
                );
            }
        }

        Ok((ustart, uend))
    }

    fn validate_index_range(&self, start: usize, end: usize, size: usize) -> InstructionResult {
        if start > end {
            runtime_error!(
                "Indexing with a descending range isn't supported, start: {}, end: {}",
                start,
                end
            )
        } else if start > size || end > size {
            runtime_error!(
                "Index out of bounds, start: {}, end: {}, size: {}",
                start,
                end,
                size
            )
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

        match (value, index) {
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

                if let Some(result) = s.graphemes(true).nth(index) {
                    self.set_register(result_register, Str(result.into()));
                } else {
                    return runtime_error!(
                        "Index out of bounds - index: {}, size: {}",
                        index,
                        s.graphemes(true).count()
                    );
                }
            }
            (Str(s), Range(IntRange { start, end })) => {
                let (start, end) = self.validate_int_range(start, end, None)?;

                let result = if start == end {
                    ""
                } else {
                    let mut result_start = None;
                    let mut result_end = None;
                    let mut grapheme_count = 0;
                    for (i, (grapheme_start, grapheme)) in s.grapheme_indices(true).enumerate() {
                        grapheme_count += 1;
                        if i == start {
                            result_start = Some(grapheme_start);
                        } else if i == end - 1 {
                            // By checking against end - 1, we can allow for indexing 'one past the
                            // end' to get the last character.
                            result_end = Some(grapheme_start + grapheme.len());
                            break;
                        }
                    }
                    match (result_start, result_end) {
                        (Some(result_start), Some(result_end)) => &s[result_start..result_end],
                        _ => {
                            return runtime_error!(
                                "Index out of bounds for string - start: {}, end {}, size: {}",
                                start,
                                end,
                                grapheme_count
                            );
                        }
                    }
                };

                self.set_register(result_register, Str(result.into()))
            }
            (Str(s), IndexRange(value::IndexRange { start, end })) => {
                let end_unwrapped = end.unwrap_or_else(|| s.len());
                if start > end_unwrapped {
                    return runtime_error!(
                        "Indexing with a descending range isn't supported, start: {}{}",
                        start,
                        if end.is_some() {
                            format!(", {}", end_unwrapped)
                        } else {
                            "".to_string()
                        },
                    );
                }

                let result = if start == end_unwrapped {
                    ""
                } else {
                    let mut result_start = None;
                    let mut result_end = None;
                    let mut grapheme_count = 0;
                    for (i, (grapheme_start, grapheme)) in s.grapheme_indices(true).enumerate() {
                        grapheme_count += 1;
                        if i == start {
                            result_start = Some(grapheme_start);
                            if end.is_none() {
                                break;
                            }
                        } else if i == end_unwrapped - 1 {
                            result_end = Some(grapheme_start + grapheme.len());
                            break;
                        }
                    }
                    match (result_start, result_end) {
                        (Some(result_start), Some(result_end)) => &s[result_start..result_end],
                        (Some(result_start), None) if end.is_none() => &s[result_start..],
                        _ => {
                            return runtime_error!(
                                "Index out of bounds for string - start: {}{}, size: {}",
                                start,
                                if end.is_some() {
                                    format!(", {}", end_unwrapped)
                                } else {
                                    "".to_string()
                                },
                                grapheme_count
                            );
                        }
                    }
                };

                self.set_register(result_register, Str(result.into()))
            }
            (Num2(n), Number(i)) => {
                let i = usize::from(i);
                match i {
                    0 | 1 => self.set_register(result_register, Number(n[i].into())),
                    other => return runtime_error!("Index out of bounds for Num2, {}", other),
                }
            }
            (Num4(n), Number(i)) => {
                let i = usize::from(i);
                match i {
                    0 | 1 | 2 | 3 => self.set_register(result_register, Number(n[i].into())),
                    other => return runtime_error!("Index out of bounds for Num4, {}", other),
                }
            }
            (Map(map), index) if map.contents().meta.contains_key(&MetaKey::BinaryOp(Index)) => {
                return self.call_overloaded_binary_op(
                    result_register,
                    value_register,
                    map,
                    index,
                    Index,
                );
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
        value: u8,
        key: ConstantIndex,
    ) -> InstructionResult {
        let key_string = self.value_string_from_constant(key);
        let value = self.clone_register(value);

        match self.get_register_mut(map_register) {
            Value::Map(map) => {
                map.contents_mut().data.insert(key_string.into(), value);
                Ok(())
            }
            unexpected => runtime_error!(
                "MapInsert: Expected Map, found '{}'",
                unexpected.type_as_string()
            ),
        }
    }

    fn run_meta_insert(
        &mut self,
        map_register: u8,
        value: u8,
        meta_id: MetaId,
    ) -> InstructionResult {
        let value = self.clone_register(value);
        let meta_key = match meta_id_to_key(meta_id, None) {
            Ok(meta_key) => meta_key,
            Err(error) => return runtime_error!("MetaInsert: {}", error),
        };

        match self.get_register_mut(map_register) {
            Value::Map(map) => {
                map.contents_mut().meta.insert(meta_key, value);
                Ok(())
            }
            unexpected => runtime_error!(
                "MetaInsert: Expected Map, found '{}'",
                unexpected.type_as_string()
            ),
        }
    }

    fn run_meta_insert_named(
        &mut self,
        map_register: u8,
        value: u8,
        meta_id: MetaId,
        name: ConstantIndex,
    ) -> InstructionResult {
        let value = self.clone_register(value);
        let meta_key = match meta_id_to_key(meta_id, Some(self.get_constant_str(name))) {
            Ok(meta_key) => meta_key,
            Err(error) => return runtime_error!("MetaInsert: {}", error),
        };

        match self.get_register_mut(map_register) {
            Value::Map(map) => {
                map.contents_mut().meta.insert(meta_key, value);
                Ok(())
            }
            unexpected => runtime_error!(
                "MetaInsert: Expected Map, found '{}'",
                unexpected.type_as_string()
            ),
        }
    }

    fn run_access(
        &mut self,
        result_register: u8,
        map_register: u8,
        key: ConstantIndex,
    ) -> InstructionResult {
        use Value::*;

        let map_value = self.clone_register(map_register);
        let key_string = self.get_constant_str(key);

        macro_rules! core_op {
            ($module:ident, $iterator_fallback:expr) => {{
                let op = self.get_core_op(
                    key_string,
                    &self.context_shared.core_lib.$module,
                    stringify!($module),
                    $iterator_fallback,
                )?;
                self.set_register(result_register, op);
            }};
        }

        match map_value {
            Map(map) => match map.contents().data.get_with_string(&key_string) {
                Some(value) => {
                    self.set_register(result_register, value.clone());
                }
                None => core_op!(map, true),
            },
            List(_) => core_op!(list, true),
            Num2(_) => core_op!(num2, false),
            Num4(_) => core_op!(num4, false),
            Number(_) => core_op!(number, false),
            Range(_) => core_op!(range, true),
            Str(_) => core_op!(string, true),
            Tuple(_) => core_op!(tuple, true),
            Iterator(_) => core_op!(iterator, false),
            unexpected => {
                return self.unexpected_type_error("MapAccess: Expected Map", &unexpected)
            }
        }

        Ok(())
    }

    fn get_core_op(
        &self,
        key: &str,
        module: &ValueMap,
        module_name: &str,
        iterator_fallback: bool,
    ) -> RuntimeResult {
        use Value::*;

        let maybe_op = match module.contents().data.get_with_string(key).cloned() {
            None if iterator_fallback => self
                .context_shared
                .core_lib
                .iterator
                .contents()
                .data
                .get_with_string(&key)
                .cloned(),
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
                Function(f) => {
                    let f_as_instance_function = RuntimeFunction {
                        instance_function: true,
                        ..f
                    };
                    Function(f_as_instance_function)
                }
                Generator(f) => {
                    let f_as_instance_function = RuntimeFunction {
                        instance_function: true,
                        ..f
                    };
                    Generator(f_as_instance_function)
                }
                other => other,
            },
            None => return runtime_error!("'{}' not found in module '{}'", key, module_name),
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

        let result = (&*function)(
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
        function: RuntimeFunction,
        frame_base: u8,
        call_arg_count: u8,
        instance_register: Option<u8>,
    ) -> InstructionResult {
        let RuntimeFunction {
            chunk,
            ip: function_ip,
            arg_count: function_arg_count,
            instance_function,
            variadic,
            captures,
        } = function;

        // Spawn a VM for the generator
        let mut generator_vm = self.spawn_shared_vm();
        // Push a frame for running the generator function
        generator_vm.push_frame(
            chunk,
            function_ip,
            0, // arguments will be copied starting in register 0
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

        // Check for variadic arguments, and validate argument count
        if variadic {
            if call_arg_count >= expected_arg_count {
                // Capture the varargs into a tuple and place them in the
                // generator vm's last arg register
                let varargs_start = frame_base + 1 + expected_arg_count;
                let varargs_count = call_arg_count - expected_arg_count;
                let varargs =
                    Value::Tuple(self.register_slice(varargs_start, varargs_count).into());
                generator_vm.set_register(expected_arg_count + arg_offset, varargs);
            } else {
                return runtime_error!(
                    "Insufficient arguments for function call, expected {}, found {}",
                    expected_arg_count,
                    call_arg_count,
                );
            }
        } else if call_arg_count != expected_arg_count {
            return runtime_error!(
                "Incorrect argument count, expected {}, found {}",
                expected_arg_count,
                call_arg_count,
            );
        }

        // Copy any regular (non-instance, non-variadic) arguments into the generator vm
        for (arg_index, arg) in self
            .register_slice(frame_base + 1, expected_arg_count)
            .iter()
            .cloned()
            .enumerate()
        {
            generator_vm.set_register(arg_index as u8 + arg_offset, arg);
        }

        if let Some(captures) = captures {
            // Copy the function's captures into the generator vm
            let capture_offset = arg_offset + expected_arg_count;
            for (capture_index, capture) in captures.data().iter().cloned().enumerate() {
                generator_vm.set_register(capture_index as u8 + capture_offset, capture);
            }
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
        function: Value,
        frame_base: u8,
        call_arg_count: u8,
        instance_register: Option<u8>,
    ) -> InstructionResult {
        use Value::*;

        match function {
            ExternalFunction(external_function) => self.call_external_function(
                result_register,
                external_function,
                frame_base,
                call_arg_count,
                instance_register,
            ),
            Generator(runtime_function) => self.call_generator(
                result_register,
                runtime_function,
                frame_base,
                call_arg_count,
                instance_register,
            ),
            Function(RuntimeFunction {
                chunk,
                ip: function_ip,
                arg_count: function_arg_count,
                instance_function,
                variadic,
                captures,
            }) => {
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

                if variadic {
                    if call_arg_count >= expected_arg_count {
                        // The last defined arg is the start of the var_args,
                        // e.g. f = |x, y, z...|
                        // arg index 2 is the first vararg, and where the tuple will be placed
                        let arg_base = frame_base + 1;
                        let varargs_start = arg_base + expected_arg_count;
                        let varargs_count = call_arg_count - expected_arg_count;
                        let varargs =
                            Value::Tuple(self.register_slice(varargs_start, varargs_count).into());
                        self.set_register(varargs_start, varargs);
                        self.truncate_registers(varargs_start + 1);
                    } else {
                        return runtime_error!(
                            "Insufficient arguments for function call, expected {}, found {}",
                            expected_arg_count,
                            call_arg_count,
                        );
                    }
                } else if call_arg_count != expected_arg_count {
                    return runtime_error!(
                        "Incorrect argument count, expected {}, found {}",
                        expected_arg_count,
                        call_arg_count,
                    );
                }

                if let Some(captures) = captures {
                    // Ensure that the value stack is initialized to the end of the args,
                    // so that the captures can be directly copied to the correct position.
                    // Q: Why would the stack need to be truncated?
                    // A: Registers aren't automatically cleaned up during execution.
                    // Q: Why would the stack need to be extended?
                    // A: If there are no args then the frame base may have been left
                    //    uninitialized, so using .extend() here for the captures would place them
                    //    in the wrong position.
                    let captures_start =
                        self.register_index(adjusted_frame_base + function_arg_count);
                    self.value_stack.resize(captures_start, Value::Empty);
                    self.value_stack.extend(captures.data().iter().cloned());
                }

                if !self.call_stack.is_empty() {
                    // Set info for when the current frame is returned to
                    self.frame_mut().return_register_and_ip = Some((result_register, self.ip()));
                }

                // Set up a new frame for the called function
                self.push_frame(chunk, function_ip, adjusted_frame_base);

                Ok(())
            }
            unexpected => self.unexpected_type_error("Expected Function", &unexpected),
        }
    }

    fn run_debug(
        &mut self,
        register: u8,
        expression_constant: ConstantIndex,
        instruction_ip: usize,
    ) -> InstructionResult {
        let value = self.clone_register(register);
        let vm = self.child_vm();
        let value_string = match vm.run_unary_op(UnaryOp::Display, value)? {
            result @ Value::Str(_) => result,
            other => {
                return runtime_error!(
                    "debug: Expected string to display, found '{}'",
                    other.type_as_string()
                )
            }
        };

        let prefix = match (
            self.reader.chunk.debug_info.get_source_span(instruction_ip),
            self.reader.chunk.source_path.as_ref(),
        ) {
            (Some(span), Some(path)) => format!("[{}: {}] ", path.display(), span.start.line),
            (Some(span), None) => format!("[{}] ", span.start.line),
            (None, Some(path)) => format!("[{}: #ERR] ", path.display()),
            (None, None) => "[#ERR] ".to_string(),
        };

        let expression_string = self.get_constant_str(expression_constant);

        self.logger().writeln(&format!(
            "{}{}: {}",
            prefix, expression_string, value_string
        ));

        Ok(())
    }

    fn run_check_type(&self, register: u8, type_id: TypeId) -> Result<(), RuntimeError> {
        let value = self.get_register(register);
        match type_id {
            TypeId::List => {
                if !matches!(value, Value::List(_)) {
                    return self.unexpected_type_error("Expected List", &value);
                }
            }
            TypeId::Tuple => {
                if !matches!(value, Value::Tuple(_)) {
                    return self.unexpected_type_error("Expected Tuple", &value);
                }
            }
        }
        Ok(())
    }

    fn run_check_size(&self, register: u8, expected_size: usize) -> Result<(), RuntimeError> {
        let value_size = self.get_register(register).size();

        if value_size == expected_size {
            Ok(())
        } else {
            runtime_error!(
                "Value has a size of '{}', expected '{}'",
                value_size,
                expected_size
            )
        }
    }

    pub fn chunk(&self) -> Arc<Chunk> {
        self.reader.chunk.clone()
    }

    fn set_chunk_and_ip(&mut self, chunk: Arc<Chunk>, ip: usize) {
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

    fn push_frame(&mut self, chunk: Arc<Chunk>, ip: usize, frame_base: u8) {
        let previous_frame_base = if let Some(frame) = self.call_stack.last() {
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

        if self.call_stack.pop().is_none() {
            return runtime_error!("pop_frame: Empty call stack");
        };

        if !self.call_stack.is_empty() && self.frame().return_register_and_ip.is_some() {
            let (return_register, return_ip) = self.frame().return_register_and_ip.unwrap();

            self.set_register(return_register, return_value);
            self.set_chunk_and_ip(self.frame().chunk.clone(), return_ip);

            Ok(None)
        } else {
            Ok(Some(return_value))
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

    fn set_register(&mut self, register: u8, value: Value) {
        let index = self.register_index(register);

        if index >= self.value_stack.len() {
            self.value_stack.resize(index + 1, Value::Empty);
        }

        self.value_stack[index] = value;
    }

    fn clone_register(&self, register: u8) -> Value {
        self.get_register(register).clone()
    }

    fn get_register(&self, register: u8) -> &Value {
        let index = self.register_index(register);
        match self.value_stack.get(index) {
            Some(value) => value,
            None => {
                panic!(
                    "Out of bounds access, index: {}, register: {}, ip: {}",
                    index,
                    register,
                    self.ip()
                );
            }
        }
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
        let bounds = self.reader.chunk.constants.get_str_bounds(constant_index);
        ValueString::new_with_bounds(self.reader.chunk.string_constants_arc.clone(), bounds)
            .unwrap() // The bounds have been already checked in the constant pool
    }

    fn unexpected_type_error<T>(&self, message: &str, value: &Value) -> Result<T, RuntimeError> {
        runtime_error!("{}, found '{}'", message, value.type_as_string())
    }

    fn binary_op_error(&self, lhs: &Value, rhs: &Value, op: &str) -> InstructionResult {
        runtime_error!(
            "Unable to perform operation '{}' with '{}' and '{}'",
            op,
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
        size - (index.abs() as usize).min(size)
    } else {
        index as usize
    }
}
