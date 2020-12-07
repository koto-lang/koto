use {
    crate::{
        core::CoreLib,
        external::{self, Args, ExternalFunction},
        frame::Frame,
        num2, num4, type_as_string,
        value::{self, RegisterSlice, RuntimeFunction},
        value_iterator::{IntRange, Iterable, ValueIterator, ValueIteratorOutput},
        vm_error, Error, Loader, RuntimeResult, Value, ValueList, ValueMap, ValueString, ValueVec,
    },
    koto_bytecode::{Chunk, Instruction, InstructionReader},
    koto_parser::ConstantIndex,
    std::{
        collections::HashMap,
        fmt,
        path::PathBuf,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, RwLock, RwLockReadGuard, RwLockWriteGuard,
        },
    },
};

#[derive(Clone, Debug)]
pub enum ControlFlow {
    Continue,
    Return(Value),
    Yield(Value),
}

// Instructions will place their results in registers, there's no Ok type
pub type InstructionResult = Result<(), Error>;

pub struct VmContext {
    pub prelude: ValueMap,
    core_lib: CoreLib,
    global: ValueMap,
    loader: Loader,
    modules: HashMap<PathBuf, Option<ValueMap>>,
    spawned_stop_flags: Vec<Arc<AtomicBool>>,
}

impl Default for VmContext {
    fn default() -> Self {
        let core_lib = CoreLib::default();

        let mut prelude = ValueMap::default();
        prelude.add_map("io", core_lib.io.clone());
        prelude.add_map("iterator", core_lib.iterator.clone());
        prelude.add_map("koto", core_lib.koto.clone());
        prelude.add_map("list", core_lib.list.clone());
        prelude.add_map("map", core_lib.map.clone());
        prelude.add_map("number", core_lib.number.clone());
        prelude.add_map("range", core_lib.range.clone());
        prelude.add_map("string", core_lib.string.clone());
        prelude.add_map("test", core_lib.test.clone());
        prelude.add_map("thread", core_lib.thread.clone());
        prelude.add_map("tuple", core_lib.tuple.clone());

        Self {
            prelude,
            core_lib,
            global: Default::default(),
            loader: Default::default(),
            modules: Default::default(),
            spawned_stop_flags: Default::default(),
        }
    }
}

impl VmContext {
    fn spawn_new_context(&self) -> Self {
        Self {
            prelude: self.prelude.clone(),
            core_lib: self.core_lib.clone(),
            loader: self.loader.clone(),
            modules: self.modules.clone(),
            global: Default::default(),
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

impl Drop for VmContext {
    fn drop(&mut self) {
        self.stop_spawned_vms();
    }
}

pub struct Vm {
    context: Arc<RwLock<VmContext>>,
    reader: InstructionReader,
    value_stack: Vec<Value>,
    call_stack: Vec<Frame>,
    stop_flag: Option<Arc<AtomicBool>>,
}

impl Default for Vm {
    fn default() -> Self {
        Self {
            context: Arc::new(RwLock::new(VmContext::default())),
            reader: InstructionReader::default(),
            value_stack: Vec::with_capacity(32),
            call_stack: vec![],
            stop_flag: None,
        }
    }
}

impl Vm {
    pub fn spawn_new_vm(&mut self) -> Self {
        Self {
            context: Arc::new(RwLock::new(self.context().spawn_new_context())),
            reader: InstructionReader::default(),
            value_stack: Vec::with_capacity(32),
            call_stack: vec![],
            stop_flag: None,
        }
    }

    pub fn spawn_shared_vm(&mut self) -> Self {
        Self {
            context: self.context.clone(),
            reader: self.reader.clone(),
            value_stack: Vec::with_capacity(32),
            call_stack: vec![],
            stop_flag: None,
        }
    }

    pub fn spawn_shared_concurrent_vm(&mut self) -> Self {
        let stop_flag = Arc::new(AtomicBool::new(false));
        self.context_mut()
            .spawned_stop_flags
            .push(stop_flag.clone());

        Self {
            context: self.context.clone(),
            reader: self.reader.clone(),
            value_stack: Vec::with_capacity(32),
            call_stack: vec![],
            stop_flag: Some(stop_flag),
        }
    }

    pub fn context(&self) -> RwLockReadGuard<VmContext> {
        self.context.read().unwrap()
    }

    pub fn context_mut(&mut self) -> RwLockWriteGuard<VmContext> {
        self.context.write().unwrap()
    }

    pub fn get_global_value(&self, id: &str) -> Option<Value> {
        self.context().global.data().get_with_string(id).cloned()
    }

    pub fn get_global_function(&self, id: &str) -> Option<RuntimeFunction> {
        match self.get_global_value(id) {
            Some(Value::Function(function)) => Some(function),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.context_mut().reset();
        self.value_stack = Default::default();
        self.call_stack = Default::default();
    }

    pub fn run(&mut self, chunk: Arc<Chunk>) -> RuntimeResult {
        self.push_frame(chunk, 0, 0, None);
        self.execute_instructions()
    }

    pub fn continue_running(&mut self) -> RuntimeResult {
        if self.call_stack.is_empty() {
            Ok(Value::Empty)
        } else {
            self.execute_instructions()
        }
    }

    pub fn run_function(&mut self, function: &RuntimeFunction, args: &[Value]) -> RuntimeResult {
        if !self.call_stack.is_empty() {
            return vm_error!(
                self.chunk(),
                self.ip(),
                "run_function: the call stack isn't empty"
            );
        }

        let current_chunk = self.chunk();
        let current_ip = self.ip();

        if args.len() as u8 != function.arg_count {
            return vm_error!(
                self.chunk(),
                self.ip(),
                "Incorrect argument count, expected {}, found {}",
                function.arg_count,
                args.len(),
            );
        }

        let frame_base = if let Some(frame) = self.call_stack.last() {
            frame.register_base
        } else {
            0
        };

        let arg_register = (self.value_stack.len() - frame_base) as u8;
        self.value_stack.extend_from_slice(args);

        self.push_frame(
            function.chunk.clone(),
            function.ip,
            arg_register,
            function.captures.clone(),
        );

        self.frame_mut().catch_barrier = true;

        let result = self.execute_instructions();
        if result.is_err() {
            self.pop_frame(Value::Empty)?;
        }

        self.set_chunk_and_ip(current_chunk, current_ip);

        result
    }

    pub fn run_tests(&mut self, tests: ValueMap) -> RuntimeResult {
        // It's important here to make sure we don't hang on to any references to the internal
        // test map data while calling the test functions, otherwise we'll end up in deadlocks.
        let self_arg = [Value::Map(tests.clone())];

        let pre_test = tests.data().get_with_string("pre_test").cloned();
        let post_test = tests.data().get_with_string("post_test").cloned();

        for (key, value) in tests.cloned_iter() {
            match (key, value) {
                (Value::Str(id), Value::Function(test)) if id.starts_with("test_") => {
                    let make_test_error = |error, message: &str| {
                        Err(Error::TestError {
                            message: format!("{} '{}'", message, &id[5..]),
                            error: Box::new(error),
                        })
                    };

                    if let Some(Value::Function(pre_test)) = &pre_test {
                        let pre_test_result = match pre_test.arg_count {
                            0 => self.run_function(&pre_test.clone(), &[]),
                            _ => self.run_function(&pre_test.clone(), &self_arg),
                        };

                        if let Err(error) = pre_test_result {
                            return make_test_error(error, "Error while preparing to run test");
                        }
                    }

                    let test_result = match test.arg_count {
                        0 => self.run_function(&test, &[]),
                        _ => self.run_function(&test, &self_arg),
                    };

                    if let Err(error) = test_result {
                        return make_test_error(error, "Error while running test");
                    }

                    if let Some(Value::Function(post_test)) = &post_test {
                        let post_test_result = match post_test.arg_count {
                            0 => self.run_function(&post_test.clone(), &[]),
                            _ => self.run_function(&post_test.clone(), &self_arg),
                        };

                        if let Err(error) = post_test_result {
                            return make_test_error(error, "Error after running test");
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(Value::Empty)
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
                Err(error) => {
                    let mut recover_register_and_ip = None;

                    while let Some(frame) = self.call_stack.last() {
                        if let Some((error_register, catch_ip)) = frame.catch_stack.last() {
                            recover_register_and_ip = Some((*error_register, *catch_ip));
                            break;
                        } else {
                            if frame.catch_barrier {
                                return Err(error);
                            }

                            self.pop_frame(Value::Empty)?;
                        }
                    }

                    if let Some((register, ip)) = recover_register_and_ip {
                        self.set_register(register, Value::Str(error.to_string().into()));
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
    ) -> Result<ControlFlow, Error> {
        use Value::*;

        let mut control_flow = ControlFlow::Continue;

        match instruction {
            Instruction::Error { message } => {
                vm_error!(self.chunk(), instruction_ip, "{}", message)
            }
            Instruction::Copy { target, source } => self.run_copy(target, source),
            Instruction::SetEmpty { register } => {
                self.set_register(register, Empty);
                Ok(())
            }
            Instruction::SetBool { register, value } => {
                self.set_register(register, Bool(value));
                Ok(())
            }
            Instruction::SetNumber { register, value } => {
                self.set_register(register, Number(value));
                Ok(())
            }
            Instruction::LoadNumber { register, constant } => {
                let n = self.reader.chunk.constants.get_number(constant);
                self.set_register(register, Number(n));
                Ok(())
            }
            Instruction::LoadString { register, constant } => {
                let string = self.value_string_from_constant(constant);
                self.set_register(register, Str(string));
                Ok(())
            }
            Instruction::LoadGlobal { register, constant } => {
                self.run_load_global(register, constant, instruction_ip)
            }
            Instruction::SetGlobal { global, source } => self.run_set_global(global, source),
            Instruction::Import { register, constant } => {
                self.run_import(register, constant, instruction_ip)
            }
            Instruction::MakeTuple {
                register,
                start,
                count,
            } => self.run_make_tuple(register, start, count),
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
            } => self.run_make_num2(register, count, element_register, instruction_ip),
            Instruction::MakeNum4 {
                register,
                count,
                element_register,
            } => self.run_make_num4(register, count, element_register, instruction_ip),
            Instruction::Range {
                register,
                start,
                end,
            } => self.run_make_range(register, Some(start), Some(end), false, instruction_ip),
            Instruction::RangeInclusive {
                register,
                start,
                end,
            } => self.run_make_range(register, Some(start), Some(end), true, instruction_ip),
            Instruction::RangeTo { register, end } => {
                self.run_make_range(register, None, Some(end), false, instruction_ip)
            }
            Instruction::RangeToInclusive { register, end } => {
                self.run_make_range(register, None, Some(end), true, instruction_ip)
            }
            Instruction::RangeFrom { register, start } => {
                self.run_make_range(register, Some(start), None, false, instruction_ip)
            }
            Instruction::RangeFull { register } => {
                self.run_make_range(register, None, None, false, instruction_ip)
            }
            Instruction::MakeIterator { register, iterable } => {
                self.run_make_iterator(register, iterable, instruction_ip)
            }
            Instruction::Function { .. } => self.run_make_function(instruction),
            Instruction::Capture {
                function,
                target,
                source,
            } => self.run_capture_value(function, target, source, instruction_ip),
            Instruction::LoadCapture { register, capture } => {
                self.run_load_capture(register, capture, instruction_ip)
            }
            Instruction::SetCapture { capture, source } => {
                self.run_set_capture(capture, source, instruction_ip)
            }
            Instruction::Negate { register, source } => {
                self.run_negate(register, source, instruction_ip)
            }
            Instruction::Add { register, lhs, rhs } => {
                self.run_add(register, lhs, rhs, &instruction, instruction_ip)
            }
            Instruction::Subtract { register, lhs, rhs } => {
                self.run_subtract(register, lhs, rhs, &instruction, instruction_ip)
            }
            Instruction::Multiply { register, lhs, rhs } => {
                self.run_multiply(register, lhs, rhs, &instruction, instruction_ip)
            }
            Instruction::Divide { register, lhs, rhs } => {
                self.run_divide(register, lhs, rhs, &instruction, instruction_ip)
            }
            Instruction::Modulo { register, lhs, rhs } => {
                self.run_modulo(register, lhs, rhs, &instruction, instruction_ip)
            }
            Instruction::Less { register, lhs, rhs } => {
                self.run_less(register, lhs, rhs, &instruction, instruction_ip)
            }
            Instruction::LessOrEqual { register, lhs, rhs } => {
                self.run_less_or_equal(register, lhs, rhs, &instruction, instruction_ip)
            }
            Instruction::Greater { register, lhs, rhs } => {
                self.run_greater(register, lhs, rhs, &instruction, instruction_ip)
            }
            Instruction::GreaterOrEqual { register, lhs, rhs } => {
                self.run_greater_or_equal(register, lhs, rhs, &instruction, instruction_ip)
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
            } => self.run_jump_if(register, offset, jump_condition, instruction_ip),
            Instruction::JumpBack { offset } => {
                self.jump_ip_back(offset);
                Ok(())
            }
            Instruction::JumpBackIf {
                register,
                offset,
                jump_condition,
            } => self.run_jump_back_if(register, offset, jump_condition, instruction_ip),
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
                instruction_ip,
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
                instruction_ip,
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
            Instruction::Size { register, value } => self.run_size(register, value),
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
            } => {
                self.run_iterator_next(Some(register), iterator, jump_offset, false, instruction_ip)
            }
            Instruction::IterNextTemp {
                register,
                iterator,
                jump_offset,
            } => {
                self.run_iterator_next(Some(register), iterator, jump_offset, true, instruction_ip)
            }
            Instruction::IterNextQuiet {
                iterator,
                jump_offset,
            } => self.run_iterator_next(None, iterator, jump_offset, false, instruction_ip),
            Instruction::ValueIndex {
                register,
                value,
                index,
            } => self.run_value_index(register, value, index, instruction_ip),
            Instruction::SliceFrom {
                register,
                value,
                index,
            } => self.run_slice(register, value, index, false, instruction_ip),
            Instruction::SliceTo {
                register,
                value,
                index,
            } => self.run_slice(register, value, index, true, instruction_ip),
            Instruction::ListPushValue { list, value } => {
                self.run_list_push(list, value, instruction_ip)
            }
            Instruction::ListPushValues {
                list,
                values_start,
                count,
            } => {
                for value_register in values_start..(values_start + count) {
                    self.run_list_push(list, value_register, instruction_ip)?;
                }
                Ok(())
            }
            Instruction::ListUpdate { list, index, value } => {
                self.run_list_update(list, index, value, instruction_ip)
            }
            Instruction::Index {
                register,
                value,
                index,
            } => self.run_index(register, value, index, instruction_ip),
            Instruction::MapInsert {
                register,
                value,
                key,
            } => self.run_map_insert(register, value, key, instruction_ip),
            Instruction::Access { register, map, key } => {
                self.run_access(register, map, key, instruction_ip)
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
            Instruction::Debug { register, constant } => {
                self.run_debug(register, constant, instruction_ip)
            }
        }?;

        Ok(control_flow)
    }

    fn run_copy(&mut self, target: u8, source: u8) -> InstructionResult {
        let value = match self.clone_register(source) {
            Value::TemporaryTuple(RegisterSlice { start, count }) => {
                // A temporary tuple shouldn't make it into a named value,
                // so here it gets converted into a regular tuple.
                Value::Tuple(self.register_slice(start, count).into())
            }
            other => other,
        };
        self.set_register(target, value);
        Ok(())
    }

    fn run_load_global(
        &mut self,
        register: u8,
        constant_index: ConstantIndex,
        instruction_ip: usize,
    ) -> InstructionResult {
        let global_name = self.get_constant_str(constant_index);
        let global = self
            .context()
            .global
            .data()
            .get_with_string(global_name)
            .cloned();

        match global {
            Some(value) => {
                self.set_register(register, value);
                Ok(())
            }
            None => vm_error!(self.chunk(), instruction_ip, "'{}' not found", global_name),
        }
    }

    fn run_set_global(
        &mut self,
        constant_index: ConstantIndex,
        source_register: u8,
    ) -> InstructionResult {
        let global_name = Value::Str(self.value_string_from_constant(constant_index));
        let value = self.clone_register(source_register);
        self.context_mut()
            .global
            .data_mut()
            .insert(global_name, value);
        Ok(())
    }

    fn run_make_tuple(&mut self, register: u8, start: u8, count: u8) -> InstructionResult {
        let mut copied = Vec::with_capacity(count as usize);

        for register in start..start + count {
            copied.push(self.clone_register(register));
        }

        self.set_register(register, Value::Tuple(copied.into()));
        Ok(())
    }

    fn run_make_range(
        &mut self,
        register: u8,
        start_register: Option<u8>,
        end_register: Option<u8>,
        inclusive: bool,
        instruction_ip: usize,
    ) -> InstructionResult {
        use Value::{IndexRange, Number, Range};

        let start = start_register.map(|register| self.get_register(register));
        let end = end_register.map(|register| self.get_register(register));

        let range = match (start, end) {
            (Some(Number(start)), Some(Number(end))) => {
                let (start, end) = if inclusive {
                    if start <= end {
                        (*start as isize, *end as isize + 1)
                    } else {
                        (*start as isize, *end as isize - 1)
                    }
                } else {
                    (*start as isize, *end as isize)
                };

                Range(IntRange { start, end })
            }
            (None, Some(Number(end))) => {
                if *end < 0.0 {
                    return vm_error!(
                        self.chunk(),
                        instruction_ip,
                        "RangeTo: negative numbers not allowed, found '{}'",
                        end
                    );
                }
                let end = if inclusive {
                    *end as usize + 1
                } else {
                    *end as usize
                };
                IndexRange(value::IndexRange {
                    start: 0,
                    end: Some(end),
                })
            }
            (Some(Number(start)), None) => {
                if *start < 0.0 {
                    return vm_error!(
                        self.chunk(),
                        instruction_ip,
                        "RangeFrom: negative numbers not allowed, found '{}'",
                        start
                    );
                }
                IndexRange(value::IndexRange {
                    start: *start as usize,
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
                return self.unexpected_type_error(
                    "Expected Number for range start",
                    unexpected,
                    instruction_ip,
                );
            }
            (_, Some(unexpected)) => {
                return self.unexpected_type_error(
                    "Expected Number for range end",
                    unexpected,
                    instruction_ip,
                );
            }
        };

        self.set_register(register, range);
        Ok(())
    }

    fn run_make_iterator(
        &mut self,
        register: u8,
        iterable_register: u8,
        instruction_ip: usize,
    ) -> InstructionResult {
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
                        instruction_ip,
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
        instruction_ip: usize,
    ) -> InstructionResult {
        use Value::{Iterator, TemporaryTuple, Tuple};

        let result = match self.get_register_mut(iterator) {
            Iterator(iterator) => iterator.next(),
            unexpected => {
                return vm_error!(
                    self.chunk(),
                    instruction_ip,
                    "Expected Iterator, found '{}'",
                    type_as_string(unexpected)
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
            (Some(Err(error)), _) => match error {
                Error::ErrorWithoutLocation { message } => {
                    return vm_error!(self.chunk(), instruction_ip, message)
                }
                _ => return Err(error),
            },
            (None, _) => self.jump_ip(jump_offset),
        };

        Ok(())
    }

    fn run_value_index(
        &mut self,
        register: u8,
        value: u8,
        index: i8,
        instruction_ip: usize,
    ) -> InstructionResult {
        use Value::*;

        match self.get_register(value) {
            List(list) => {
                let index = signed_index_to_unsigned(index, list.data().len());
                let result = list.data().get(index).cloned().unwrap_or(Empty);
                self.set_register(register, result);
            }
            Tuple(tuple) => {
                let index = signed_index_to_unsigned(index, tuple.data().len());
                let result = tuple.data().get(index).cloned().unwrap_or(Empty);
                self.set_register(register, result);
            }
            TemporaryTuple(RegisterSlice { start, count }) => {
                let count = *count;
                let result = if (index.abs() as u8) < count {
                    let index = signed_index_to_unsigned(index, count as usize);
                    self.clone_register(start + index as u8)
                } else {
                    Empty
                };
                self.set_register(register, result);
            }
            unexpected => {
                return self.unexpected_type_error(
                    "ValueIndex: Expected indexable value",
                    unexpected,
                    instruction_ip,
                );
            }
        };

        Ok(())
    }

    fn run_slice(
        &mut self,
        register: u8,
        value: u8,
        index: i8,
        is_slice_to: bool,
        instruction_ip: usize,
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
                return self.unexpected_type_error(
                    "SliceFrom: expected List or Tuple",
                    unexpected,
                    instruction_ip,
                );
            }
        };

        self.set_register(register, result);

        Ok(())
    }

    fn run_make_function(&mut self, function_instruction: Instruction) -> InstructionResult {
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

        Ok(())
    }

    fn run_capture_value(
        &mut self,
        function: u8,
        capture_index: u8,
        value: u8,
        instruction_ip: usize,
    ) -> InstructionResult {
        match self.get_register(function) {
            Value::Function(f) => match &f.captures {
                Some(captures) => {
                    captures.data_mut()[capture_index as usize] = self.clone_register(value)
                }
                None => {
                    return vm_error!(
                        self.chunk(),
                        instruction_ip,
                        "Capture: missing capture list for function"
                    )
                }
            },
            unexpected => {
                return self.unexpected_type_error(
                    "Capture: expected Function",
                    unexpected,
                    instruction_ip,
                );
            }
        }

        Ok(())
    }

    fn run_load_capture(
        &mut self,
        register: u8,
        capture_index: u8,
        instruction_ip: usize,
    ) -> InstructionResult {
        match self.frame().get_capture(capture_index) {
            Some(value) => {
                self.set_register(register, value);
            }
            None => {
                if self.call_stack.len() == 1 {
                    return vm_error!(
                        self.chunk(),
                        instruction_ip,
                        "LoadCapture: attempting to capture outside of function"
                    );
                } else {
                    return vm_error!(
                        self.chunk(),
                        instruction_ip,
                        "LoadCapture: invalid capture index"
                    );
                }
            }
        }

        Ok(())
    }

    fn run_set_capture(
        &mut self,
        capture_index: u8,
        value_register: u8,
        instruction_ip: usize,
    ) -> InstructionResult {
        let value = self.clone_register(value_register);

        if !self.frame_mut().set_capture(capture_index, value) {
            return vm_error!(
                self.chunk(),
                instruction_ip,
                "SetCapture: invalid capture index: {} ",
                capture_index
            );
        }

        Ok(())
    }

    fn run_negate(&mut self, register: u8, value: u8, instruction_ip: usize) -> InstructionResult {
        use Value::*;

        let result = match &self.get_register(value) {
            Bool(b) => Bool(!b),
            Number(n) => Number(-n),
            Num2(v) => Num2(-v),
            Num4(v) => Num4(-v),
            unexpected => {
                return self.unexpected_type_error(
                    "Negate: expected negatable value",
                    unexpected,
                    instruction_ip,
                );
            }
        };
        self.set_register(register, result);

        Ok(())
    }

    fn run_add(
        &mut self,
        register: u8,
        lhs: u8,
        rhs: u8,
        instruction: &Instruction,
        instruction_ip: usize,
    ) -> InstructionResult {
        use Value::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result = match (lhs_value, rhs_value) {
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
            (Map(a), Map(b)) => {
                let mut result = a.data().clone();
                result.extend(&b.data());
                Map(ValueMap::with_data(result))
            }
            (Str(a), Str(b)) => {
                let result = a.to_string() + b.as_ref();
                Str(result.into())
            }
            _ => {
                return self.binary_op_error(lhs_value, rhs_value, instruction, instruction_ip);
            }
        };
        self.set_register(register, result);

        Ok(())
    }

    fn run_subtract(
        &mut self,
        register: u8,
        lhs: u8,
        rhs: u8,
        instruction: &Instruction,
        instruction_ip: usize,
    ) -> InstructionResult {
        use Value::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Number(a - b),
            (Number(a), Num2(b)) => Num2(a - b),
            (Num2(a), Num2(b)) => Num2(a - b),
            (Num2(a), Number(b)) => Num2(a - b),
            (Number(a), Num4(b)) => Num4(a - b),
            (Num4(a), Num4(b)) => Num4(a - b),
            (Num4(a), Number(b)) => Num4(a - b),
            _ => {
                return self.binary_op_error(lhs_value, rhs_value, instruction, instruction_ip);
            }
        };
        self.set_register(register, result);

        Ok(())
    }

    fn run_multiply(
        &mut self,
        register: u8,
        lhs: u8,
        rhs: u8,
        instruction: &Instruction,
        instruction_ip: usize,
    ) -> InstructionResult {
        use Value::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Number(a * b),
            (Number(a), Num2(b)) => Num2(a * b),
            (Num2(a), Num2(b)) => Num2(a * b),
            (Num2(a), Number(b)) => Num2(a * b),
            (Number(a), Num4(b)) => Num4(a * b),
            (Num4(a), Num4(b)) => Num4(a * b),
            (Num4(a), Number(b)) => Num4(a * b),
            _ => {
                return self.binary_op_error(lhs_value, rhs_value, instruction, instruction_ip);
            }
        };
        self.set_register(register, result);

        Ok(())
    }

    fn run_divide(
        &mut self,
        register: u8,
        lhs: u8,
        rhs: u8,
        instruction: &Instruction,
        instruction_ip: usize,
    ) -> InstructionResult {
        use Value::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Number(a / b),
            (Number(a), Num2(b)) => Num2(a / b),
            (Num2(a), Num2(b)) => Num2(a / b),
            (Num2(a), Number(b)) => Num2(a / b),
            (Number(a), Num4(b)) => Num4(a / b),
            (Num4(a), Num4(b)) => Num4(a / b),
            (Num4(a), Number(b)) => Num4(a / b),
            _ => {
                return self.binary_op_error(lhs_value, rhs_value, instruction, instruction_ip);
            }
        };
        self.set_register(register, result);

        Ok(())
    }

    fn run_modulo(
        &mut self,
        register: u8,
        lhs: u8,
        rhs: u8,
        instruction: &Instruction,
        instruction_ip: usize,
    ) -> InstructionResult {
        use Value::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Number(a % b),
            (Number(a), Num2(b)) => Num2(a % b),
            (Num2(a), Num2(b)) => Num2(a % b),
            (Num2(a), Number(b)) => Num2(a % b),
            (Number(a), Num4(b)) => Num4(a % b),
            (Num4(a), Num4(b)) => Num4(a % b),
            (Num4(a), Number(b)) => Num4(a % b),
            _ => {
                return self.binary_op_error(lhs_value, rhs_value, instruction, instruction_ip);
            }
        };
        self.set_register(register, result);

        Ok(())
    }

    fn run_less(
        &mut self,
        register: u8,
        lhs: u8,
        rhs: u8,
        instruction: &Instruction,
        instruction_ip: usize,
    ) -> InstructionResult {
        use Value::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Bool(a < b),
            (Str(a), Str(b)) => Bool(a.as_str() < b.as_str()),
            _ => {
                return self.binary_op_error(lhs_value, rhs_value, instruction, instruction_ip);
            }
        };
        self.set_register(register, result);

        Ok(())
    }

    fn run_less_or_equal(
        &mut self,
        register: u8,
        lhs: u8,
        rhs: u8,
        instruction: &Instruction,
        instruction_ip: usize,
    ) -> InstructionResult {
        use Value::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Bool(a <= b),
            (Str(a), Str(b)) => Bool(a.as_str() <= b.as_str()),
            _ => {
                return self.binary_op_error(lhs_value, rhs_value, instruction, instruction_ip);
            }
        };
        self.set_register(register, result);

        Ok(())
    }

    fn run_greater(
        &mut self,
        register: u8,
        lhs: u8,
        rhs: u8,
        instruction: &Instruction,
        instruction_ip: usize,
    ) -> InstructionResult {
        use Value::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Bool(a > b),
            (Str(a), Str(b)) => Bool(a.as_str() > b.as_str()),
            _ => {
                return self.binary_op_error(lhs_value, rhs_value, instruction, instruction_ip);
            }
        };
        self.set_register(register, result);

        Ok(())
    }

    fn run_greater_or_equal(
        &mut self,
        register: u8,
        lhs: u8,
        rhs: u8,
        instruction: &Instruction,
        instruction_ip: usize,
    ) -> InstructionResult {
        use Value::*;

        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result = match (lhs_value, rhs_value) {
            (Number(a), Number(b)) => Bool(a >= b),
            (Str(a), Str(b)) => Bool(a.as_str() >= b.as_str()),
            _ => {
                return self.binary_op_error(lhs_value, rhs_value, instruction, instruction_ip);
            }
        };
        self.set_register(register, result);

        Ok(())
    }

    fn run_equal(&mut self, register: u8, lhs: u8, rhs: u8) -> InstructionResult {
        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result = (lhs_value == rhs_value).into();
        self.set_register(register, result);
        Ok(())
    }

    fn run_not_equal(&mut self, register: u8, lhs: u8, rhs: u8) -> InstructionResult {
        let lhs_value = self.get_register(lhs);
        let rhs_value = self.get_register(rhs);
        let result = (lhs_value != rhs_value).into();
        self.set_register(register, result);
        Ok(())
    }

    fn run_jump_if(
        &mut self,
        register: u8,
        offset: usize,
        jump_condition: bool,
        instruction_ip: usize,
    ) -> InstructionResult {
        match self.get_register(register) {
            Value::Bool(b) => {
                if *b == jump_condition {
                    self.jump_ip(offset);
                }
            }
            unexpected => {
                return self.unexpected_type_error(
                    "JumpIf: expected Bool",
                    unexpected,
                    instruction_ip,
                );
            }
        }
        Ok(())
    }

    fn run_jump_back_if(
        &mut self,
        register: u8,
        offset: usize,
        jump_condition: bool,
        instruction_ip: usize,
    ) -> InstructionResult {
        match self.get_register(register) {
            Value::Bool(b) => {
                if *b == jump_condition {
                    self.jump_ip_back(offset);
                }
            }
            unexpected => {
                return self.unexpected_type_error(
                    "JumpIf: expected Bool",
                    unexpected,
                    instruction_ip,
                );
            }
        }
        Ok(())
    }

    fn run_size(&mut self, register: u8, value: u8) -> InstructionResult {
        use Value::*;

        let result = match self.get_register(value) {
            List(l) => l.len(),
            Str(s) => s.len(),
            Tuple(t) => t.data().len(),
            TemporaryTuple(RegisterSlice { count, .. }) => *count as usize,
            Map(m) => m.len(),
            Num2(_) => 2,
            Num4(_) => 4,
            Range(IntRange { start, end }) => (end - start) as usize,
            _ => 1,
        };
        self.set_register(register, Number(result as f64));

        Ok(())
    }

    fn run_import(
        &mut self,
        result_register: u8,
        import_constant: ConstantIndex,
        instruction_ip: usize,
    ) -> InstructionResult {
        let import_name = self.value_string_from_constant(import_constant);

        let maybe_global = self
            .context()
            .global
            .data()
            .get_with_string(&import_name)
            .cloned();
        if let Some(value) = maybe_global {
            self.set_register(result_register, value);
        } else {
            let maybe_in_prelude = self
                .context()
                .prelude
                .data()
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
                    Err(e) => {
                        return vm_error!(
                            self.chunk(),
                            instruction_ip,
                            "Failed to import '{}': {}",
                            import_name,
                            e
                        )
                    }
                };
                let maybe_module = self.context().modules.get(&module_path).cloned();
                match maybe_module {
                    Some(Some(module)) => self.set_register(result_register, Value::Map(module)),
                    Some(None) => {
                        return vm_error!(
                            self.chunk(),
                            instruction_ip,
                            "Recursive import of module '{}'",
                            import_name
                        )
                    }
                    None => {
                        // Insert a placeholder for the new module, preventing recursive imports
                        self.context_mut().modules.insert(module_path.clone(), None);

                        // Run the module chunk
                        let mut vm = self.spawn_new_vm();
                        match vm.run(module_chunk) {
                            Ok(_) => {
                                if let Some(main) = vm.get_global_function("main") {
                                    if let Err(error) = vm.run_function(&main, &[]) {
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

                        // Cache the resulting module's global map
                        let module_global = vm.context().global.clone();
                        self.context_mut()
                            .modules
                            .insert(module_path, Some(module_global.clone()));

                        self.set_register(result_register, Value::Map(module_global));
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
        instruction_ip: usize,
    ) -> InstructionResult {
        use Value::*;

        let result = if element_count == 1 {
            match self.get_register(element_register) {
                Number(n) => num2::Num2(*n, *n),
                Num2(n) => *n,
                List(list) => {
                    let mut result = num2::Num2::default();
                    for (i, value) in list.data().iter().take(2).enumerate() {
                        match value {
                            Number(n) => result[i] = *n,
                            unexpected => {
                                return self.unexpected_type_error(
                                    "num2: Expected Number",
                                    unexpected,
                                    instruction_ip,
                                );
                            }
                        }
                    }
                    result
                }
                unexpected => {
                    return self.unexpected_type_error(
                        "num2: Expected Number, Num2, or List",
                        unexpected,
                        instruction_ip,
                    );
                }
            }
        } else {
            let mut result = num2::Num2::default();
            for i in 0..element_count {
                match self.get_register(element_register + i) {
                    Number(n) => result[i as usize] = *n,
                    unexpected => {
                        return self.unexpected_type_error(
                            "num2: Expected Number, Num2, or List",
                            unexpected,
                            instruction_ip,
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
        instruction_ip: usize,
    ) -> InstructionResult {
        use Value::*;
        let result = if element_count == 1 {
            match self.get_register(element_register) {
                Number(n) => {
                    let n = *n as f32;
                    num4::Num4(n, n, n, n)
                }
                Num2(n) => num4::Num4(n[0] as f32, n[1] as f32, 0.0, 0.0),
                Num4(n) => *n,
                List(list) => {
                    let mut result = num4::Num4::default();
                    for (i, value) in list.data().iter().take(4).enumerate() {
                        match value {
                            Number(n) => result[i] = *n as f32,
                            unexpected => {
                                return self.unexpected_type_error(
                                    "num4: Expected Number",
                                    unexpected,
                                    instruction_ip,
                                );
                            }
                        }
                    }
                    result
                }
                unexpected => {
                    return self.unexpected_type_error(
                        "num4: Expected Number, Num4, or List",
                        unexpected,
                        instruction_ip,
                    );
                }
            }
        } else {
            let mut result = num4::Num4::default();
            for i in 0..element_count {
                match self.get_register(element_register + i) {
                    Number(n) => result[i as usize] = *n as f32,
                    unexpected => {
                        return self.unexpected_type_error(
                            "num4: Expected Number, Num4, or List",
                            unexpected,
                            instruction_ip,
                        );
                    }
                }
            }
            result
        };

        self.set_register(result_register, Num4(result));
        Ok(())
    }

    fn run_list_push(
        &mut self,
        list_register: u8,
        value_register: u8,
        instruction_ip: usize,
    ) -> InstructionResult {
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
                return vm_error!(
                    self.chunk(),
                    instruction_ip,
                    "Expected List, found '{}'",
                    unexpected,
                );
            }
        };
        Ok(())
    }

    fn run_list_update(
        &mut self,
        list_register: u8,
        index_register: u8,
        value_register: u8,
        instruction_ip: usize,
    ) -> InstructionResult {
        use Value::*;

        let index_value = self.clone_register(index_register);
        let value = self.clone_register(value_register);

        match self.get_register_mut(list_register) {
            List(list) => {
                let list_len = list.len();
                match index_value {
                    Number(index) => {
                        let u_index = index as usize;
                        if index >= 0.0 && u_index < list_len {
                            list.data_mut()[u_index] = value;
                        } else {
                            return vm_error!(
                                self.chunk(),
                                instruction_ip,
                                "Index '{}' not in List",
                                index
                            );
                        }
                    }
                    Range(IntRange { start, end }) => {
                        let ustart = start as usize;
                        let uend = end as usize;

                        if start < 0 || end < 0 {
                            return vm_error!(
                                self.chunk(),
                                instruction_ip,
                                "Indexing with negative indices isn't supported, \
                                                start: {}, end: {}",
                                start,
                                end
                            );
                        } else if start > end {
                            return vm_error!(
                                self.chunk(),
                                instruction_ip,
                                "Indexing with a descending range isn't supported, \
                                                start: {}, end: {}",
                                start,
                                end
                            );
                        } else if ustart > list_len || uend > list_len {
                            return vm_error!(
                                self.chunk(),
                                instruction_ip,
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
                            return vm_error!(
                                self.chunk(),
                                instruction_ip,
                                "Indexing with a descending range isn't supported, \
                                                start: {}, end: {}",
                                start,
                                end
                            );
                        } else if start > list_len || end > list_len {
                            return vm_error!(
                                self.chunk(),
                                instruction_ip,
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
                        return self.unexpected_type_error(
                            "Expected List",
                            &unexpected,
                            instruction_ip,
                        );
                    }
                }
            }
            unexpected => {
                return vm_error!(
                    self.chunk(),
                    instruction_ip,
                    "Expected List, found '{}'",
                    unexpected
                );
            }
        };

        Ok(())
    }

    fn validate_index(&self, n: f64, size: usize, instruction_ip: usize) -> InstructionResult {
        let index = n as usize;

        if n < 0.0 {
            vm_error!(
                self.chunk(),
                instruction_ip,
                "Negative indices aren't allowed ('{}')",
                n
            )
        } else if index >= size {
            vm_error!(
                self.chunk(),
                instruction_ip,
                "Index out of bounds - index: {}, size: {}",
                n,
                size
            )
        } else {
            Ok(())
        }
    }

    fn validate_int_range(
        &self,
        start: isize,
        end: isize,
        size: usize,
        instruction_ip: usize,
    ) -> InstructionResult {
        let ustart = start as usize;
        let uend = end as usize;

        if start < 0 || end < 0 {
            vm_error!(
                self.chunk(),
                instruction_ip,
                "Indexing with negative indices isn't supported, start: {}, end: {}",
                start,
                end
            )
        } else if start > end {
            vm_error!(
                self.chunk(),
                instruction_ip,
                "Indexing with a descending range isn't supported, start: {}, end: {}",
                start,
                end
            )
        } else if ustart > size || uend > size {
            vm_error!(
                self.chunk(),
                instruction_ip,
                "Index out of bounds, size of {} - start: {}, end: {}",
                size,
                start,
                end
            )
        } else {
            Ok(())
        }
    }

    fn validate_index_range(
        &self,
        start: usize,
        end: usize,
        size: usize,
        instruction_ip: usize,
    ) -> InstructionResult {
        if start > end {
            vm_error!(
                self.chunk(),
                instruction_ip,
                "Indexing with a descending range isn't supported, start: {}, end: {}",
                start,
                end
            )
        } else if start > size || end > size {
            vm_error!(
                self.chunk(),
                instruction_ip,
                "Index out of bounds, size of {} - start: {}, end: {}",
                size,
                start,
                end
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
        instruction_ip: usize,
    ) -> InstructionResult {
        use Value::*;

        let value = self.clone_register(value_register);
        let index = self.clone_register(index_register);

        match (value, index) {
            (List(l), Number(n)) => {
                self.validate_index(n, l.len(), instruction_ip)?;
                self.set_register(result_register, l.data()[n as usize].clone());
            }

            (List(l), Range(IntRange { start, end })) => {
                self.validate_int_range(start, end, l.len(), instruction_ip)?;
                self.set_register(
                    result_register,
                    List(ValueList::from_slice(
                        &l.data()[(start as usize)..(end as usize)],
                    )),
                )
            }
            (List(l), IndexRange(value::IndexRange { start, end })) => {
                let end = end.unwrap_or_else(|| l.len());
                self.validate_index_range(start, end, l.len(), instruction_ip)?;
                self.set_register(
                    result_register,
                    List(ValueList::from_slice(&l.data()[start..end])),
                )
            }
            (Tuple(t), Number(n)) => {
                self.validate_index(n, t.data().len(), instruction_ip)?;
                self.set_register(result_register, t.data()[n as usize].clone());
            }

            (Tuple(t), Range(IntRange { start, end })) => {
                self.validate_int_range(start, end, t.data().len(), instruction_ip)?;
                self.set_register(
                    result_register,
                    Tuple(t.data()[(start as usize)..(end as usize)].into()),
                )
            }
            (Tuple(t), IndexRange(value::IndexRange { start, end })) => {
                let end = end.unwrap_or(t.data().len());
                self.validate_index_range(start, end, t.data().len(), instruction_ip)?;
                self.set_register(result_register, Tuple(t.data()[start..end].into()))
            }
            (Num2(n), Number(i)) => {
                let i = i.floor() as usize;
                match i {
                    0 | 1 => self.set_register(result_register, Number(n[i])),
                    other => {
                        return vm_error!(
                            self.chunk(),
                            instruction_ip,
                            "Index out of bounds for Num2, {}",
                            other
                        )
                    }
                }
            }
            (Num4(n), Number(i)) => {
                let i = i.floor() as usize;
                match i {
                    0 | 1 | 2 | 3 => self.set_register(result_register, Number(n[i].into())),
                    other => {
                        return vm_error!(
                            self.chunk(),
                            instruction_ip,
                            "Index out of bounds for Num4, {}",
                            other
                        )
                    }
                }
            }
            (unexpected_value, unexpected_index) => {
                return vm_error!(
                    self.chunk(),
                    instruction_ip,
                    "Unable to index '{}' with '{}'",
                    type_as_string(&unexpected_value),
                    type_as_string(&unexpected_index),
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
        instruction_ip: usize,
    ) -> InstructionResult {
        let key_string = self.value_string_from_constant(key);
        let value = self.clone_register(value);

        match self.get_register_mut(map_register) {
            Value::Map(map) => {
                map.data_mut().insert(Value::Str(key_string), value);
                Ok(())
            }
            unexpected => vm_error!(
                self.chunk(),
                instruction_ip,
                "MapInsert: Expected Map, found '{}'",
                type_as_string(&unexpected)
            ),
        }
    }

    fn run_access(
        &mut self,
        result_register: u8,
        map_register: u8,
        key: ConstantIndex,
        instruction_ip: usize,
    ) -> InstructionResult {
        use Value::*;

        let map_value = self.clone_register(map_register);
        let key_string = self.get_constant_str(key);

        macro_rules! core_op {
            ($module:ident, $iterator_fallback:expr) => {{
                let op = self.get_core_op(
                    key_string,
                    &self.context().core_lib.$module,
                    stringify!($module),
                    $iterator_fallback,
                    instruction_ip,
                )?;
                self.set_register(result_register, op);
            }};
        };

        match map_value {
            Map(map) => match map.data().get_with_string(&key_string) {
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
                return self.unexpected_type_error(
                    "MapAccess: Expected Map",
                    &unexpected,
                    instruction_ip,
                )
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
        instruction_ip: usize,
    ) -> RuntimeResult {
        use Value::*;

        let maybe_op = match module.data().get_with_string(key).cloned() {
            None if iterator_fallback => self
                .context()
                .core_lib
                .iterator
                .data()
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
            None => {
                return vm_error!(
                    self.chunk(),
                    instruction_ip,
                    "'{}' not found in module '{}'",
                    key,
                    module_name
                )
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
        instruction_ip: usize,
    ) -> InstructionResult {
        let function = external_function.function.as_ref();

        let mut call_arg_count = call_arg_count;

        let adjusted_frame_base = if external_function.is_instance_function {
            if let Some(instance_register) = instance_register {
                let parent = self.clone_register(instance_register);
                self.set_register(frame_base, parent);
                call_arg_count += 1;
                frame_base
            } else {
                return vm_error!(
                    self.chunk(),
                    instruction_ip,
                    "Expected self for external instance function"
                );
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
            Err(error) => {
                match error {
                    Error::ErrorWithoutLocation { message } => {
                        return vm_error!(self.chunk(), instruction_ip, message)
                    }
                    _ => return Err(error), // TODO extract external error and enforce its use
                }
            }
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
        instruction_ip: usize,
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
            captures,
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
                return vm_error!(
                    self.chunk(),
                    instruction_ip,
                    "Missing instance for call to instance function"
                );
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
                return vm_error!(
                    self.chunk(),
                    instruction_ip,
                    "Insufficient arguments for function call, expected {}, found {}",
                    expected_arg_count,
                    call_arg_count,
                );
            }
        } else if call_arg_count != expected_arg_count {
            return vm_error!(
                self.chunk(),
                instruction_ip,
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
        instruction_ip: usize,
    ) -> InstructionResult {
        use Value::*;

        match function {
            ExternalFunction(external_function) => self.call_external_function(
                result_register,
                external_function,
                frame_base,
                call_arg_count,
                instance_register,
                instruction_ip,
            ),
            Generator(runtime_function) => self.call_generator(
                result_register,
                runtime_function,
                frame_base,
                call_arg_count,
                instance_register,
                instruction_ip,
            ),
            Function(RuntimeFunction {
                chunk,
                ip: function_ip,
                arg_count: function_arg_count,
                instance_function,
                variadic,
                captures,
            }) => {
                let expected_count = match (instance_function, variadic) {
                    (true, true) => function_arg_count - 2,
                    (true, false) | (false, true) => function_arg_count - 1,
                    (false, false) => function_arg_count,
                };

                // Clone the instance register into the first register of the frame
                let adjusted_frame_base = if instance_function {
                    if let Some(instance_register) = instance_register {
                        let instance = self.clone_register(instance_register);
                        self.set_register(frame_base, instance);
                        frame_base
                    } else {
                        return vm_error!(
                            self.chunk(),
                            instruction_ip,
                            "Missing instance for call to instance function"
                        );
                    }
                } else {
                    // If there's no self arg, then the frame's instance register is unused,
                    // so the new function's frame base is offset by 1
                    frame_base + 1
                };

                if variadic {
                    if call_arg_count >= expected_count {
                        // The last defined arg is the start of the var_args,
                        // e.g. f = |x, y, z...|
                        // arg index 2 is the first vararg, and where the tuple will be placed
                        let arg_base = frame_base + 1;
                        let varargs_start = arg_base + expected_count;
                        let varargs_count = call_arg_count - expected_count;
                        let varargs =
                            Value::Tuple(self.register_slice(varargs_start, varargs_count).into());
                        self.set_register(varargs_start, varargs);
                        self.truncate_registers(varargs_start + 1);
                    } else {
                        return vm_error!(
                            self.chunk(),
                            instruction_ip,
                            "Insufficient arguments for function call, expected {}, found {}",
                            expected_count,
                            call_arg_count,
                        );
                    }
                } else if call_arg_count != expected_count {
                    return vm_error!(
                        self.chunk(),
                        instruction_ip,
                        "Incorrect argument count, expected {}, found {}",
                        expected_count,
                        call_arg_count,
                    );
                }

                // Set info for when the current frame is returned to
                self.frame_mut().return_register_and_ip = Some((result_register, self.ip()));

                // Set up a new frame for the called function
                self.push_frame(chunk, function_ip, adjusted_frame_base, captures);

                Ok(())
            }
            unexpected => {
                self.unexpected_type_error("Expected Function", &unexpected, instruction_ip)
            }
        }
    }

    fn run_debug(
        &self,
        register: u8,
        constant: ConstantIndex,
        instruction_ip: usize,
    ) -> InstructionResult {
        let prefix = match (
            self.reader.chunk.debug_info.get_source_span(instruction_ip),
            self.reader.chunk.source_path.as_ref(),
        ) {
            (Some(span), Some(path)) => format!("[{}: {}] ", path.display(), span.start.line),
            (Some(span), None) => format!("[{}] ", span.start.line),
            (None, Some(path)) => format!("[{}: #ERR] ", path.display()),
            (None, None) => "[#ERR] ".to_string(),
        };
        let value = self.get_register(register);
        println!("{}{}: {}", prefix, self.get_constant_str(constant), value);
        Ok(())
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

    fn push_frame(
        &mut self,
        chunk: Arc<Chunk>,
        ip: usize,
        frame_base: u8,
        captures: Option<ValueList>,
    ) {
        let previous_frame_base = if let Some(frame) = self.call_stack.last() {
            frame.register_base
        } else {
            0
        };
        let new_frame_base = previous_frame_base + frame_base as usize;

        self.call_stack
            .push(Frame::new(chunk.clone(), new_frame_base, captures));
        self.set_chunk_and_ip(chunk, ip);
    }

    fn pop_frame(&mut self, return_value: Value) -> Result<Option<Value>, Error> {
        self.truncate_registers(0);

        if self.call_stack.pop().is_none() {
            return vm_error!(self.chunk(), 0, "pop_frame: Empty call stack");
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

    fn unexpected_type_error<T>(
        &self,
        message: &str,
        value: &Value,
        instruction_ip: usize,
    ) -> Result<T, Error> {
        vm_error!(
            self.chunk(),
            instruction_ip,
            format!("{}, found '{}'", message, type_as_string(&value))
        )
    }

    fn binary_op_error(
        &self,
        lhs: &Value,
        rhs: &Value,
        op: &Instruction,
        ip: usize,
    ) -> InstructionResult {
        vm_error!(
            self.chunk(),
            ip,
            "Unable to perform operation {} with '{}' and '{}'",
            op,
            type_as_string(lhs),
            type_as_string(rhs),
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
        size - (index.abs() as usize)
    } else {
        index as usize
    }
}
