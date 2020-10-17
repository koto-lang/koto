use {
    crate::{
        core::CoreLib,
        external::{self, Args},
        frame::Frame,
        loader, type_as_string,
        value::{self, deep_copy_value, RuntimeFunction},
        value_iterator::{IntRange, Iterable, ValueIterator, ValueIteratorOutput},
        vm_error, Error, Loader, RuntimeResult, Value, ValueList, ValueMap, ValueString, ValueVec,
    },
    koto_bytecode::{Chunk, Instruction, InstructionReader},
    koto_parser::ConstantIndex,
    koto_types::{num2, num4},
    rustc_hash::FxHashMap,
    std::{
        collections::HashMap,
        fmt,
        path::PathBuf,
        sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
    },
};

#[derive(Clone, Debug)]
pub enum ControlFlow {
    Continue,
    Return(Value),
    Yield(Value),
}

#[derive(Default)]
pub struct VmContext {
    pub prelude: ValueMap,
    core_lib: CoreLib,
    global: ValueMap,
    loader: Loader,
    modules: HashMap<PathBuf, ValueMap>,
    string_constants: FxHashMap<(u64, ConstantIndex), ValueString>,
}

impl VmContext {
    fn new() -> Self {
        let core_lib = CoreLib::default();

        let mut prelude = ValueMap::default();
        prelude.add_map("iterator", core_lib.iterator.clone());
        prelude.add_map("list", core_lib.list.clone());
        prelude.add_map("map", core_lib.map.clone());
        prelude.add_map("range", core_lib.range.clone());
        prelude.add_map("string", core_lib.string.clone());

        Self {
            core_lib,
            prelude,
            string_constants: FxHashMap::with_capacity_and_hasher(16, Default::default()),
            ..Default::default()
        }
    }

    fn reset(&mut self) {
        self.loader = Default::default();
    }
}

#[derive(Default)]
pub struct Vm {
    context: Arc<RwLock<VmContext>>,
    reader: InstructionReader,
    value_stack: Vec<Value>,
    call_stack: Vec<Frame>,
}

impl Vm {
    pub fn new() -> Self {
        Self {
            context: Arc::new(RwLock::new(VmContext::new())),
            reader: Default::default(),
            value_stack: Vec::with_capacity(32),
            call_stack: vec![Frame::default()],
        }
    }

    pub fn spawn_shared_vm(&mut self) -> Self {
        Self {
            context: self.context.clone(),
            reader: self.reader.clone(),
            call_stack: vec![Frame::default()],
            ..Default::default()
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
        self.reset();
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
                    let wrap_error_message = |error, wrapper_message| {
                        let wrap_message =
                            |message| format!("{} '{}': {}", wrapper_message, &id[5..], message);

                        Err(match error {
                            Error::VmError {
                                message,
                                chunk,
                                instruction,
                            } => Error::VmError {
                                message: wrap_message(message),
                                chunk,
                                instruction,
                            },
                            Error::ErrorWithoutLocation { message } => {
                                Error::ErrorWithoutLocation {
                                    message: wrap_message(message),
                                }
                            }
                            Error::LoaderError(loader::LoaderError { message, span }) => {
                                Error::LoaderError(loader::LoaderError {
                                    message: wrap_message(message),
                                    span,
                                })
                            }
                        })
                    };

                    if let Some(Value::Function(pre_test)) = &pre_test {
                        let pre_test_result = match pre_test.arg_count {
                            0 => self.run_function(&pre_test.clone(), &[]),
                            _ => self.run_function(&pre_test.clone(), &self_arg),
                        };

                        if let Err(error) = pre_test_result {
                            return wrap_error_message(error, "Error while preparing to run test");
                        }
                    }

                    let test_result = match test.arg_count {
                        0 => self.run_function(&test, &[]),
                        _ => self.run_function(&test, &self_arg),
                    };

                    if let Err(error) = test_result {
                        return wrap_error_message(error, "Error while running test");
                    }

                    if let Some(Value::Function(post_test)) = &post_test {
                        let post_test_result = match post_test.arg_count {
                            0 => self.run_function(&post_test.clone(), &[]),
                            _ => self.run_function(&post_test.clone(), &self_arg),
                        };

                        if let Err(error) = post_test_result {
                            return wrap_error_message(error, "Error after running test");
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

    fn copy_value(&self, value: &Value) -> Value {
        match value.clone() {
            Value::RegisterTuple(value::RegisterTuple { start, count }) => {
                let mut copied = Vec::with_capacity(count as usize);
                for register in start..start + count {
                    copied.push(self.get_register(register).clone());
                }
                Value::Tuple(copied.into())
            }
            other => other,
        }
    }

    fn execute_instruction(
        &mut self,
        instruction: Instruction,
        instruction_ip: usize,
    ) -> Result<ControlFlow, Error> {
        use Value::*;

        let mut result = ControlFlow::Continue;

        match instruction {
            Instruction::Error { message } => {
                return vm_error!(self.chunk(), instruction_ip, "{}", message);
            }
            Instruction::Copy { target, source } => {
                let value = self.copy_register(source);
                self.set_register(target, value);
            }
            Instruction::DeepCopy { target, source } => {
                let value = self.copy_register(source);
                self.set_register(target, deep_copy_value(&value));
            }
            Instruction::SetEmpty { register } => self.set_register(register, Empty),
            Instruction::SetBool { register, value } => self.set_register(register, Bool(value)),
            Instruction::SetNumber { register, value } => {
                self.set_register(register, Number(value))
            }
            Instruction::LoadNumber { register, constant } => self.set_register(
                register,
                Number(self.reader.chunk.constants.get_f64(constant)),
            ),
            Instruction::LoadString { register, constant } => {
                let string = self.value_string_from_constant(constant);
                self.set_register(register, Str(string))
            }
            Instruction::LoadGlobal { register, constant } => {
                let global_name = self.get_constant_string(constant);
                let global = self
                    .context()
                    .global
                    .data()
                    .get_with_string(global_name)
                    .cloned();
                match global {
                    Some(value) => self.set_register(register, value),
                    None => {
                        return vm_error!(
                            self.chunk(),
                            instruction_ip,
                            "'{}' not found",
                            global_name
                        );
                    }
                }
            }
            Instruction::SetGlobal { global, source } => {
                let global_name = self.value_string_from_constant(global);
                let value = self.copy_register(source);
                self.context_mut()
                    .global
                    .data_mut()
                    .insert(Str(global_name), value);
            }
            Instruction::Import { register, constant } => {
                self.run_import(register, constant, instruction_ip)?;
            }
            Instruction::MakeTuple {
                register,
                start,
                count,
            } => {
                self.set_register(
                    register,
                    RegisterTuple(value::RegisterTuple { start, count }),
                );
            }
            Instruction::MakeList {
                register,
                size_hint,
            } => {
                self.set_register(register, List(ValueList::with_capacity(size_hint)));
            }
            Instruction::MakeMap {
                register,
                size_hint,
            } => {
                self.set_register(register, Map(ValueMap::with_capacity(size_hint)));
            }
            Instruction::MakeNum2 {
                register,
                count,
                element_register,
            } => {
                self.run_make_num2(register, count, element_register, instruction_ip)?;
            }
            Instruction::MakeNum4 {
                register,
                count,
                element_register,
            } => {
                self.run_make_num4(register, count, element_register, instruction_ip)?;
            }
            Instruction::Range {
                register,
                start,
                end,
            } => {
                let range = match (self.get_register(start), self.get_register(end)) {
                    (Number(start), Number(end)) => {
                        let (start, end) = if start <= end {
                            (*start as isize, *end as isize)
                        } else {
                            // descending ranges will be evaluated with (end..start).rev()
                            (*start as isize + 1, *end as isize + 1)
                        };

                        Range(IntRange { start, end })
                    }
                    unexpected => {
                        return vm_error!(
                            self.chunk(),
                            instruction_ip,
                            "Expected numbers for range bounds, found start: {}, end: {}",
                            type_as_string(&unexpected.0),
                            type_as_string(&unexpected.1)
                        )
                    }
                };
                self.set_register(register, range);
            }
            Instruction::RangeInclusive {
                register,
                start,
                end,
            } => {
                let range = match (self.get_register(start), self.get_register(end)) {
                    (Number(start), Number(end)) => {
                        let (start, end) = if start <= end {
                            (*start as isize, *end as isize + 1)
                        } else {
                            // descending ranges will be evaluated with (end..start).rev()
                            (*start as isize + 1, *end as isize)
                        };

                        Range(IntRange { start, end })
                    }
                    unexpected => {
                        return vm_error!(
                            self.chunk(),
                            instruction_ip,
                            "Expected numbers for range bounds, found start: {}, end: {}",
                            type_as_string(&unexpected.0),
                            type_as_string(&unexpected.1)
                        )
                    }
                };
                self.set_register(register, range);
            }
            Instruction::RangeTo { register, end } => {
                let range = match self.get_register(end) {
                    Number(end) => {
                        if *end < 0.0 {
                            return vm_error!(
                                self.chunk(),
                                instruction_ip,
                                "RangeTo: negative numbers not allowed, found '{}'",
                                end
                            );
                        }
                        IndexRange(value::IndexRange {
                            start: 0,
                            end: Some(*end as usize),
                        })
                    }
                    unexpected => {
                        return vm_error!(
                            self.chunk(),
                            instruction_ip,
                            "RangeTo: Expected numbers for range bounds, found end: {}",
                            type_as_string(&unexpected)
                        )
                    }
                };
                self.set_register(register, range);
            }
            Instruction::RangeToInclusive { register, end } => {
                let range = match self.get_register(end) {
                    Number(end) => {
                        if *end < 0.0 {
                            return vm_error!(
                                self.chunk(),
                                instruction_ip,
                                "RangeToInclusive: negative numbers not allowed, found '{}'",
                                end
                            );
                        }
                        IndexRange(value::IndexRange {
                            start: 0,
                            end: Some(*end as usize + 1),
                        })
                    }
                    unexpected => {
                        return vm_error!(
                            self.chunk(),
                            instruction_ip,
                            "RangeToInclusive: Expected numbers for range bounds, found end: {}",
                            type_as_string(&unexpected)
                        )
                    }
                };
                self.set_register(register, range);
            }
            Instruction::RangeFrom { register, start } => {
                let range = match self.get_register(start) {
                    Number(start) => {
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
                    unexpected => {
                        return vm_error!(
                            self.chunk(),
                            instruction_ip,
                            "RangeFrom: Expected numbers for range bounds, found end: {}",
                            type_as_string(&unexpected)
                        )
                    }
                };
                self.set_register(register, range);
            }
            Instruction::RangeFull { register } => {
                self.set_register(
                    register,
                    IndexRange(value::IndexRange {
                        start: 0,
                        end: None,
                    }),
                );
            }
            Instruction::MakeIterator { register, iterable } => {
                let iterable = self.copy_register(iterable);

                if matches!(iterable, Iterator(_)) {
                    self.set_register(register, iterable);
                } else {
                    let iterator = match iterable {
                        Range(int_range) => ValueIterator::with_range(int_range),
                        List(list) => ValueIterator::with_list(list),
                        Map(map) => ValueIterator::with_map(map),
                        Tuple(tuple) => ValueIterator::with_tuple(tuple),
                        unexpected => {
                            return vm_error!(
                                self.chunk(),
                                instruction_ip,
                                "Expected iterable value while making iterator, found '{}'",
                                type_as_string(&unexpected)
                            );
                        }
                    };

                    self.set_register(register, iterator.into());
                }
            }
            Instruction::Function {
                register,
                arg_count,
                capture_count,
                size,
                is_generator,
            } => {
                let captures = if capture_count > 0 {
                    let mut captures = ValueVec::new();
                    captures.resize(capture_count as usize, Empty);
                    Some(ValueList::with_data(captures))
                } else {
                    None
                };

                let function = Function(RuntimeFunction {
                    chunk: self.chunk(),
                    ip: self.ip(),
                    arg_count,
                    captures,
                    is_generator,
                });
                self.jump_ip(size);
                self.set_register(register, function);
            }
            Instruction::Capture {
                function,
                target,
                source,
            } => match self.get_register(function) {
                Function(f) => match &f.captures {
                    Some(captures) => {
                        captures.data_mut()[target as usize] = self.get_register(source).clone()
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
                    return vm_error!(
                        self.chunk(),
                        instruction_ip,
                        "Capture: expected Function, found '{}'",
                        type_as_string(unexpected)
                    )
                }
            },
            Instruction::LoadCapture { register, capture } => {
                match self.frame().get_capture(capture) {
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
            }
            Instruction::SetCapture { capture, source } => {
                if self.call_stack.len() == 1 {
                    return vm_error!(
                        self.chunk(),
                        instruction_ip,
                        "SetCapture: attempting to set a capture outside of a function"
                    );
                }

                let value = self.get_register(source).clone();

                if !self.frame_mut().set_capture(capture, value) {
                    return vm_error!(
                        self.chunk(),
                        instruction_ip,
                        "SetCapture: invalid capture index: {} ",
                        capture
                    );
                }
            }
            Instruction::Negate { register, source } => {
                let result = match &self.get_register(source) {
                    Bool(b) => Bool(!b),
                    Number(n) => Number(-n),
                    Num2(v) => Num2(-v),
                    Num4(v) => Num4(-v),
                    unexpected => {
                        return vm_error!(
                            self.chunk(),
                            instruction_ip,
                            "Negate: expected negatable value, found '{}'",
                            type_as_string(unexpected)
                        );
                    }
                };
                self.set_register(register, result);
            }
            Instruction::Add { register, lhs, rhs } => {
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
                        return binary_op_error(
                            self.chunk(),
                            instruction,
                            lhs_value,
                            rhs_value,
                            instruction_ip,
                        );
                    }
                };
                self.set_register(register, result);
            }
            Instruction::Subtract { register, lhs, rhs } => {
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
                        return binary_op_error(
                            self.chunk(),
                            instruction,
                            lhs_value,
                            rhs_value,
                            instruction_ip,
                        );
                    }
                };
                self.set_register(register, result);
            }
            Instruction::Multiply { register, lhs, rhs } => {
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
                        return binary_op_error(
                            self.chunk(),
                            instruction,
                            lhs_value,
                            rhs_value,
                            instruction_ip,
                        );
                    }
                };
                self.set_register(register, result);
            }
            Instruction::Divide { register, lhs, rhs } => {
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
                        return binary_op_error(
                            self.chunk(),
                            instruction,
                            lhs_value,
                            rhs_value,
                            instruction_ip,
                        );
                    }
                };
                self.set_register(register, result);
            }
            Instruction::Modulo { register, lhs, rhs } => {
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
                        return binary_op_error(
                            self.chunk(),
                            instruction,
                            lhs_value,
                            rhs_value,
                            instruction_ip,
                        );
                    }
                };
                self.set_register(register, result);
            }
            Instruction::Less { register, lhs, rhs } => {
                let lhs_value = self.get_register(lhs);
                let rhs_value = self.get_register(rhs);
                let result = match (&lhs_value, &rhs_value) {
                    (Number(a), Number(b)) => Bool(a < b),
                    _ => {
                        return binary_op_error(
                            self.chunk(),
                            instruction,
                            lhs_value,
                            rhs_value,
                            instruction_ip,
                        );
                    }
                };
                self.set_register(register, result);
            }
            Instruction::LessOrEqual { register, lhs, rhs } => {
                let lhs_value = self.get_register(lhs);
                let rhs_value = self.get_register(rhs);
                let result = match (&lhs_value, &rhs_value) {
                    (Number(a), Number(b)) => Bool(a <= b),
                    _ => {
                        return binary_op_error(
                            self.chunk(),
                            instruction,
                            lhs_value,
                            rhs_value,
                            instruction_ip,
                        );
                    }
                };
                self.set_register(register, result);
            }
            Instruction::Greater { register, lhs, rhs } => {
                let lhs_value = self.get_register(lhs);
                let rhs_value = self.get_register(rhs);
                let result = match (&lhs_value, &rhs_value) {
                    (Number(a), Number(b)) => Bool(a > b),
                    _ => {
                        return binary_op_error(
                            self.chunk(),
                            instruction,
                            lhs_value,
                            rhs_value,
                            instruction_ip,
                        );
                    }
                };
                self.set_register(register, result);
            }
            Instruction::GreaterOrEqual { register, lhs, rhs } => {
                let lhs_value = self.get_register(lhs);
                let rhs_value = self.get_register(rhs);
                let result = match (&lhs_value, &rhs_value) {
                    (Number(a), Number(b)) => Bool(a >= b),
                    _ => {
                        return binary_op_error(
                            self.chunk(),
                            instruction,
                            lhs_value,
                            rhs_value,
                            instruction_ip,
                        );
                    }
                };
                self.set_register(register, result);
            }
            Instruction::Equal { register, lhs, rhs } => {
                let lhs_value = self.get_register(lhs);
                let rhs_value = self.get_register(rhs);
                let result = (lhs_value == rhs_value).into();
                self.set_register(register, result);
            }
            Instruction::NotEqual { register, lhs, rhs } => {
                let lhs_value = self.get_register(lhs);
                let rhs_value = self.get_register(rhs);
                let result = (lhs_value != rhs_value).into();
                self.set_register(register, result);
            }
            Instruction::Jump { offset } => {
                self.jump_ip(offset);
            }
            Instruction::JumpIf {
                register,
                offset,
                jump_condition,
            } => match self.get_register(register) {
                Bool(b) => {
                    if *b == jump_condition {
                        self.jump_ip(offset);
                    }
                }
                unexpected => {
                    return vm_error!(
                        self.chunk(),
                        instruction_ip,
                        "Expected Bool, found '{}'",
                        type_as_string(&unexpected),
                    );
                }
            },
            Instruction::JumpBack { offset } => {
                self.jump_ip_back(offset);
            }
            Instruction::JumpBackIf {
                register,
                offset,
                jump_condition,
            } => match self.get_register(register) {
                Bool(b) => {
                    if *b == jump_condition {
                        self.jump_ip_back(offset);
                    }
                }
                unexpected => {
                    return vm_error!(
                        self.chunk(),
                        instruction_ip,
                        "Expected Bool, found '{}'",
                        type_as_string(&unexpected),
                    );
                }
            },
            Instruction::Call {
                result,
                function,
                arg_register,
                arg_count,
            } => {
                let function = self.get_register(function).clone();
                self.call_function(
                    result,
                    &function,
                    arg_register,
                    arg_count,
                    None,
                    instruction_ip,
                )?;
            }
            Instruction::CallChild {
                result,
                function,
                arg_register,
                arg_count,
                parent,
            } => {
                let function = self.get_register(function).clone();
                self.call_function(
                    result,
                    &function,
                    arg_register,
                    arg_count,
                    Some(parent),
                    instruction_ip,
                )?;
            }
            Instruction::Return { register } => {
                if let Some(return_value) = self.pop_frame(self.get_register(register).clone())? {
                    // If pop_frame returns a new return_value, then execution should stop.
                    result = ControlFlow::Return(return_value);
                }
            }
            Instruction::Yield { register } => {
                result = ControlFlow::Yield(self.get_register(register).clone());
            }
            Instruction::Type { register, source } => {
                let result = match self.get_register(source) {
                    Bool(_) => "bool".to_string(),
                    Empty => "empty".to_string(),
                    Function(_) => "function".to_string(),
                    ExternalFunction(_) => "function".to_string(),
                    ExternalValue(value) => value.read().unwrap().value_type(),
                    List(_) => "list".to_string(),
                    Map(_) => "map".to_string(),
                    Number(_) => "number".to_string(),
                    Num2(_) => "num2".to_string(),
                    Num4(_) => "num4".to_string(),
                    Range(_) => "range".to_string(),
                    Str(_) => "string".to_string(),
                    unexpected => {
                        return vm_error!(
                            self.chunk(),
                            instruction_ip,
                            "type is only supported for user types, found {}",
                            unexpected
                        );
                    }
                };

                self.set_register(register, Str(result.into()));
            }
            Instruction::IteratorNext {
                register,
                iterator,
                jump_offset,
            } => {
                let result = match self.get_register_mut(iterator) {
                    Iterator(iterator) => iterator.next(),
                    unexpected => {
                        return vm_error!(
                            self.chunk(),
                            instruction_ip,
                            "Expected Iterator, found '{}'",
                            type_as_string(&unexpected),
                        )
                    }
                };

                match result {
                    Some(Ok(ValueIteratorOutput::Value(value))) => {
                        self.set_register(register, value)
                    }
                    Some(Ok(ValueIteratorOutput::ValuePair(first, second))) => {
                        self.set_register(
                            register,
                            Value::RegisterTuple(value::RegisterTuple {
                                start: register + 1,
                                count: 2,
                            }),
                        );
                        self.set_register(register + 1, first);
                        self.set_register(register + 2, second);
                    }
                    Some(Err(error)) => match error {
                        Error::ErrorWithoutLocation { message } => {
                            return vm_error!(self.chunk(), instruction_ip, message)
                        }
                        _ => return Err(error),
                    },
                    None => self.jump_ip(jump_offset),
                };
            }
            Instruction::ValueIndex {
                register,
                expression,
                index,
            } => {
                let expression_value = self.get_register(expression).clone();

                match expression_value {
                    List(l) => {
                        let value = l.data().get(index as usize).cloned().unwrap_or(Empty);
                        self.set_register(register, value);
                    }
                    RegisterTuple(value::RegisterTuple { start, count }) => {
                        if index >= count {
                            return vm_error!(
                                self.chunk(),
                                instruction_ip,
                                "Index out of range index: {}, count: {}",
                                index,
                                count
                            );
                        }
                        let value = self.get_register(start + index).clone();
                        self.set_register(register, value);
                    }
                    other => {
                        if index == 0 {
                            self.set_register(register, other);
                        } else {
                            self.set_register(register, Empty);
                        }
                    }
                };
            }
            Instruction::ListPushValue { list, value } => {
                self.run_list_push(list, value, instruction_ip)?;
            }
            Instruction::ListPushValues {
                list,
                values_start,
                count,
            } => {
                for value_register in values_start..(values_start + count) {
                    self.run_list_push(list, value_register, instruction_ip)?;
                }
            }
            Instruction::ListUpdate { list, index, value } => {
                self.run_list_update(list, index, value, instruction_ip)?;
            }
            Instruction::ListIndex {
                register,
                list,
                index,
            } => {
                self.run_list_index(register, list, index, instruction_ip)?;
            }
            Instruction::MapInsert {
                register,
                value,
                key,
            } => {
                let key_string = self.value_string_from_constant(key);
                let value = self.get_register(value).clone();

                match self.get_register_mut(register) {
                    Map(map) => map.data_mut().insert(Str(key_string), value),
                    unexpected => {
                        return vm_error!(
                            self.chunk(),
                            instruction_ip,
                            "MapInsert: Expected Map, found '{}'",
                            type_as_string(&unexpected),
                        )
                    }
                };
            }
            Instruction::MapAccess { register, map, key } => {
                self.run_map_access(register, map, key, instruction_ip)?;
            }
            Instruction::TryStart {
                arg_register,
                catch_offset,
            } => {
                let catch_ip = self.ip() + catch_offset;
                self.frame_mut().catch_stack.push((arg_register, catch_ip));
            }
            Instruction::TryEnd => {
                self.frame_mut().catch_stack.pop();
            }
            Instruction::Debug { register, constant } => {
                let prefix = match (
                    self.reader.chunk.debug_info.get_source_span(instruction_ip),
                    self.reader.chunk.source_path.as_ref(),
                ) {
                    (Some(span), Some(path)) => {
                        format!("[{}: {}] ", path.display(), span.start.line)
                    }
                    (Some(span), None) => format!("[{}] ", span.start.line),
                    (None, Some(path)) => format!("[{}: #ERR] ", path.display()),
                    (None, None) => "[#ERR] ".to_string(),
                };
                let value = self.get_register(register);
                println!(
                    "{}{}: {}",
                    prefix,
                    self.get_constant_string(constant),
                    value
                );
            }
        }

        Ok(result)
    }

    fn run_import(
        &mut self,
        result_register: u8,
        import_constant: ConstantIndex,
        instruction_ip: usize,
    ) -> Result<(), Error> {
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
                            e.message
                        )
                    }
                };
                let maybe_module = self.context().modules.get(&module_path).cloned();
                match maybe_module {
                    Some(module) => self.set_register(result_register, Value::Map(module)),
                    None => {
                        // Run the chunk, and cache the resulting global map
                        let mut vm = Vm::new();
                        vm.context_mut().prelude = self.context().prelude.clone();
                        vm.run(module_chunk)?;
                        if let Some(main) = vm.get_global_function("main") {
                            vm.run_function(&main, &[])?;
                        }
                        self.context_mut()
                            .modules
                            .insert(module_path, vm.context().global.clone());
                        self.set_register(result_register, Value::Map(vm.context().global.clone()));
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
    ) -> Result<(), Error> {
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
                                return vm_error!(
                                    self.chunk(),
                                    instruction_ip,
                                    "num2 only accepts Numbers as arguments, - found {}",
                                    unexpected
                                )
                            }
                        }
                    }
                    result
                }
                unexpected => {
                    return vm_error!(
                        self.chunk(),
                        instruction_ip,
                        "num2 only accepts a Number, Num2, or List as first argument \
                         - found {}",
                        unexpected
                    );
                }
            }
        } else {
            let mut result = num2::Num2::default();
            for i in 0..element_count {
                match self.get_register(element_register + i) {
                    Number(n) => result[i as usize] = *n,
                    unexpected => {
                        return vm_error!(
                            self.chunk(),
                            instruction_ip,
                            "num2 only accepts Numbers as arguments, \
                             or Num2 or List as first argument - found {}",
                            unexpected
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
    ) -> Result<(), Error> {
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
                                return vm_error!(
                                    self.chunk(),
                                    instruction_ip,
                                    "num4 only accepts Numbers as arguments, - found {}",
                                    unexpected
                                )
                            }
                        }
                    }
                    result
                }
                unexpected => {
                    return vm_error!(
                        self.chunk(),
                        instruction_ip,
                        "num4 only accepts a Number, Num4, or List as first argument \
                         - found {}",
                        unexpected
                    );
                }
            }
        } else {
            let mut result = num4::Num4::default();
            for i in 0..element_count {
                match self.get_register(element_register + i) {
                    Number(n) => result[i as usize] = *n as f32,
                    unexpected => {
                        return vm_error!(
                            self.chunk(),
                            instruction_ip,
                            "num4 only accepts Numbers as arguments, \
                             or Num4 or List as first argument - found {}",
                            unexpected
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
    ) -> Result<(), Error> {
        use Value::*;

        let value = self.get_register(value_register).clone();

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
                    type_as_string(&unexpected),
                )
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
    ) -> Result<(), Error> {
        use Value::*;

        let index_value = self.get_register(index_register).clone();
        let value = self.get_register(value_register).clone();

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
                        return vm_error!(
                            self.chunk(),
                            instruction_ip,
                            "Unexpected type for List index: '{}'",
                            type_as_string(&unexpected)
                        );
                    }
                }
            }
            unexpected => {
                return vm_error!(
                    self.chunk(),
                    instruction_ip,
                    "Expected List, found '{}'",
                    type_as_string(&unexpected),
                )
            }
        };

        Ok(())
    }

    fn run_list_index(
        &mut self,
        result_register: u8,
        list_register: u8,
        index_register: u8,
        instruction_ip: usize,
    ) -> Result<(), Error> {
        use Value::*;

        let list_value = self.get_register(list_register).clone();
        let index_value = self.get_register(index_register).clone();

        match list_value {
            List(l) => {
                let list_len = l.len();

                match index_value {
                    Number(n) => {
                        if n < 0.0 {
                            return vm_error!(
                                self.chunk(),
                                instruction_ip,
                                "Negative list indices aren't allowed (found '{}')",
                                n
                            );
                        }
                        match l.data().get(n as usize) {
                            Some(value) => {
                                self.set_register(result_register, value.clone());
                            }
                            None => {
                                return vm_error!(
                                    self.chunk(),
                                    instruction_ip,
                                    "List index out of bounds - index: {}, list size: {}",
                                    n,
                                    list_len
                                )
                            }
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
                            // TODO Avoid allocating new vec,
                            // introduce 'slice' value type
                            self.set_register(
                                result_register,
                                List(ValueList::from_slice(&l.data()[ustart..uend])),
                            )
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
                            self.set_register(
                                result_register,
                                List(ValueList::from_slice(&l.data()[start..end])),
                            )
                        }
                    }
                    unexpected => {
                        return vm_error!(
                            self.chunk(),
                            instruction_ip,
                            "Expected Number or Range, found '{}'",
                            type_as_string(&unexpected),
                        )
                    }
                }
            }
            Num2(n) => match index_value {
                Number(i) => {
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
                unexpected => {
                    return vm_error!(
                        self.chunk(),
                        instruction_ip,
                        "Expected Number as index for Num2, found '{}'",
                        type_as_string(&unexpected),
                    )
                }
            },
            Num4(n) => match index_value {
                Number(i) => {
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
                unexpected => {
                    return vm_error!(
                        self.chunk(),
                        instruction_ip,
                        "Expected Number as index for Num4, found '{}'",
                        type_as_string(&unexpected),
                    )
                }
            },
            unexpected => {
                return vm_error!(
                    self.chunk(),
                    instruction_ip,
                    "Expected indexable value, found '{}'",
                    type_as_string(&unexpected),
                )
            }
        };

        Ok(())
    }

    fn run_map_access(
        &mut self,
        result_register: u8,
        map_register: u8,
        key: ConstantIndex,
        instruction_ip: usize,
    ) -> Result<(), Error> {
        use Value::*;

        let map_value = self.get_register(map_register).clone();
        let key_string = self.get_constant_string(key);

        macro_rules! get_core_op {
            ($module:ident, $module_name:expr) => {{
                let maybe_op = self
                    .context()
                    .core_lib
                    .$module
                    .data()
                    .get_with_string(&key_string)
                    .cloned();

                match maybe_op {
                    Some(op) => match op {
                        ExternalFunction(f) => {
                            // Core module functions accessed in a lookup need to be invoked as
                            // if they were declared as instance functions. This feels a bit hacky
                            // but I haven't found a simpler approach yet!
                            let f_as_instance_function = external::ExternalFunction {
                                is_instance_function: true,
                                ..f.clone()
                            };
                            self.set_register(
                                result_register,
                                Value::ExternalFunction(f_as_instance_function),
                            );
                        }
                        other => {
                            self.set_register(result_register, other.clone());
                        }
                    },
                    None => {
                        return vm_error!(
                            self.chunk(),
                            instruction_ip,
                            "{} operation '{}' not found",
                            $module_name,
                            key_string,
                        );
                    }
                }

                Ok(())
            }};
        };

        match map_value {
            Map(map) => match map.data().get_with_string(&key_string) {
                Some(value) => {
                    self.set_register(result_register, value.clone());
                }
                None => get_core_op!(map, "Map")?,
            },
            List(_) => get_core_op!(list, "List")?,
            Range(_) => get_core_op!(range, "Range")?,
            Str(_) => get_core_op!(string, "String")?,
            Iterator(_) => get_core_op!(iterator, "Iterator")?,
            unexpected => {
                return vm_error!(
                    self.chunk(),
                    instruction_ip,
                    "MapAccess: Expected Map, found '{}'",
                    type_as_string(&unexpected),
                )
            }
        }

        Ok(())
    }

    fn call_function(
        &mut self,
        result_register: u8,
        function: &Value,
        arg_register: u8,
        call_arg_count: u8,
        parent_register: Option<u8>,
        instruction_ip: usize,
    ) -> RuntimeResult {
        use Value::*;

        match function {
            ExternalFunction(external) => {
                let function = external.function.as_ref();

                let mut call_arg_count = call_arg_count;

                if external.is_instance_function {
                    if let Some(parent_register) = parent_register {
                        self.insert_register(
                            arg_register,
                            self.get_register(parent_register).clone(),
                        );
                        call_arg_count += 1;
                    } else {
                        return vm_error!(
                            self.chunk(),
                            instruction_ip,
                            "Expected self for external instance function"
                        );
                    }
                };

                let result = (&*function)(
                    self,
                    &Args {
                        register: arg_register,
                        count: call_arg_count,
                    },
                );
                match result {
                    Ok(value) => {
                        self.set_register(result_register, value);
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
            }
            Function(RuntimeFunction {
                chunk,
                ip: function_ip,
                arg_count: function_arg_count,
                captures,
                is_generator,
            }) => {
                if *is_generator {
                    let mut generator_vm = self.spawn_shared_vm();

                    generator_vm.call_stack.clear();
                    generator_vm.push_frame(
                        chunk.clone(),
                        *function_ip,
                        0, // arguments will be copied starting in register 0
                        captures.clone(),
                    );

                    // prepare args for the spawned vm
                    let mut call_arg_count = call_arg_count;
                    let mut set_arg_offset = 0;

                    if let Some(parent_register) = parent_register {
                        if call_arg_count < *function_arg_count {
                            generator_vm
                                .set_register(0, self.get_register(parent_register).clone());
                            set_arg_offset = 1;
                            call_arg_count += 1;
                        }
                    }

                    let args_to_copy = if *function_arg_count == 0 {
                        0
                    } else {
                        *function_arg_count - set_arg_offset
                    };

                    // Copy args starting at register 0
                    for arg in 0..args_to_copy {
                        generator_vm.set_register(
                            (arg + set_arg_offset) as u8,
                            self.get_register(arg_register + arg).clone(),
                        );
                    }

                    if *function_arg_count != call_arg_count {
                        return vm_error!(
                            self.chunk(),
                            instruction_ip,
                            "Incorrect argument count, expected {}, found {}",
                            function_arg_count,
                            call_arg_count,
                        );
                    }

                    self.set_register(result_register, ValueIterator::with_vm(generator_vm).into())
                } else {
                    let expected_count = *function_arg_count;
                    let mut call_arg_count = call_arg_count;

                    if let Some(parent_register) = parent_register {
                        if call_arg_count < expected_count {
                            self.insert_register(
                                arg_register,
                                self.get_register(parent_register).clone(),
                            );
                            call_arg_count += 1;
                        }
                    }

                    if call_arg_count != expected_count {
                        return vm_error!(
                            self.chunk(),
                            instruction_ip,
                            "Incorrect argument count, expected {}, found {}",
                            function_arg_count,
                            call_arg_count,
                        );
                    }

                    self.frame_mut().return_register_and_ip = Some((result_register, self.ip()));

                    self.push_frame(chunk.clone(), *function_ip, arg_register, captures.clone());
                }
            }
            unexpected => {
                return vm_error!(
                    self.chunk(),
                    instruction_ip,
                    "Expected Function, found '{}'",
                    type_as_string(&unexpected),
                );
            }
        };

        Ok(Empty)
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
        arg_register: u8,
        captures: Option<ValueList>,
    ) {
        let previous_frame_base = if let Some(frame) = self.call_stack.last() {
            frame.register_base
        } else {
            0
        };
        let new_frame_base = previous_frame_base + arg_register as usize;

        self.call_stack
            .push(Frame::new(chunk.clone(), new_frame_base, captures));
        self.set_chunk_and_ip(chunk, ip);
    }

    fn pop_frame(&mut self, return_value: Value) -> Result<Option<Value>, Error> {
        let frame = match self.call_stack.pop() {
            Some(frame) => frame,
            None => {
                return vm_error!(self.chunk(), 0, "pop_frame: Empty call stack");
            }
        };

        if !self.call_stack.is_empty() && self.frame().return_register_and_ip.is_some() {
            let return_value = match return_value {
                Value::RegisterTuple(value::RegisterTuple { start, count }) => {
                    // If the return value is a register tuple (i.e. when returning multiple values),
                    // then copy the list's values to the frame base,
                    // and adjust the list's start register to match their new position.
                    if start != 0 {
                        let start = start as usize;
                        for i in 0..count as usize {
                            let source = frame.register_base + start + i;
                            let target = frame.register_base + i;
                            self.value_stack[target] = self.value_stack[source].clone();
                        }
                    }

                    // Keep the register tuple values around after the frame has been popped
                    self.value_stack
                        .truncate(frame.register_base + count as usize);

                    Value::RegisterTuple(value::RegisterTuple {
                        start: frame.register_base as u8,
                        count,
                    })
                }
                other => {
                    self.value_stack.truncate(frame.register_base);
                    other
                }
            };

            let (return_register, return_ip) = self.frame().return_register_and_ip.unwrap();

            self.set_register(return_register, return_value);
            self.set_chunk_and_ip(self.frame().chunk.clone(), return_ip);

            Ok(None)
        } else {
            // If there's no return register, then make a copy of the value to return out of the VM
            // (i.e. we need to make sure we've collected a RegisterTuple into a List).
            let return_value = self.copy_value(&return_value);
            self.value_stack.truncate(frame.register_base);

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

    fn insert_register(&mut self, register: u8, value: Value) {
        let index = self.register_index(register);

        if index >= self.value_stack.len() {
            self.value_stack.resize(index + 1, Value::Empty);
        }

        self.value_stack.insert(index, value);
    }

    // Returns a copy of a value, while upgrading register tuples
    fn copy_register(&mut self, register: u8) -> Value {
        let value = self.get_register(register).clone();
        match value {
            Value::RegisterTuple(value::RegisterTuple { start, count }) => {
                let mut copied = Vec::with_capacity(count as usize);
                for register in start..start + count {
                    copied.push(self.get_register(register).clone());
                }
                let tuple = Value::Tuple(copied.into());
                self.set_register(register, tuple.clone());
                tuple
            }
            other => other,
        }
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

    pub fn get_args(&self, args: &Args) -> &[Value] {
        self.register_slice(args.register, args.count)
    }

    pub fn get_args_as_vec(&self, args: &Args) -> ValueVec {
        self.get_args(args).iter().cloned().collect()
    }

    fn get_constant_string(&self, constant_index: ConstantIndex) -> &str {
        self.reader.chunk.constants.get_string(constant_index)
    }

    fn value_string_from_constant(&mut self, constant_index: ConstantIndex) -> ValueString {
        let constants_hash = self.reader.chunk.constants_hash;

        let maybe_string = self
            .context()
            .string_constants
            .get(&(constants_hash, constant_index))
            .cloned();

        match maybe_string {
            Some(s) => s,
            None => {
                let s: ValueString = self
                    .reader
                    .chunk
                    .constants
                    .get_string(constant_index)
                    .to_string()
                    .into();
                self.context_mut()
                    .string_constants
                    .insert((constants_hash, constant_index), s.clone());
                s
            }
        }
    }
}

impl fmt::Debug for Vm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Vm")
    }
}

fn binary_op_error(
    chunk: Arc<Chunk>,
    op: Instruction,
    lhs: &Value,
    rhs: &Value,
    ip: usize,
) -> Result<ControlFlow, Error> {
    vm_error!(
        chunk,
        ip,
        "Unable to perform operation {:?} with lhs: '{}' and rhs: '{}'",
        op,
        lhs,
        rhs,
    )
}
