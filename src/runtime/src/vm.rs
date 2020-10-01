#![allow(dead_code)]

use {
    crate::{
        core, external,
        frame::Frame,
        loader, type_as_string,
        value::{self, deep_copy_value, RuntimeFunction},
        value_iterator::{IntRange, Iterable, ValueIterator, ValueIteratorOutput},
        vm_error, Error, Loader, RuntimeResult, Value, ValueList, ValueMap, ValueVec,
    },
    koto_bytecode::{Chunk, Instruction, InstructionReader},
    koto_parser::{num2, num4, ConstantIndex},
    rustc_hash::FxHashMap,
    std::{collections::HashMap, path::PathBuf, sync::Arc},
};

#[derive(Clone, Debug)]
pub enum ControlFlow {
    Continue,
    ReturnValue(Value),
}

#[derive(Clone)]
struct CoreLib {
    list: ValueMap,
    map: ValueMap,
    range: ValueMap,
    string: ValueMap,
}

impl Default for CoreLib {
    fn default() -> Self {
        Self {
            list: core::list::make_module(),
            map: core::map::make_module(),
            range: core::range::make_module(),
            string: core::string::make_module(),
        }
    }
}

#[derive(Default)]
pub struct Vm {
    core_lib: CoreLib,
    prelude: ValueMap,
    global: ValueMap,
    loader: Loader,
    modules: HashMap<PathBuf, ValueMap>,

    reader: InstructionReader,
    string_constants: FxHashMap<(u64, ConstantIndex), Arc<String>>,
    value_stack: Vec<Value>,
    call_stack: Vec<Frame>,
}

impl Vm {
    pub fn new() -> Self {
        let core_lib = CoreLib::default();

        let mut prelude = ValueMap::default();
        prelude.add_map("list", core_lib.list.clone());
        prelude.add_map("map", core_lib.map.clone());
        prelude.add_map("range", core_lib.range.clone());
        prelude.add_map("string", core_lib.string.clone());

        Self {
            core_lib,
            prelude,
            global: Default::default(),
            loader: Default::default(),
            modules: Default::default(),
            reader: Default::default(),
            string_constants: FxHashMap::with_capacity_and_hasher(16, Default::default()),
            value_stack: Vec::with_capacity(32),
            call_stack: vec![Frame::default()],
        }
    }

    pub fn spawn_shared_vm(&mut self) -> Self {
        Self {
            core_lib: self.core_lib.clone(),
            prelude: self.prelude.clone(),
            global: self.global.clone(),
            loader: self.loader.clone(),
            modules: self.modules.clone(),
            reader: self.reader.clone(),
            call_stack: vec![Frame::default()],
            ..Default::default()
        }
    }

    pub fn prelude_mut(&mut self) -> &mut ValueMap {
        &mut self.prelude
    }

    pub fn get_global_value(&self, id: &str) -> Option<Value> {
        self.global.data().get_with_string(id).cloned()
    }

    pub fn get_global_function(&self, id: &str) -> Option<RuntimeFunction> {
        match self.get_global_value(id) {
            Some(Value::Function(function)) => Some(function),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.reader = Default::default();
        self.value_stack = Default::default();
        self.call_stack = Default::default();
        self.string_constants = Default::default();
    }

    pub fn run(&mut self, chunk: Arc<Chunk>) -> RuntimeResult {
        self.reset();
        self.push_frame(chunk, 0, 0, None);
        self.execute_instructions()
    }

    pub fn run_function(&mut self, function: &RuntimeFunction, args: &[Value]) -> RuntimeResult {
        let current_chunk = self.chunk();
        let current_ip = self.ip();

        let expected_arg_count = if function.is_instance_function {
            function.arg_count + 1
        } else {
            function.arg_count
        };

        if expected_arg_count != args.len() as u8 {
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
                            Error::ExternalError { message } => Error::ExternalError {
                                message: wrap_message(message),
                            },
                            Error::LoaderError(loader::LoaderError { message, span }) => {
                                Error::LoaderError(loader::LoaderError {
                                    message: wrap_message(message),
                                    span,
                                })
                            }
                        })
                    };

                    if let Some(Value::Function(pre_test)) = &pre_test {
                        let pre_test_result = if pre_test.is_instance_function {
                            self.run_function(&pre_test.clone(), &self_arg)
                        } else {
                            self.run_function(&pre_test.clone(), &[])
                        };

                        if let Err(error) = pre_test_result {
                            return wrap_error_message(error, "Error while preparing to run test");
                        }
                    }

                    let test_result = if test.is_instance_function {
                        self.run_function(&test, &self_arg)
                    } else {
                        self.run_function(&test, &[])
                    };

                    if let Err(error) = test_result {
                        return wrap_error_message(error, "Error while running test");
                    }

                    if let Some(Value::Function(post_test)) = &post_test {
                        let post_test_result = if post_test.is_instance_function {
                            self.run_function(&post_test.clone(), &self_arg)
                        } else {
                            self.run_function(&post_test.clone(), &[])
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
                Ok(ControlFlow::ReturnValue(return_value)) => {
                    result = return_value;
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
                        self.set_register(register, Value::Str(Arc::new(error.to_string())));
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
            Value::RegisterList(value::RegisterList { start, count }) => {
                let mut copied = ValueVec::with_capacity(count as usize);
                for register in start..start + count {
                    copied.push(self.get_register(register).clone());
                }
                Value::List(ValueList::with_data(copied))
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
                self.set_register(target, self.copy_value(self.get_register(source)));
            }
            Instruction::DeepCopy { target, source } => {
                self.set_register(
                    target,
                    deep_copy_value(&self.copy_value(self.get_register(source))),
                );
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
                let string = self.arc_string_from_constant(constant);
                self.set_register(register, Str(string))
            }
            Instruction::LoadGlobal { register, constant } => {
                let global_name = self.get_constant_string(constant);
                let global = self.global.data().get_with_string(global_name).cloned();
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
                let global_name = self.arc_string_from_constant(global);
                self.global
                    .data_mut()
                    .insert(Str(global_name), self.get_register(source).clone());
            }
            Instruction::Import { register, constant } => {
                self.run_import(register, constant, instruction_ip)?;
            }
            Instruction::RegisterList {
                register,
                start,
                count,
            } => {
                self.set_register(register, RegisterList(value::RegisterList { start, count }));
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
            Instruction::MakeIterator { register, range } => {
                let iterator = match self.get_register(range) {
                    Range(int_range) => Iterator(ValueIterator::new(Iterable::Range(*int_range))),
                    List(list) => Iterator(ValueIterator::new(Iterable::List(list.clone()))),
                    Map(map) => Iterator(ValueIterator::new(Iterable::Map(map.clone()))),
                    unexpected => {
                        return vm_error!(
                            self.chunk(),
                            instruction_ip,
                            "Expected iterable value while making iterator, found '{}'",
                            type_as_string(&unexpected)
                        );
                    }
                };
                self.set_register(register, iterator);
            }
            Instruction::Function {
                register,
                arg_count,
                capture_count,
                size,
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
                    is_instance_function: false,
                });
                self.jump_ip(size);
                self.set_register(register, function);
            }
            Instruction::InstanceFunction {
                register,
                arg_count,
                capture_count,
                size,
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
                    is_instance_function: true,
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
                        let result = String::clone(a) + b.as_ref();
                        Str(Arc::new(result))
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
            Instruction::In { register, lhs, rhs } => {
                let lhs_value = self.get_register(lhs);
                let rhs_value = self.get_register(rhs);
                let result = match (lhs_value, rhs_value) {
                    (_, List(l)) => l.data().contains(lhs_value).into(),
                    (key, Map(m)) => m.data().contains_key(key).into(),
                    (Str(s1), Str(s2)) => s2.contains(s1.as_ref()).into(),
                    (Number(n), Range(r)) => (r.start..r.end).contains(&(*n as isize)).into(),
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
                    result = ControlFlow::ReturnValue(return_value);
                }
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

                self.set_register(register, Str(Arc::new(result)));
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
                    Some(ValueIteratorOutput::Value(value)) => self.set_register(register, value),
                    Some(ValueIteratorOutput::ValuePair(first, second)) => {
                        self.set_register(
                            register,
                            Value::RegisterList(value::RegisterList {
                                start: register + 1,
                                count: 2,
                            }),
                        );
                        self.set_register(register + 1, first);
                        self.set_register(register + 2, second);
                    }
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
                    RegisterList(value::RegisterList { start, count }) => {
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
                let key_string = self.arc_string_from_constant(key);
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
        let import_name = self.arc_string_from_constant(import_constant);

        let maybe_global = self.global.data().get_with_string(&import_name).cloned();
        if let Some(value) = maybe_global {
            self.set_register(result_register, value);
        } else {
            let maybe_in_prelude = self.prelude.data().get_with_string(&import_name).cloned();
            if let Some(value) = maybe_in_prelude {
                self.set_register(result_register, value);
            } else {
                let source_path = self.reader.chunk.source_path.clone();
                let (module_chunk, module_path) = match self
                    .loader
                    .compile_module(&import_name.as_str(), source_path)
                {
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
                let maybe_module = self.modules.get(&module_path).cloned();
                match maybe_module {
                    Some(module) => self.set_register(result_register, Value::Map(module)),
                    None => {
                        // Run the chunk, and cache the resulting global map
                        let mut vm = Vm::new();
                        vm.prelude = self.prelude.clone();
                        vm.run(module_chunk)?;
                        if let Some(main) = vm.get_global_function("main") {
                            vm.run_function(&main, &[])?;
                        }
                        self.modules.insert(module_path, vm.global.clone());
                        self.set_register(result_register, Value::Map(vm.global));
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
                                ValueIteratorOutput::Value(value) => value,
                                ValueIteratorOutput::ValuePair(_, _) => unreachable!(),
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
                    .core_lib
                    .$module
                    .data()
                    .get_with_string(&key_string)
                    .cloned();

                match maybe_op {
                    Some(op) => match op {
                        ExternalFunction(f) => {
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

                let args = self
                    .register_slice(arg_register, call_arg_count)
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>();

                let result = (&*function)(self, &args);
                match result {
                    Ok(value) => {
                        self.set_register(result_register, value);
                    }
                    Err(error) => {
                        match error {
                            Error::ExternalError { message } => {
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
                is_instance_function,
            }) => {
                if *is_instance_function {
                    if let Some(parent_register) = parent_register {
                        self.insert_register(
                            arg_register,
                            self.get_register(parent_register).clone(),
                        );
                    } else {
                        return vm_error!(
                            self.chunk(),
                            instruction_ip,
                            "Expected self for function"
                        );
                    }
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

                self.frame_mut().return_register_and_ip = Some((result_register, self.ip()));

                self.push_frame(chunk.clone(), *function_ip, arg_register, captures.clone());
            }
            unexpected => {
                return vm_error!(
                    self.chunk(),
                    instruction_ip,
                    "Expected Function, found '{}'",
                    type_as_string(&unexpected),
                )
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
                Value::RegisterList(value::RegisterList { start, count }) => {
                    // If the return value is a register list (i.e. when returning multiple values),
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

                    // Keep the register list values around after the frame has been popped
                    self.value_stack
                        .truncate(frame.register_base + count as usize);

                    Value::RegisterList(value::RegisterList {
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
            // (i.e. we need to make sure we've collected a RegisterList into a List).
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

    fn register_slice(&self, register: u8, count: u8) -> &[Value] {
        if count > 0 {
            let start = self.register_index(register);
            &self.value_stack[start..start + count as usize]
        } else {
            &[]
        }
    }

    fn get_constant_string(&self, constant_index: ConstantIndex) -> &str {
        self.reader.chunk.constants.get_string(constant_index)
    }

    fn arc_string_from_constant(&mut self, constant_index: ConstantIndex) -> Arc<String> {
        let constants_hash = self.reader.chunk.constants_hash;

        let maybe_string = self
            .string_constants
            .get(&(constants_hash, constant_index))
            .cloned();

        match maybe_string {
            Some(s) => s,
            None => {
                let s = Arc::new(
                    self.reader
                        .chunk
                        .constants
                        .get_string(constant_index)
                        .to_string(),
                );
                self.string_constants
                    .insert((constants_hash, constant_index), s.clone());
                s
            }
        }
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{external_error, Value::*, ValueHashMap},
        koto_bytecode::chunk_to_string_annotated,
    };

    fn test_script(script: &str, expected_output: Value) {
        let mut vm = Vm::new();

        vm.global.add_value("test_global", Number(42.0));
        vm.global.add_fn("assert", |_, args| {
            for value in args.iter() {
                match value {
                    Bool(b) => {
                        if !b {
                            return external_error!("Assertion failed");
                        }
                    }
                    unexpected => {
                        return external_error!(
                            "assert expects booleans as arguments, found '{}'",
                            type_as_string(unexpected),
                        )
                    }
                }
            }
            Ok(Empty)
        });

        let chunk = match vm.loader.compile_script(script, &None) {
            Ok(chunk) => chunk,
            Err(error) => panic!(error.message),
        };

        let print_chunk = |script: &str, chunk| {
            println!("{}\n", script);
            let script_lines = script.lines().collect::<Vec<_>>();

            println!("{}", chunk_to_string_annotated(chunk, &script_lines));
        };

        match vm.run(chunk) {
            Ok(result) => {
                if result != expected_output {
                    print_chunk(script, vm.chunk());
                }
                assert_eq!(result, expected_output);
            }
            Err(e) => {
                print_chunk(script, vm.chunk());
                panic!(format!("Error while running script: {}", e.to_string()));
            }
        }
    }

    fn number_list<T>(values: &[T]) -> Value
    where
        T: Copy,
        f64: From<T>,
    {
        let values = values
            .iter()
            .map(|n| Number(f64::from(*n)))
            .collect::<Vec<_>>();
        value_list(&values)
    }

    fn value_list(values: &[Value]) -> Value {
        List(ValueList::from_slice(&values))
    }

    fn num2(a: f64, b: f64) -> Value {
        Num2(koto_parser::num2::Num2(a, b))
    }

    fn num4(a: f32, b: f32, c: f32, d: f32) -> Value {
        Num4(koto_parser::num4::Num4(a, b, c, d))
    }

    fn string(s: &str) -> Value {
        Str(Arc::new(s.to_string()))
    }

    mod literals {
        use super::*;

        #[test]
        fn empty() {
            test_script("()", Empty);
        }

        #[test]
        fn bool_true() {
            test_script("true", Bool(true));
        }

        #[test]
        fn bool_false() {
            test_script("false", Bool(false));
        }

        #[test]
        fn number() {
            test_script("24.0", Number(24.0));
        }

        #[test]
        fn string() {
            test_script("\"Hello\"", Str(Arc::new("Hello".to_string())));
        }
    }

    mod operators {
        use super::*;

        #[test]
        fn add_multiply() {
            test_script("1 + 2 * 3 + 4", Number(11.0));
        }

        #[test]
        fn subtract_divide_modulo() {
            test_script("(20 - 2) / 3 % 4", Number(2.0));
        }

        #[test]
        fn comparison() {
            test_script(
                "false or 1 < 2 <= 2 <= 3 and 3 >= 2 >= 2 > 1 or false",
                Bool(true),
            );
        }

        #[test]
        fn equality() {
            test_script("1 + 1 == 2 and 2 + 2 != 5", Bool(true));
        }

        #[test]
        fn not_bool() {
            test_script("not false", Bool(true));
        }

        #[test]
        fn not_expression() {
            test_script("not 1 + 1 == 2", Bool(false));
        }

        #[test]
        fn assignment() {
            let script = "
a = 1 * 3
a + 1";
            test_script(script, Number(4.0));
        }

        #[test]
        fn negation() {
            let script = "
a = 99
-a";
            test_script(script, Number(-99.0));
        }
    }

    mod ranges {
        use super::*;

        #[test]
        fn range() {
            test_script("0..10", Range(IntRange { start: 0, end: 10 }));
            test_script("0..-10", Range(IntRange { start: 1, end: -9 }));
        }

        #[test]
        fn range_inclusive() {
            test_script("10..=20", Range(IntRange { start: 10, end: 21 }));
            test_script("4..=0", Range(IntRange { start: 5, end: 0 }));
        }

        #[test]
        fn in_operator() {
            let script = "
assert 10 in 5..15
assert 10 in 0..=10
assert not 10 in 0..10";

            test_script(script, Empty);
        }

        #[test]
        fn subtract_divide_modulo() {
            test_script("(20 - 2) / 3 % 4", Number(2.0));
        }

        #[test]
        fn comparison() {
            test_script(
                "false or 1 < 2 <= 2 <= 3 and 3 >= 2 >= 2 > 1 or false",
                Bool(true),
            );
        }

        #[test]
        fn equality() {
            test_script("1 + 1 == 2 and 2 + 2 != 5", Bool(true));
        }

        #[test]
        fn not_bool() {
            test_script("not false", Bool(true));
        }

        #[test]
        fn not_expression() {
            test_script("not 1 + 1 == 2", Bool(false));
        }

        #[test]
        fn assignment() {
            let script = "
a = 1 * 3
a + 1";
            test_script(script, Number(4.0));
        }

        #[test]
        fn negation() {
            let script = "
a = 99
-a";
            test_script(script, Number(-99.0));
        }
    }

    mod lists {
        use super::*;

        #[test]
        fn empty() {
            test_script("[]", List(ValueList::default()));
        }

        #[test]
        fn literals() {
            test_script("[1 2 3 4]", number_list(&[1, 2, 3, 4]));
        }

        #[test]
        fn from_ids() {
            let script = "
a = 1
[a a a]";
            test_script(script, number_list(&[1, 1, 1]));
        }

        #[test]
        fn from_range() {
            test_script("[3..0]", number_list(&[3, 2, 1]));
        }

        #[test]
        fn from_multiple_ranges() {
            test_script("[0..3 3..=0]", number_list(&[0, 1, 2, 3, 2, 1, 0]));
        }

        #[test]
        fn access_element() {
            let script = "
a = [1 2 3]
a[1]";
            test_script(script, Number(2.0));
        }

        #[test]
        fn access_range() {
            let script = "
a = [10 20 30 40 50]
a[1..3]";
            test_script(script, number_list(&[20, 30]));
        }

        #[test]
        fn access_range_inclusive() {
            let script = "
a = [10 20 30 40 50]
a[1..=3]";
            test_script(script, number_list(&[20, 30, 40]));
        }

        #[test]
        fn access_range_to() {
            let script = "
a = [10 20 30 40 50]
a[..2]";
            test_script(script, number_list(&[10, 20]));
        }

        #[test]
        fn access_range_to_inclusive() {
            let script = "
a = [10 20 30 40 50]
a[..=2]";
            test_script(script, number_list(&[10, 20, 30]));
        }

        #[test]
        fn access_range_from() {
            let script = "
a = [10 20 30 40 50]
a[2..]";
            test_script(script, number_list(&[30, 40, 50]));
        }

        #[test]
        fn access_range_full() {
            let script = "
a = [10 20 30 40 50]
a[..]";
            test_script(script, number_list(&[10, 20, 30, 40, 50]));
        }

        #[test]
        fn assign_element() {
            let script = "
a = [1 2 3]
x = 2
a[x] = -1
a";
            test_script(script, number_list(&[1, 2, -1]));
        }

        #[test]
        fn assign_range() {
            let script = "
a = [1 2 3 4 5]
a[1..=3] = 0
a";
            test_script(script, number_list(&[1, 0, 0, 0, 5]));
        }

        #[test]
        fn assign_range_to() {
            let script = "
a = [1 2 3 4 5]
a[..3] = 0
a";
            test_script(script, number_list(&[0, 0, 0, 4, 5]));
        }

        #[test]
        fn assign_range_to_inclusive() {
            let script = "
a = [1 2 3 4 5]
a[..=3] = 8
a";
            test_script(script, number_list(&[8, 8, 8, 8, 5]));
        }

        #[test]
        fn assign_range_from() {
            let script = "
a = [1 2 3 4 5]
a[2..] = 9
a";
            test_script(script, number_list(&[1, 2, 9, 9, 9]));
        }

        #[test]
        fn assign_range_full() {
            let script = "
a = [1 2 3 4 5]
a[..] = 9
a";
            test_script(script, number_list(&[9, 9, 9, 9, 9]));
        }

        #[test]
        fn addition() {
            test_script("[1 2 3] + [4 5]", number_list(&[1, 2, 3, 4, 5]));
        }

        #[test]
        fn shared_data_by_default() {
            let script = "
l = [1 2 3]
l2 = l
l[1] = -1
l2[1]";
            test_script(script, Number(-1.0));
        }

        #[test]
        fn copy() {
            let script = "
l = [1 2 3]
l2 = copy l
l[1] = -1
l2[1]";
            test_script(script, Number(2.0));
        }

        #[test]
        fn in_operator() {
            let script = r#"
assert -1 in [2 -1 5]
assert not 7 in [2 -1 5]
assert "foo" in [0 [] "foo"]
"#;
            test_script(script, Empty);
        }
    }

    mod multi_assignment {
        use super::*;

        #[test]
        fn assign_2_to_1() {
            let script = "
a = 1, 2
a";
            test_script(script, number_list(&[1, 2]));
        }

        #[test]
        fn assign_1_to_2() {
            let script = "
a, b = -1
a, b";
            test_script(script, value_list(&[Number(-1.0), Empty]));
        }

        #[test]
        fn list_elements_1_to_2() {
            let script = "
x = [0 0]
x[0], x[1] = 99
x";
            test_script(script, value_list(&[Number(99.0), Empty]));
        }

        #[test]
        fn list_elements_2_to_2() {
            let script = "
x = [0 0]
x[0], x[1] = -1, 42
x";
            test_script(script, number_list(&[-1, 42]));
        }

        #[test]
        fn unpack_list() {
            let script = "
a, b, c = [7 8]
a, b, c";
            test_script(script, value_list(&[Number(7.0), Number(8.0), Empty]));
        }

        #[test]
        fn multiple_lists() {
            let script = "
a, b, c = [1 2], [3 4]
a, b, c";
            test_script(
                script,
                value_list(&[number_list(&[1, 2]), number_list(&[3, 4]), Empty]),
            );
        }
    }

    mod if_expressions {
        use super::*;

        #[test]
        fn if_no_else() {
            let script = "
x = if 5 < 4
  42
x
";
            test_script(script, Empty);
        }

        #[test]
        fn if_else_if_result_from_if() {
            let script = "
x = if 5 > 4
  42
else if 1 < 2
  -1
else
  99
x";
            test_script(script, Number(42.0));
        }

        #[test]
        fn if_else_if_result_from_else_if() {
            let script = "
x = if 5 < 4
  42
else if 1 < 2
  -1
else
  99
x";
            test_script(script, Number(-1.0));
        }

        #[test]
        fn if_else_if_result_from_else() {
            let script = "
x = if 5 < 4
  42
else if 2 < 1
  -1
else
  99
x";
            test_script(script, Number(99.0));
        }

        #[test]
        fn multiple_else_ifs() {
            let script = "
x = if false
  42
else if false
  -1
else if false
  99
else if true
  100
else
  0
x";
            test_script(script, Number(100.0));
        }
    }

    mod match_expressions {
        use super::*;

        #[test]
        fn match_assignment() {
            let script = "
x = match 0 == 1
  true then 42
  false then 99
x
";
            test_script(script, Number(99.0));
        }

        #[test]
        fn match_multiple() {
            let script = r#"
x = 11
match x % 3, x % 5
  0, 0 then "Fizz Buzz"
  0, _ then "Fizz"
  _, 0 then "Buzz"
  _ then x
"#;
            test_script(script, Number(11.0));
        }

        #[test]
        fn match_with_condition() {
            let script = r#"
x = "hello"
match x
  "goodbye" then 1
  () then 99
  y if y == "O_o" then -1
  y if y == "hello"
    42
"#;
            test_script(script, Number(42.0));
        }

        #[test]
        fn match_on_alternative() {
            let script = "
match 42
  1 or 2 then 11
  3 or 4 or 5 then 22
  21 or 42 then 33
  _ then 44
";
            test_script(script, Number(33.0));
        }

        #[test]
        fn match_on_multiple_expressions_with_alternatives_wildcard() {
            let script = "
match 0, 1
  0, 0 or 1, 1 then -1
  _, 0 or _, 99 then -2
  x, 0 or x, [1] then -3
  0, _ or 1, _ then -4 # The first alternative (0, _) should match
  _ then -5
";
            test_script(script, Number(-4.0));
        }

        #[test]
        fn match_on_multiple_expressions_with_alternatives_id() {
            let script = "
match 0, 1
  0, 0 or 1, 1 then -1
  _, 0 or _, 99 then -2
  x, 1 or x, [1] then -3 # The first alternative (x, 1) should match
  0, _ or 1, _ then -4
  _ then -5
";
            test_script(script, Number(-3.0));
        }

        #[test]
        fn match_map_result() {
            let script = r#"
m = match "hello"
  "foo"
    value_1: -1
    value_2: 99
  "hello"
    value_1: 4
    value_2: 20
  _
    value_1: 10
    value_2: 7
m.value_1 + m.value_2
"#;
            test_script(script, Number(24.0));
        }
    }

    mod globals {
        use super::*;

        #[test]
        fn load_value() {
            test_script("test_global", Number(42.0));
        }

        #[test]
        fn function() {
            test_script("assert 1 + 1 == 2", Empty);
        }

        #[test]
        fn function_two_args() {
            test_script("assert (1 + 1 == 2) (2 < 3)", Empty);
        }
    }

    mod functions {
        use super::*;

        #[test]
        fn no_args() {
            let script = "
f = || 42
f()";
            test_script(script, Number(42.0));
        }

        #[test]
        fn single_arg() {
            let script = "
square = |x| x * x
square 8";
            test_script(script, Number(64.0));
        }

        #[test]
        fn two_args() {
            let script = "
add = |a b|
  a + b
add 5 6";
            test_script(script, Number(11.0));
        }

        #[test]
        fn nested_function() {
            let script = "
add = |a b|
  add2 = |x y| x + y
  add2 a b
add 10 20";
            test_script(script, Number(30.0));
        }

        #[test]
        fn nested_calls() {
            let script = "
add = |a b| a + b
add 10 (add 20 30)";
            test_script(script, Number(60.0));
        }

        #[test]
        fn recursive_call() {
            let script = "
f = |n|
  if n == 0
    0
  else
    f n - 1
f 4
";
            test_script(script, Number(0.0));
        }

        #[test]
        fn recursive_call_fib() {
            let script = "
fib = |n|
  if n <= 0
    0
  else if n == 1
    1
  else
    (fib n - 1) + (fib n - 2)
fib 4
";
            test_script(script, Number(3.0));
        }

        #[test]
        fn recursive_call_via_multi_assign() {
            let script = "
f, g = (|n| if n == 0 then 1 else f n - 1), (|n| if n == 0 then 2 else g n - 1)
f 4, g 4
";
            test_script(script, number_list(&[1, 2]));
        }

        #[test]
        fn multiple_return_values() {
            let script = "
f = |x| x - 1, x + 1
a, b = f 0
a, b";
            test_script(script, number_list(&[-1, 1]));
        }

        #[test]
        fn return_no_value() {
            let script = "
f = |x|
  if x < 0
    return
  x
f -42";
            test_script(script, Empty);
        }

        #[test]
        fn return_expression() {
            let script = "
f = |x|
  if x < 0
    return x * -1
  x
f -42";
            test_script(script, Number(42.0));
        }

        #[test]
        fn captured_value() {
            let script = "
f = |x|
  inner = || x * x
  inner()
f 3";
            test_script(script, Number(9.0));
        }

        #[test]
        fn capture_via_mutation() {
            let script = "
data = [1 2 3]
f = ||
  data[1] = 99
  data = () # reassignment doesn't affect the original copy of data
f()
data[1]";
            test_script(script, Number(99.0));
        }

        #[test]
        fn nested_captured_values() {
            let script = "
capture_test = |a b c|
  inner = ||
    inner2 = |x|
      x + b + c
    inner2 a
  b, c = (), () # inner and inner2 have captured their own copies of b and c
  inner()
capture_test 1 2 3";
            test_script(script, Number(6.0));
        }

        #[test]
        fn modifying_a_captured_value() {
            let script = "
make_counter = ||
  count = 0
  return || count += 1
c = make_counter()
c2 = make_counter()
assert c() == 1
assert c() == 2
assert c2() == 1
assert c() == 3
c2()";
            test_script(script, Number(2.0));
        }

        #[test]
        fn multi_assignment_of_captured_values() {
            let script = "
f = |x|
  inner = ||
    x[0], x[1] = x[0] + 1, x[1] + 1
  inner()
  x
f [1 2]";
            test_script(script, number_list(&[2, 3]));
        }

        #[test]
        fn export_assignment() {
            let script = "
f = ||
  export x = 42
f()
x";
            test_script(script, Number(42.0));
        }

        #[test]
        fn multi_assignment_of_function_results() {
            let script = "
f = |n| n
a, b = f 1, f 2
a";
            test_script(script, Number(1.0));
        }

        #[test]
        fn function_blocks_as_args_dont_break_assignment() {
            // The nested block (as first arg to a call to f) in f2 broke parsing,
            // so that f3 wasn't assigned correctly,
            // and then couldn't be found after assignment.
            let script = "
f = |x| x
f2 = ||
  f |x|
    x
f3 = |x| f2() x
f3 1";
            test_script(script, Number(1.0));
        }

        #[test]
        fn function_blocks_as_args_dont_break_assignment_during_lookup() {
            // See comment in test above, the same applies to args in the lookup call to f.g
            let script = "
f = { g: |x| x }
f2 = ||
  f.g |x|
    x
f3 = |x| f2() x
f3 1";
            test_script(script, Number(1.0));
        }
    }

    mod loops {
        use super::*;

        #[test]
        fn while_loop() {
            let script = "
count = 0
(count += 1) while count < 10
count";
            test_script(script, Number(10.0));
        }

        #[test]
        fn until_loop() {
            let script = "
count = 10
(count += 1) until count == 20
count";
            test_script(script, Number(20.0));
        }

        #[test]
        fn for_loop() {
            let script = "
count = 32
(count += 1) for _ in 0..10
count";
            test_script(script, Number(42.0));
        }

        #[test]
        fn for_conditional() {
            let script = "
count = 0
(count += 1) for i in 0..10 if i > 4
count";
            test_script(script, Number(5.0));
        }

        #[test]
        fn for_list() {
            let script = "
sum = 0
(sum += a) for a in [10 20 30 40]
sum";
            test_script(script, Number(100.0));
        }

        #[test]
        fn for_break() {
            let script = "
sum = 0
for i in 1..10
  if i == 5
    break
  sum += i
sum";
            test_script(script, Number(10.0));
        }

        #[test]
        fn for_break_nested() {
            let script = "
sum = 0
for i in [1 2 3]
  for j in 0..5
    if j == 2
      break
    sum += i
sum";
            test_script(script, Number(12.0));
        }

        #[test]
        fn for_continue() {
            let script = "
sum = 0
for i in 1..10
  if i > 5
    continue
  sum += i
sum";
            test_script(script, Number(15.0));
        }

        #[test]
        fn for_continue_nested() {
            let script = "
sum = 0
for i in [2 4 6]
  for j in 0..10
    if j > 1
      continue
    sum += i
sum";
            test_script(script, Number(24.0));
        }

        #[test]
        fn while_break() {
            let script = "
i, sum = 0, 0
while (i += 1) < 1000000
  if i > 5
    break
  sum += 1
sum";
            test_script(script, Number(5.0));
        }

        #[test]
        fn while_continue() {
            let script = "
i, sum = 0, 0
while (i += 1) < 10
  if i > 6
    continue
  sum += 1
sum";
            test_script(script, Number(6.0));
        }

        #[test]
        fn return_from_nested_loop() {
            let script = "
f = ||
  for i in 0..100
    for j in 0..100
      if i == j == 5
        return i
  -1
f()";
            test_script(script, Number(5.0));
        }

        #[test]
        fn multiple_ranges_2_to_1() {
            let script = "
sum = 0
for a, b in [[1 2] [3 4]]
  sum += a + b
sum
";
            test_script(script, Number(10.0));
        }

        #[test]
        fn multiple_ranges_2_to_2() {
            let script = "
sum = 0
for a, b in [1 2 3], [4 5 6]
  sum += a + b
sum
";
            test_script(script, Number(21.0));
        }
    }

    mod maps {
        use super::*;

        #[test]
        fn empty() {
            test_script("{}", Map(ValueMap::new()));
        }

        #[test]
        fn from_literals() {
            let mut result_data = ValueHashMap::new();
            result_data.add_value("foo", Number(42.0));
            result_data.add_value("bar", Str(Arc::new("baz".to_string())));

            test_script(
                "{foo: 42, bar: \"baz\"}",
                Map(ValueMap::with_data(result_data)),
            );
        }

        #[test]
        fn access() {
            let script = "
m = {foo: -1}
m.foo";
            test_script(script, Number(-1.0));
        }

        #[test]
        fn insert() {
            let script = "
m = {}
m.foo = 42
m.foo";
            test_script(script, Number(42.0));
        }

        #[test]
        fn update() {
            let script = "
m = {bar: -1}
m.bar = 99
m.bar";
            test_script(script, Number(99.0));
        }

        #[test]
        fn implicit_values() {
            let script = "
foo, baz = 42, -1
m = {foo, bar: 99, baz}
m.baz";
            test_script(script, Number(-1.0));
        }

        #[test]
        fn instance_function_no_args() {
            let script = "
make_o = || {foo: 42, get_foo: |self| self.foo}
o = make_o()
o.get_foo()";
            test_script(script, Number(42.0));
        }

        #[test]
        fn instance_function_with_args() {
            let script = "
make_o = || {foo: 0, set_foo: |self a b| self.foo = a + b}
o = make_o()
o.set_foo 10 20
o.foo";
            test_script(script, Number(30.0));
        }

        #[test]
        fn addition() {
            let script = "
m = {foo: -1, bar: 42} + {foo: 99}
[m.foo m.bar]";
            test_script(script, number_list(&[99, 42]));
        }

        #[test]
        fn equality() {
            let script = "
m = {foo: 42, bar: || 99}
m2 = m
m == m2";
            test_script(script, Bool(true));
        }

        #[test]
        fn inequality() {
            let script = "
m = {foo: 42, bar: || 99}
m2 = copy m
m2.foo = 99
m != m2";
            test_script(script, Bool(true));
        }

        #[test]
        fn shared_data_by_default() {
            let script = "
m = {foo: 42}
m2 = m
m.foo = -1
m2.foo";
            test_script(script, Number(-1.0));
        }

        #[test]
        fn copy() {
            let script = "
m = {foo: 42}
m2 = copy m
m.foo = -1
m2.foo";
            test_script(script, Number(42.0));
        }

        #[test]
        fn in_operator() {
            let script = r#"
m = {foo: 42, bar: 0}
assert "foo" in m
assert not "baz" in m
"#;
            test_script(script, Empty);
        }
    }

    mod lookups {
        use super::*;

        #[test]
        fn list_in_map() {
            let script = "
m = {x: [100 200]}
m.x[1]";
            test_script(script, Number(200.0));
        }

        #[test]
        fn map_in_list() {
            let script = "
m = {foo: 99}
l = [m m m]
l[2].foo";
            test_script(script, Number(99.0));
        }

        #[test]
        fn assign_to_map_in_list() {
            let script = "
m = {bar: 0}
l = [m m m]
l[1].bar = -1
l[1].bar";
            test_script(script, Number(-1.0));
        }

        #[test]
        fn assign_to_list_in_map_in_list() {
            let script = "
m = {foo: [1 2 3]}
l = [m m m]
l[2].foo[0] = 99
l[2].foo[0]";
            test_script(script, Number(99.0));
        }

        #[test]
        fn function_call() {
            let script = "
m = {get_map: || { foo: -1 }}
m.get_map().foo";
            test_script(script, Number(-1.0));
        }

        #[test]
        fn copy_nested() {
            let script = "
m = {foo: {bar: -1}}
m2 = copy m.foo
m.foo.bar = 99
m2.bar";
            test_script(script, Number(-1.0));
        }

        #[test]
        fn copy_from_expression() {
            let script = "
m = {foo: {bar: 88}, get_foo: |self| self.foo}
m2 = copy (m.get_foo())
m.get_foo().bar = 99
m2.bar";
            test_script(script, Number(88.0));
        }

        #[test]
        fn capture_in_map_block() {
            let script = "
x = 42
make_map = ||
  foo: x
m = make_map()
m.foo
";
            test_script(script, Number(42.0));
        }
    }

    mod placeholders {
        use super::*;

        #[test]
        fn placeholder_in_assignment() {
            let script = "
f = || 1, 2, 3
a, _, c = f()
a, c";
            test_script(script, number_list(&[1, 3]));
        }

        #[test]
        fn placeholder_argument() {
            let script = "
fold = |xs f|
  result = 0
  for x in xs
    result = f result x
  result
fold 0..5 |n _| n + 1";
            test_script(script, Number(5.0));
        }
    }

    mod list_comprehensions {
        use super::*;

        #[test]
        fn for_loop() {
            test_script("[x for x in 0..5]", number_list(&[0, 1, 2, 3, 4]));
        }

        #[test]
        fn conditional_for() {
            let script = "
f = |x| x * x
[f(x) for x in [2 3 4] if x % 2 == 0]";
            test_script(script, number_list(&[4, 16]));
        }

        #[test]
        fn while_loop() {
            let script = "
x = 0
[(x += 1) while x < 3]";
            test_script(script, number_list(&[1, 2, 3]));
        }

        #[test]
        fn for_loop_function_calls() {
            let script = "
count = 0
f = |n| count += n
x = [f 1 for _ in 0..5]";
            test_script(script, number_list(&[1, 2, 3, 4, 5]));
        }

        #[test]
        fn while_loop_function_calls() {
            let script = "
count = 0
f = |n| n
x = [f count while (count += 1) <= 5]";
            test_script(script, number_list(&[1, 2, 3, 4, 5]));
        }
    }

    mod num2_test {
        use super::*;

        #[test]
        fn with_1_arg_1() {
            test_script("num2 1", num2(1.0, 1.0));
        }

        #[test]
        fn with_1_arg_2() {
            test_script("num2 2", num2(2.0, 2.0));
        }

        #[test]
        fn with_2_args() {
            test_script("num2 1 2", num2(1.0, 2.0));
        }

        #[test]
        fn from_list() {
            test_script("num2 [-1]", num2(-1.0, 0.0));
        }

        #[test]
        fn from_num2() {
            test_script("num2 (num2 1 2)", num2(1.0, 2.0));
        }

        #[test]
        fn add_multiply() {
            test_script("(num2 1) + (num2 0.5) * 3.0", num2(2.5, 2.5));
        }

        #[test]
        fn subtract_divide() {
            test_script("((num2 10 20) - (num2 2)) / 2.0", num2(4.0, 9.0));
        }

        #[test]
        fn modulo() {
            test_script("(num2 15 25) % (num2 10) % 4", num2(1.0, 1.0));
        }

        #[test]
        fn negation() {
            let script = "
x = num2 1 -2
-x";
            test_script(script, num2(-1.0, 2.0));
        }

        #[test]
        fn index() {
            let script = "
x = num2 4 5
x[1]";
            test_script(script, Number(5.0));
        }
    }

    mod num4_test {
        use super::*;

        #[test]
        fn with_1_arg_1() {
            test_script("num4 1", num4(1.0, 1.0, 1.0, 1.0));
        }

        #[test]
        fn with_1_arg_2() {
            test_script("num4 2", num4(2.0, 2.0, 2.0, 2.0));
        }

        #[test]
        fn with_2_args() {
            test_script("num4 1 2", num4(1.0, 2.0, 0.0, 0.0));
        }

        #[test]
        fn with_3_args() {
            test_script("num4 3 2 1", num4(3.0, 2.0, 1.0, 0.0));
        }

        #[test]
        fn with_4_args() {
            test_script("num4 -1 1 -2 2", num4(-1.0, 1.0, -2.0, 2.0));
        }

        #[test]
        fn from_list() {
            test_script("num4 [-1 1]", num4(-1.0, 1.0, 0.0, 0.0));
        }

        #[test]
        fn from_num2() {
            test_script("num4 (num2 1 2)", num4(1.0, 2.0, 0.0, 0.0));
        }

        #[test]
        fn from_num4() {
            test_script("num4 (num4 3 4)", num4(3.0, 4.0, 0.0, 0.0));
        }

        #[test]
        fn add_multiply() {
            test_script("(num4 1) + (num4 0.5) * 3.0", num4(2.5, 2.5, 2.5, 2.5));
        }

        #[test]
        fn subtract_divide() {
            test_script(
                "((num4 10 20 30 40) - (num4 2)) / 2.0",
                num4(4.0, 9.0, 14.0, 19.0),
            );
        }

        #[test]
        fn modulo() {
            test_script(
                "(num4 15 25 35 45) % (num4 10) % 4",
                num4(1.0, 1.0, 1.0, 1.0),
            );
        }

        #[test]
        fn negation() {
            let script = "
x = num4 1 -2 3 -4
-x";
            test_script(script, num4(-1.0, 2.0, -3.0, 4.0));
        }

        #[test]
        fn index() {
            let script = "
x = num4 9 8 7 6
x[3]";
            test_script(script, Number(6.0));
        }
    }

    mod strings {
        use super::*;

        #[test]
        fn addition() {
            test_script(r#""Hello, " + "World!""#, string("Hello, World!"));
        }

        #[test]
        fn in_operator() {
            let script = r#"
assert "Hello" in "Hello, World!"
assert not "Hello" in "World!"
"#;
            test_script(script, Empty);
        }
    }

    mod error_recovery {
        use super::*;

        #[test]
        fn try_catch() {
            let script = "
x = 1
try
  x += 1
  x += y
catch _
  x + 1
";
            test_script(script, Number(3.0));
        }

        #[test]
        fn try_catch_finally() {
            let script = "
try
  x
catch e
  -1
finally
  99
";
            test_script(script, Number(99.0));
        }

        #[test]
        fn try_catch_nested() {
            let script = "
x = 0
try
  x += 1
  try
    x += 1
    x += y
  catch _
    x += 1
  x += y
catch _
  x += 1
";
            test_script(script, Number(4.0));
        }
    }
}
