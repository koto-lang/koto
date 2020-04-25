#![allow(dead_code)]

use crate::{
    type_as_string,
    value_iterator::{IntRange, Iterable, ValueIterator2},
    vm_error, Id, Runtime, RuntimeResult, Value, ValueList, ValueMap,
};
use koto_bytecode::{Bytecode, Instruction, InstructionReader};
use koto_parser::ConstantPool;
use rustc_hash::FxHashMap;
use std::sync::Arc;

#[derive(Debug, Default)]
struct Frame {
    base: usize,
    return_ip: usize,
    result: Value,
}

impl Frame {
    fn new(base: usize, return_ip: usize) -> Self {
        Self {
            base,
            return_ip,
            ..Default::default()
        }
    }
}

#[derive(Default)]
pub struct Vm {
    global: ValueMap,
    constants: ConstantPool,
    string_constants: FxHashMap<usize, Arc<String>>,
    value_stack: Vec<Value>,
    call_stack: Vec<Frame>,
}

impl Vm {
    pub fn new() -> Self {
        Self {
            value_stack: Vec::with_capacity(32),
            ..Default::default()
        }
    }

    pub fn run(&mut self, bytecode: &Bytecode) -> RuntimeResult {
        use Value::*;

        self.value_stack.clear();
        self.call_stack.clear();
        self.call_stack.push(Frame::default());
        let mut result = Empty;

        let mut reader = InstructionReader::new(bytecode);

        while let Some(instruction) = reader.next() {
            match instruction {
                Instruction::Error { message } => {
                    return vm_error!(reader.ip, "{}", message);
                }
                Instruction::Copy { target, source } => {
                    self.set_register(target, self.get_register(source).clone());
                }
                Instruction::SetEmpty { register } => self.set_register(register, Empty),
                Instruction::SetTrue { register } => self.set_register(register, Bool(true)),
                Instruction::SetFalse { register } => self.set_register(register, Bool(false)),
                Instruction::Return { register } => {
                    self.frame_mut().result = self.get_register(register).clone();

                    let return_ip = self.frame().return_ip;
                    result = self.pop_frame()?;

                    if self.call_stack.is_empty() {
                        break;
                    } else {
                        reader.ip = return_ip;
                    }
                }
                Instruction::LoadNumber { register, constant } => {
                    self.set_register(register, Number(self.constants.get_f64(constant as usize)))
                }
                Instruction::LoadString { register, constant } => {
                    let string = self.arc_string_from_constant(constant);
                    self.set_register(register, Str(string))
                }
                Instruction::LoadGlobal { register, constant } => {
                    let global_name = self.get_constant_string(constant as usize);
                    let global = self.global.data().get(global_name).cloned();
                    match global {
                        Some(value) => self.set_register(register, value),
                        None => {
                            return vm_error!(reader.ip, "global '{}' not found", global_name);
                        }
                    }
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
                Instruction::RangeExclusive {
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
                                reader.ip,
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
                                reader.ip,
                                "Expected numbers for range bounds, found start: {}, end: {}",
                                type_as_string(&unexpected.0),
                                type_as_string(&unexpected.1)
                            )
                        }
                    };
                    self.set_register(register, range);
                }
                Instruction::MakeIterator { register, range } => {
                    let iterator = match self.get_register(range) {
                        Range(int_range) => {
                            Iterator(ValueIterator2::new(Iterable::Range(*int_range)))
                        }
                        List(list) => Iterator(ValueIterator2::new(Iterable::List(list.clone()))),
                        Map(_) => {
                            unimplemented!("MakeIterator - List");
                        }
                        unexpected => {
                            return vm_error!(
                                reader.ip,
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
                    size,
                } => {
                    let function = VmFunction {
                        ip: reader.ip,
                        arg_count,
                        is_instance_function: false,
                    };
                    reader.ip += size;
                    self.set_register(register, function);
                }
                Instruction::InstanceFunction {
                    register,
                    arg_count,
                    size,
                } => {
                    let function = VmFunction {
                        ip: reader.ip,
                        arg_count,
                        is_instance_function: true,
                    };
                    reader.ip += size;
                    self.set_register(register, function);
                }
                Instruction::Add { register, lhs, rhs } => {
                    let lhs_value = self.get_register(lhs);
                    let rhs_value = self.get_register(rhs);
                    let result = match (&lhs_value, &rhs_value) {
                        (Number(a), Number(b)) => Number(a + b),
                        _ => {
                            return binary_op_error(instruction, lhs_value, rhs_value, reader.ip);
                        }
                    };
                    self.set_register(register, result);
                }
                Instruction::Multiply { register, lhs, rhs } => {
                    let lhs_value = self.get_register(lhs);
                    let rhs_value = self.get_register(rhs);
                    let result = match (&lhs_value, &rhs_value) {
                        (Number(a), Number(b)) => Number(a * b),
                        _ => {
                            return binary_op_error(instruction, lhs_value, rhs_value, reader.ip);
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
                            return binary_op_error(instruction, lhs_value, rhs_value, reader.ip);
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
                            return binary_op_error(instruction, lhs_value, rhs_value, reader.ip);
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
                    reader.ip += offset;
                }
                Instruction::JumpIf {
                    register,
                    offset,
                    jump_condition,
                } => match self.get_register(register) {
                    Bool(b) => {
                        if *b == jump_condition {
                            reader.ip += offset;
                        }
                    }
                    unexpected => {
                        return vm_error!(
                            reader.ip,
                            "Expected Bool, found '{}'",
                            type_as_string(&unexpected),
                        );
                    }
                },
                Instruction::JumpBack { offset } => {
                    reader.ip -= offset;
                }
                Instruction::JumpBackIf {
                    register,
                    offset,
                    jump_condition,
                } => match self.get_register(register) {
                    Bool(b) => {
                        if *b == jump_condition {
                            reader.ip -= offset;
                        }
                    }
                    unexpected => {
                        return vm_error!(
                            reader.ip,
                            "Expected Bool, found '{}'",
                            type_as_string(&unexpected),
                        );
                    }
                },
                Instruction::Call {
                    register,
                    arg_register,
                    arg_count,
                } => {
                    self.call_function(&mut reader, register, arg_register, arg_count, None)?;
                }
                Instruction::CallChild {
                    register,
                    parent,
                    arg_register,
                    arg_count,
                } => {
                    self.call_function(
                        &mut reader,
                        register,
                        arg_register,
                        arg_count,
                        Some(parent),
                    )?;
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
                                reader.ip,
                                "Expected Iterator, found '{}'",
                                type_as_string(&unexpected),
                            )
                        }
                    };

                    match result {
                        Some(value) => self.set_register(register, value),
                        None => reader.ip += jump_offset,
                    };
                }
                Instruction::ListPush { register, value } => {
                    let value = self.get_register(value).clone();

                    match self.get_register_mut(register) {
                        List(list) => match value {
                            Range(range) => {
                                list.data_mut()
                                    .extend(ValueIterator2::new(Iterable::Range(range)));
                            }
                            _ => list.data_mut().push(value),
                        },
                        unexpected => {
                            return vm_error!(
                                reader.ip,
                                "Expected List, found '{}'",
                                type_as_string(&unexpected),
                            )
                        }
                    };
                }
                Instruction::ListUpdate {
                    list,
                    index,
                    value: value_register,
                } => {
                    let index_value = self.get_register(index).clone();
                    let value = self.get_register(value_register).clone();

                    match self.get_register_mut(list) {
                        List(list) => match index_value {
                            Number(index) => match list.data_mut().get_mut(index as usize) {
                                Some(element) => *element = value,
                                None => {
                                    return vm_error!(reader.ip, "Index '{}' not in List", index);
                                }
                            },
                            Range(IntRange { start, end }) => {
                                let ustart = start as usize;
                                let uend = end as usize;

                                if start < 0 || end < 0 {
                                    return vm_error!(
                                        reader.ip,
                                        "Indexing with negative indices isn't supported, \
                                         start: {}, end: {}",
                                        start,
                                        end
                                    );
                                } else if start > end {
                                    return vm_error!(
                                        reader.ip,
                                        "Indexing with a descending range isn't supported, \
                                         start: {}, end: {}",
                                        start,
                                        end
                                    );
                                } else if ustart > list.len() || uend > list.len() {
                                    return vm_error!(
                                        reader.ip,
                                        "Index out of bounds, \
                                         List has a length of {} - start: {}, end: {}",
                                        list.len(),
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
                            IndexRange { .. } => unimplemented!(),
                            unexpected => {
                                return vm_error!(
                                    reader.ip,
                                    "Unexpected type for List index: '{}'",
                                    type_as_string(&unexpected)
                                );
                            }
                        },
                        unexpected => {
                            return vm_error!(
                                reader.ip,
                                "Expected List, found '{}'",
                                type_as_string(&unexpected),
                            )
                        }
                    };
                }
                Instruction::ListIndex {
                    register,
                    list,
                    index,
                } => {
                    let list_value = self.get_register(list).clone();
                    let index_value = self.get_register(index).clone();

                    match list_value {
                        List(l) => match index_value {
                            Number(n) => {
                                if n < 0.0 {
                                    return vm_error!(
                                        reader.ip,
                                        "Negative list indices aren't allowed (found '{}')",
                                        n
                                    );
                                }
                                self.set_register(register, l.data()[n as usize].clone());
                            }
                            Range(IntRange { start, end }) => {
                                let ustart = start as usize;
                                let uend = end as usize;

                                if start < 0 || end < 0 {
                                    return vm_error!(
                                        reader.ip,
                                        "Indexing with negative indices isn't supported, \
                                         start: {}, end: {}",
                                        start,
                                        end
                                    );
                                } else if start > end {
                                    return vm_error!(
                                        reader.ip,
                                        "Indexing with a descending range isn't supported, \
                                         start: {}, end: {}",
                                        start,
                                        end
                                    );
                                } else if ustart > l.len() || uend > l.len() {
                                    return vm_error!(
                                        reader.ip,
                                        "Index out of bounds, \
                                         List has a length of {} - start: {}, end: {}",
                                        l.len(),
                                        start,
                                        end
                                    );
                                } else {
                                    // match &value_to_set {
                                    //     Some(value) => {
                                    //         let mut list_data = list.data_mut();
                                    //         for i in ustart..uend {
                                    //             list_data[i] = value.clone();
                                    //         }
                                    //         return Ok(None);
                                    //     }
                                    //     None => {

                                    // TODO Avoid allocating new vec,
                                    // introduce 'slice' value type
                                    self.set_register(
                                        register,
                                        List(ValueList::from_slice(&l.data()[ustart..uend])),
                                    )
                                }
                            }
                            IndexRange { .. } => unimplemented!("ListIndex IndexRange"),
                            unexpected => {
                                return vm_error!(
                                    reader.ip,
                                    "Expected Number or Range, found '{}'",
                                    type_as_string(&unexpected),
                                )
                            }
                        },
                        unexpected => {
                            return vm_error!(
                                reader.ip,
                                "Expected List, found '{}'",
                                type_as_string(&unexpected),
                            )
                        }
                    };
                }
                Instruction::MapInsert {
                    register,
                    key,
                    value,
                } => {
                    let key = self.get_register(key).clone();
                    let value = self.get_register(value).clone();

                    match self.get_register_mut(register) {
                        Map(map) => match key {
                            Str(id_string) => {
                                map.data_mut().insert(Id::new(id_string), value);
                            }
                            unexpected => {
                                return vm_error!(
                                    reader.ip,
                                    "Expected String for Map key, found '{}'",
                                    type_as_string(&unexpected),
                                );
                            }
                        },
                        unexpected => {
                            return vm_error!(
                                reader.ip,
                                "Expected Map, found '{}'",
                                type_as_string(&unexpected),
                            )
                        }
                    };
                }
                Instruction::MapAccess { register, map, key } => {
                    let map_value = self.get_register(map).clone();
                    let key_value = self.get_register(key).clone();

                    match map_value {
                        Map(map) => match key_value {
                            Str(id_string) => match map.data().get(&id_string) {
                                Some(value) => {
                                    self.set_register(register, value.clone());
                                }
                                None => {
                                    return vm_error!(
                                        reader.ip,
                                        "Map entry '{}' not found",
                                        id_string,
                                    );
                                }
                            },
                            unexpected => {
                                return vm_error!(
                                    reader.ip,
                                    "Expected String for Map key, found '{}'",
                                    type_as_string(&unexpected),
                                );
                            }
                        },
                        unexpected => {
                            return vm_error!(
                                reader.ip,
                                "Expected Map, found '{}'",
                                type_as_string(&unexpected),
                            )
                        }
                    };
                }
            }
        }

        Ok(result)
    }

    fn call_function<'a>(
        &mut self,
        reader: &mut InstructionReader<'a>,
        function_register: u8,
        arg_register: u8,
        call_arg_count: u8,
        parent_register: Option<u8>,
    ) -> RuntimeResult {
        use Value::*;

        match self.get_register(function_register).clone() {
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
                        let position = reader.ip;
                        return vm_error!(position, "Expected self for external instance function");
                    }
                };

                let args = self.register_slice(arg_register, call_arg_count);
                let result = (&*function)(&mut Runtime::default(), args);
                match result {
                    Ok(value) => {
                        self.set_register(arg_register, value);
                    }
                    error @ Err(_) => {
                        return error;
                    }
                }
            }
            VmFunction {
                ip: function_ip,
                arg_count: function_arg_count,
                is_instance_function,
            } => {
                if is_instance_function {
                    if let Some(parent_register) = parent_register {
                        self.insert_register(
                            arg_register,
                            self.get_register(parent_register).clone(),
                        );
                    } else {
                        return vm_error!(reader.ip, "Expected self for function");
                    }
                }

                if function_arg_count != call_arg_count {
                    return vm_error!(
                        reader.ip,
                        "Incorrect argument count, expected {}, found {}",
                        function_arg_count,
                        call_arg_count,
                    );
                }

                self.push_frame(reader.ip, arg_register);

                reader.ip = function_ip;
            }
            unexpected => {
                return vm_error!(
                    reader.ip,
                    "Expected Function, found '{}'",
                    type_as_string(&unexpected),
                )
            }
        };

        Ok(Empty)
    }
    fn frame(&self) -> &Frame {
        self.call_stack.last().unwrap()
    }

    fn frame_mut(&mut self) -> &mut Frame {
        self.call_stack.last_mut().unwrap()
    }

    fn push_frame(&mut self, return_ip: usize, arg_register: u8) {
        let frame_base = self.register_index(arg_register);
        self.call_stack.push(Frame::new(frame_base, return_ip));
    }

    fn pop_frame(&mut self) -> RuntimeResult {
        let frame = match self.call_stack.pop() {
            Some(frame) => frame,
            None => {
                return vm_error!(0, "pop_frame: Empty call stack");
            }
        };

        let return_value = frame.result.clone();

        if !self.call_stack.is_empty() {
            self.value_stack.truncate(frame.base);
            self.value_stack.push(return_value.clone());
        }

        Ok(return_value)
    }

    fn register_index(&self, register: u8) -> usize {
        self.frame().base + register as usize
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
        &self.value_stack[self.register_index(register)]
    }

    fn get_register_mut(&mut self, register: u8) -> &mut Value {
        let index = self.register_index(register);
        &mut self.value_stack[index]
    }

    fn register_slice(&self, register: u8, count: u8) -> &[Value] {
        let start = self.register_index(register);
        &self.value_stack[start..start + count as usize]
    }

    fn get_constant_string(&self, constant_index: usize) -> &str {
        self.constants.get_string(constant_index)
    }

    fn arc_string_from_constant(&mut self, constant_index: usize) -> Arc<String> {
        let maybe_string = self.string_constants.get(&constant_index).cloned();

        match maybe_string {
            Some(s) => s,
            None => {
                let s = Arc::new(self.constants.get_string(constant_index).to_string());
                self.string_constants.insert(constant_index, s.clone());
                s
            }
        }
    }
}

fn binary_op_error(op: Instruction, lhs: &Value, rhs: &Value, ip: usize) -> RuntimeResult {
    vm_error!(
        ip,
        "Unable to perform operation {:?} with lhs: '{}' and rhs: '{}'",
        op,
        lhs,
        rhs,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{external_error, Value::*, ValueHashMap};
    use koto_bytecode::{bytecode_to_string, Compiler};
    use koto_parser::KotoParser;

    fn test_script(script: &str, expected_output: Value) {
        let mut vm = Vm::new();

        let parser = KotoParser::new();
        let mut compiler = Compiler::new();

        let ast = match parser.parse(&script, &mut vm.constants) {
            Ok(ast) => ast,
            Err(e) => panic!(format!("Error while parsing script: {}", e)),
        };
        let bytecode = match compiler.compile_ast(&ast) {
            Ok(bytecode) => bytecode,
            Err(e) => panic!(format!("Error while compiling bytecode: {}", e)),
        };

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

        eprintln!("{}", script);
        eprintln!("{}", bytecode_to_string(&bytecode));
        match vm.run(&bytecode) {
            Ok(result) => {
                if result != expected_output {
                    eprintln!("{}", script);
                    eprintln!("{}", bytecode_to_string(&bytecode));
                }
                assert_eq!(result, expected_output);
            }
            Err(e) => {
                eprintln!("{}", script);
                eprintln!("{}", bytecode_to_string(&bytecode));
                panic!(format!("Error while running script: {:?}", e));
            }
        }
    }

    fn value_list<T>(values: &[T]) -> Value
    where
        T: Copy,
        f64: From<T>,
    {
        let values = values
            .iter()
            .map(|n| Number(f64::from(*n)))
            .collect::<Vec<_>>();
        List(ValueList::from_slice(&values))
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
    }

    mod operators {
        use super::*;

        #[test]
        fn arithmetic() {
            test_script("1 + 2 * 3 + 4", Number(11.0));
        }

        #[test]
        fn assignment() {
            let script = "
a = 1 * 3
a + 1";
            test_script(script, Number(4.0));
        }

        #[test]
        fn comparison() {
            test_script("false or 1 < 2 < 3 and 3 > 2 > 1 or false", Bool(true));
        }

        #[test]
        fn equality() {
            test_script("1 + 1 == 2 and 2 + 2 != 5", Bool(true));
        }
    }

    mod control_flow {
        use super::*;

        #[test]
        fn if_else_if() {
            let script = "
if 5 < 4
  42
else if 1 < 2
  -1
else
  99";
            test_script(script, Number(-1.0));
        }
    }

    mod globals {
        use super::*;

        #[test]
        fn load_value() {
            test_script("a = test_global", Number(42.0));
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
        fn nested() {
            let script = "
add = |a b|
  add2 = |x y| x + y
  add2 a b
add 10 20";
            test_script(script, Number(30.0));
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
        fn for_loop_conditional() {
            let script = "
count = 0
(count += 1) for i in 0..10 if i > 4
count";
            test_script(script, Number(5.0));
        }

        #[test]
        fn for_loop_list() {
            let script = "
sum = 0
(sum += a) for a in [10 20 30 40]
sum";
            test_script(script, Number(100.0));
        }
    }

    mod lists {
        use super::*;

        #[test]
        fn empty() {
            test_script("[]", List(ValueList::new()));
        }

        #[test]
        fn literals() {
            test_script("[1 2 3 4]", value_list(&[1, 2, 3, 4]));
        }

        #[test]
        fn from_ids() {
            let script = "
a = 1
[a a a]";
            test_script(script, value_list(&[1, 1, 1]));
        }

        #[test]
        fn from_range() {
            test_script("[3..0]", value_list(&[3, 2, 1]));
        }

        #[test]
        fn from_multiple_ranges() {
            test_script("[0..3 3..=0]", value_list(&[0, 1, 2, 3, 2, 1, 0]));
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
a = [10 20 30]
a[1..3]";
            test_script(script, value_list(&[20, 30]));
        }

        #[test]
        fn assign_element() {
            let script = "
a = [1 2 3]
x = 2
a[x] = -1
a";
            test_script(script, value_list(&[1, 2, -1]));
        }

        #[test]
        fn assign_range() {
            let script = "
a = [1 2 3 4 5]
a[1..=3] = 0
a";
            test_script(script, value_list(&[1, 0, 0, 0, 5]));
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
            result_data.insert(Id::from_str("foo"), Number(42.0));
            result_data.insert(Id::from_str("bar"), Str(Arc::new("baz".to_string())));

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
    }
}
