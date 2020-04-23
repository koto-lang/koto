#![allow(dead_code)]

use crate::{type_as_string, vm_error, Runtime, RuntimeResult, Value, ValueMap};
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
        use {Instruction::*, Value::*};

        self.value_stack.clear();
        self.call_stack.clear();
        self.call_stack.push(Frame::default());
        let mut result = Empty;

        let mut reader = InstructionReader::new(bytecode);

        while let Some(instruction) = reader.next() {
            match instruction {
                Error { message } => {
                    return vm_error!(reader.position(), "{}", message);
                }
                Copy { target, source } => {
                    let source_value = self.load_register(source);
                    self.set_register(target, source_value);
                }
                SetEmpty { register } => self.set_register(register, Empty),
                SetTrue { register } => self.set_register(register, Bool(true)),
                SetFalse { register } => self.set_register(register, Bool(false)),
                Return { register } => {
                    self.frame_mut().result = self.load_register(register);

                    let return_ip = self.frame().return_ip;
                    result = self.pop_frame()?;

                    if self.call_stack.is_empty() {
                        break;
                    } else {
                        reader.jump_to(return_ip);
                    }
                }
                LoadNumber { register, constant } => {
                    self.set_register(register, Number(self.constants.get_f64(constant as usize)))
                }
                LoadString { register, constant } => {
                    let string = self.arc_string_from_constant(constant);
                    self.set_register(register, Str(string))
                }
                LoadGlobal { register, constant } => {
                    let global_name = self.get_constant_string(constant as usize);
                    let global = self.global.data().get(global_name).cloned();
                    match global {
                        Some(value) => self.set_register(register, value),
                        None => {
                            return vm_error!(reader.position(), "'{}' not found", global_name);
                        }
                    }
                }
                MakeFunction {
                    register,
                    arg_count,
                    size,
                } => {
                    let function = VmFunction {
                        ip: reader.position(),
                        arg_count,
                    };
                    reader.jump(size);
                    self.set_register(register, function);
                }
                Add { register, lhs, rhs } => {
                    let lhs_value = self.load_register(lhs);
                    let rhs_value = self.load_register(rhs);
                    let result = match (&lhs_value, &rhs_value) {
                        (Number(a), Number(b)) => Number(a + b),
                        _ => {
                            return binary_op_error(
                                instruction,
                                lhs_value,
                                rhs_value,
                                reader.position(),
                            );
                        }
                    };
                    self.set_register(register, result);
                }
                Multiply { register, lhs, rhs } => {
                    let lhs_value = self.load_register(lhs);
                    let rhs_value = self.load_register(rhs);
                    let result = match (&lhs_value, &rhs_value) {
                        (Number(a), Number(b)) => Number(a * b),
                        _ => {
                            return binary_op_error(
                                instruction,
                                lhs_value,
                                rhs_value,
                                reader.position(),
                            );
                        }
                    };
                    self.set_register(register, result);
                }
                Less { register, lhs, rhs } => {
                    let lhs_value = self.load_register(lhs);
                    let rhs_value = self.load_register(rhs);
                    let result = match (&lhs_value, &rhs_value) {
                        (Number(a), Number(b)) => Bool(a < b),
                        _ => {
                            return binary_op_error(
                                instruction,
                                lhs_value,
                                rhs_value,
                                reader.position(),
                            );
                        }
                    };
                    self.set_register(register, result);
                }
                Greater { register, lhs, rhs } => {
                    let lhs_value = self.load_register(lhs);
                    let rhs_value = self.load_register(rhs);
                    let result = match (&lhs_value, &rhs_value) {
                        (Number(a), Number(b)) => Bool(a > b),
                        _ => {
                            return binary_op_error(
                                instruction,
                                lhs_value,
                                rhs_value,
                                reader.position(),
                            );
                        }
                    };
                    self.set_register(register, result);
                }
                Equal { register, lhs, rhs } => {
                    let lhs_value = self.load_register(lhs);
                    let rhs_value = self.load_register(rhs);
                    let result = (lhs_value == rhs_value).into();
                    self.set_register(register, result);
                }
                NotEqual { register, lhs, rhs } => {
                    let lhs_value = self.load_register(lhs);
                    let rhs_value = self.load_register(rhs);
                    let result = (lhs_value != rhs_value).into();
                    self.set_register(register, result);
                }
                Jump { offset } => {
                    reader.jump(offset);
                }
                JumpIf {
                    register,
                    offset,
                    jump_condition,
                } => match self.load_register(register) {
                    Bool(b) => {
                        if b == jump_condition {
                            reader.jump(offset);
                        }
                    }
                    unexpected => {
                        return vm_error!(
                            reader.position(),
                            "Expected Bool, found '{}'",
                            type_as_string(&unexpected),
                        );
                    }
                },
                Call {
                    register,
                    arg_register,
                    arg_count,
                } => {
                    let function = self.load_register(register);
                    match function {
                        ExternalFunction(f) => {
                            let function = f.function.as_ref();
                            let args = self.register_slice(arg_register, arg_count);
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
                        } => {
                            if function_arg_count != arg_count {
                                return vm_error!(
                                    reader.position(),
                                    "Function expects {} arguments, found {}",
                                    function_arg_count,
                                    arg_count,
                                );
                            }

                            self.push_frame(reader.position(), arg_register);

                            reader.jump_to(function_ip);
                        }
                        unexpected => {
                            return vm_error!(
                                reader.position(),
                                "Expected Function, found '{}'",
                                type_as_string(&unexpected),
                            )
                        }
                    }
                }
            }
        }

        Ok(result)
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

    fn load_register(&self, register: u8) -> Value {
        self.value_stack[self.register_index(register)].clone()
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

fn binary_op_error(op: Instruction, lhs: Value, rhs: Value, ip: usize) -> RuntimeResult {
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
    use crate::external_error;
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

        vm.global.add_value("test_global", Value::Number(42.0));
        vm.global.add_fn("assert", |_, args| {
            use Value::*;
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

    mod literals {
        use super::*;

        #[test]
        fn empty() {
            test_script("()", Value::Empty);
        }

        #[test]
        fn bool_true() {
            test_script("true", Value::Bool(true));
        }

        #[test]
        fn bool_false() {
            test_script("false", Value::Bool(false));
        }

        #[test]
        fn number() {
            test_script("24.0", Value::Number(24.0));
        }

        #[test]
        fn string() {
            test_script("\"Hello\"", Value::Str(Arc::new("Hello".to_string())));
        }
    }

    mod operators {
        use super::*;

        #[test]
        fn arithmetic() {
            test_script("1 + 2 * 3 + 4", Value::Number(11.0));
        }

        #[test]
        fn assignment() {
            let script = "
a = 1 * 3
a + 1";
            test_script(script, Value::Number(4.0));
        }

        #[test]
        fn comparison() {
            test_script(
                "false or 1 < 2 < 3 and 3 > 2 > 1 or false",
                Value::Bool(true),
            );
        }

        #[test]
        fn equality() {
            test_script("1 + 1 == 2 and 2 + 2 != 5", Value::Bool(true));
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
            test_script(script, Value::Number(-1.0));
        }
    }

    mod globals {
        use super::*;

        #[test]
        fn load_value() {
            test_script("a = test_global", Value::Number(42.0));
        }

        #[test]
        fn function() {
            test_script("assert 1 + 1 == 2", Value::Empty);
        }

        #[test]
        fn function_two_args() {
            test_script("assert (1 + 1 == 2) (2 < 3)", Value::Empty);
        }
    }

    mod functions {
        use super::*;

        #[test]
        fn single_arg() {
            let script = "
square = |x| x * x
square 8";
            test_script(script, Value::Number(64.0));
        }

        #[test]
        fn two_args() {
            let script = "
add = |a b|
  a + b
add 5 6";
            test_script(script, Value::Number(11.0));
        }

        #[test]
        fn nested() {
            let script = "
add = |a b|
  add2 = |x y| x + y
  add2 a b
add 10 20";
            test_script(script, Value::Number(30.0));
        }
    }
}
