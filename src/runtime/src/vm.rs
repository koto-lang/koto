#![allow(dead_code)]

use crate::{type_as_string, vm_error, Runtime, RuntimeResult, Value, ValueMap};
use koto_bytecode::{Bytecode, Op};
use koto_parser::ConstantPool;
use rustc_hash::FxHashMap;
use std::{
    convert::{TryFrom, TryInto},
    fmt,
    sync::Arc,
};

#[derive(Debug)]
enum Instruction {
    Error {
        message: String,
    },
    Copy {
        target: u8,
        source: u8,
    },
    SetEmpty {
        register: u8,
    },
    SetTrue {
        register: u8,
    },
    SetFalse {
        register: u8,
    },
    Return {
        register: u8,
    },
    LoadNumber {
        register: u8,
        constant: usize,
    },
    LoadString {
        register: u8,
        constant: usize,
    },
    LoadGlobal {
        register: u8,
        constant: usize,
    },
    MakeFunction {
        register: u8,
        arg_count: u8,
        size: usize,
    },
    Add {
        register: u8,
        lhs: u8,
        rhs: u8,
    },
    Multiply {
        register: u8,
        lhs: u8,
        rhs: u8,
    },
    Less {
        register: u8,
        lhs: u8,
        rhs: u8,
    },
    Greater {
        register: u8,
        lhs: u8,
        rhs: u8,
    },
    Equal {
        register: u8,
        lhs: u8,
        rhs: u8,
    },
    NotEqual {
        register: u8,
        lhs: u8,
        rhs: u8,
    },
    Jump {
        offset: usize,
    },
    JumpIf {
        register: u8,
        offset: usize,
        jump_condition: bool,
    },
    Call {
        register: u8,
        arg_register: u8,
        arg_count: u8,
    },
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Instruction::*;
        match self {
            Error { .. } => unreachable!(),
            Copy { target, source } => write!(f, "Copy\t\ttarget: {}\tsource: {}", target, source),
            SetEmpty { register } => write!(f, "SetEmpty\treg: {}", register),
            SetTrue { register } => write!(f, "SetTrue\treg: {}", register),
            SetFalse { register } => write!(f, "SetFalse\treg: {}", register),
            Return { register } => write!(f, "Return\t\treg: {}", register),
            LoadNumber { register, constant } => {
                write!(f, "LoadNumber\treg: {}\t\tconstant: {}", register, constant)
            }
            LoadString { register, constant } => {
                write!(f, "LoadString\treg: {}\t\tconstant: {}", register, constant)
            }
            LoadGlobal { register, constant } => {
                write!(f, "LoadGlobal\treg: {}\t\tconstant: {}", register, constant)
            }
            MakeFunction {
                register,
                arg_count,
                size,
            } => write!(
                f,
                "MakeFunction\treg: {}\t\targ_count: {}\tsize: {}",
                register, arg_count, size
            ),
            Add { register, lhs, rhs } => write!(
                f,
                "Add\t\treg: {}\t\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            Multiply { register, lhs, rhs } => write!(
                f,
                "Multiply\treg: {}\t\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            Less { register, lhs, rhs } => write!(
                f,
                "Less\t\treg: {}\t\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            Greater { register, lhs, rhs } => write!(
                f,
                "Greater\treg: {}\t\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            Equal { register, lhs, rhs } => write!(
                f,
                "Equal\t\treg: {}\t\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            NotEqual { register, lhs, rhs } => write!(
                f,
                "NotEqual\treg: {}\t\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            Jump { offset } => write!(f, "Jump\t\toffset: {}", offset),
            JumpIf {
                register,
                offset,
                jump_condition,
            } => write!(
                f,
                "JumpIf\t\treg: {}\t\toffset: {}\tcondition: {}",
                register, offset, jump_condition
            ),
            Call {
                register,
                arg_register,
                arg_count,
            } => write!(
                f,
                "Call\t\treg: {}\t\targ_reg: {}\targs: {}",
                register, arg_register, arg_count
            ),
        }
    }
}

struct InstructionReader<'a> {
    bytes: &'a [u8],
    ip: usize,
}

impl<'a> InstructionReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, ip: 0 }
    }

    fn jump(&mut self, offset: usize) {
        self.ip += offset;
    }

    fn jump_to(&mut self, ip: usize) {
        self.ip = ip
    }
}

impl<'a> Iterator for InstructionReader<'a> {
    type Item = Instruction;

    fn next(&mut self) -> Option<Self::Item> {
        use Instruction::*;

        macro_rules! get_byte {
            () => {{
                match self.bytes.get(self.ip) {
                    Some(byte) => {
                        self.ip += 1;
                        *byte
                    }
                    None => {
                        return Some(Error {
                            message: format!("Expected byte at position {}", self.ip),
                        });
                    }
                }
            }};
        }

        macro_rules! get_u16 {
            () => {{
                match self.bytes.get(self.ip..self.ip + 2) {
                    Some(u16_bytes) => {
                        self.ip += 2;
                        u16::from_le_bytes(u16_bytes.try_into().unwrap())
                    }
                    None => {
                        return Some(Error {
                            message: format!("Expected 2 bytes at position {}", self.ip),
                        });
                    }
                }
            }};
        }

        macro_rules! get_u32 {
            () => {{
                match self.bytes.get(self.ip..self.ip + 4) {
                    Some(u32_bytes) => {
                        self.ip += 4;
                        u32::from_le_bytes(u32_bytes.try_into().unwrap())
                    }
                    None => {
                        return Some(Error {
                            message: format!("Expected 4 bytes at position {}", self.ip),
                        });
                    }
                }
            }};
        }

        let byte = match self.bytes.get(self.ip) {
            Some(byte) => *byte,
            None => return None,
        };

        let op = match Op::try_from(byte) {
            Ok(op) => op,
            Err(_) => {
                return Some(Error {
                    message: format!(
                        "Unexpected opcode {} found at instruction {}",
                        byte, self.ip
                    ),
                });
            }
        };

        self.ip += 1;

        match op {
            Op::Copy => Some(Copy {
                target: get_byte!(),
                source: get_byte!(),
            }),
            Op::SetEmpty => Some(SetEmpty {
                register: get_byte!(),
            }),
            Op::SetTrue => Some(SetTrue {
                register: get_byte!(),
            }),
            Op::SetFalse => Some(SetFalse {
                register: get_byte!(),
            }),
            Op::Return => Some(Return {
                register: get_byte!(),
            }),
            Op::LoadNumber => Some(LoadNumber {
                register: get_byte!(),
                constant: get_byte!() as usize,
            }),
            Op::LoadNumberLong => Some(LoadNumber {
                register: get_byte!(),
                constant: get_u32!() as usize,
            }),
            Op::LoadString => Some(LoadString {
                register: get_byte!(),
                constant: get_byte!() as usize,
            }),
            Op::LoadStringLong => Some(LoadString {
                register: get_byte!(),
                constant: get_u32!() as usize,
            }),
            Op::LoadGlobal => Some(LoadGlobal {
                register: get_byte!(),
                constant: get_byte!() as usize,
            }),
            Op::LoadGlobalLong => Some(LoadGlobal {
                register: get_byte!(),
                constant: get_u32!() as usize,
            }),
            Op::MakeFunction => Some(MakeFunction {
                register: get_byte!(),
                arg_count: get_byte!(),
                size: get_u16!() as usize,
            }),
            Op::Add => Some(Add {
                register: get_byte!(),
                lhs: get_byte!(),
                rhs: get_byte!(),
            }),
            Op::Multiply => Some(Multiply {
                register: get_byte!(),
                lhs: get_byte!(),
                rhs: get_byte!(),
            }),
            Op::Less => Some(Less {
                register: get_byte!(),
                lhs: get_byte!(),
                rhs: get_byte!(),
            }),
            Op::Greater => Some(Greater {
                register: get_byte!(),
                lhs: get_byte!(),
                rhs: get_byte!(),
            }),
            Op::Equal => Some(Equal {
                register: get_byte!(),
                lhs: get_byte!(),
                rhs: get_byte!(),
            }),
            Op::NotEqual => Some(NotEqual {
                register: get_byte!(),
                lhs: get_byte!(),
                rhs: get_byte!(),
            }),
            Op::Jump => Some(Jump {
                offset: get_u16!() as usize,
            }),
            Op::JumpTrue => Some(JumpIf {
                register: get_byte!(),
                offset: get_u16!() as usize,
                jump_condition: true,
            }),
            Op::JumpFalse => Some(JumpIf {
                register: get_byte!(),
                offset: get_u16!() as usize,
                jump_condition: false,
            }),
            Op::Call => Some(Call {
                register: get_byte!(),
                arg_register: get_byte!(),
                arg_count: get_byte!(),
            }),
        }
    }
}

fn bytecode_to_string(bytecode: &Bytecode) -> String {
    let mut result = String::new();
    let mut reader = InstructionReader::new(bytecode);
    let mut ip = reader.ip;

    while let Some(instruction) = reader.next() {
        result += &format!("{}\t{}\n", ip, &instruction.to_string());
        ip = reader.ip;
    }

    result
}

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
                    return vm_error!(reader.ip, "{}", message);
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
                            return vm_error!(reader.ip, "'{}' not found", global_name);
                        }
                    }
                }
                MakeFunction {
                    register,
                    arg_count,
                    size,
                } => {
                    let function = VmFunction {
                        ip: reader.ip,
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
                            return binary_op_error(instruction, lhs_value, rhs_value, reader.ip);
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
                            return binary_op_error(instruction, lhs_value, rhs_value, reader.ip);
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
                            return binary_op_error(instruction, lhs_value, rhs_value, reader.ip);
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
                            return binary_op_error(instruction, lhs_value, rhs_value, reader.ip);
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
                            reader.ip,
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
                                    reader.ip,
                                    "Function expects {} arguments, found {}",
                                    function_arg_count,
                                    arg_count,
                                );
                            }

                            self.push_frame(reader.ip, arg_register);

                            reader.jump_to(function_ip);
                        }
                        unexpected => {
                            return vm_error!(
                                reader.ip,
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
    use koto_bytecode::compile::Compiler;
    use koto_parser::KotoParser;

    fn run_script(script: &str) -> Value {
        eprintln!("{}", script);

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

        eprintln!("{}", bytecode_to_string(&bytecode));

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
                return result;
            }
            Err(e) => {
                panic!(format!("Error while running script: {:?}", e));
            }
        }
    }

    #[test]
    fn basic() {
        let script = "()";
        let result = run_script(script);
        assert_eq!(result, Value::Empty);

        let script = "true";
        let result = run_script(script);
        assert_eq!(result, Value::Bool(true));

        let script = "false";
        let result = run_script(script);
        assert_eq!(result, Value::Bool(false));

        let script = "24.0";
        let result = run_script(script);
        assert_eq!(result, Value::Number(24.0));

        let script = "\"Hello\"";
        let result = run_script(script);
        assert_eq!(result, Value::Str(Arc::new("Hello".to_string())));
    }

    #[test]
    fn arithmetic() {
        let script = "1 + 2 * 3 + 4";
        let result = run_script(script);
        assert_eq!(result, Value::Number(11.0));
    }

    #[test]
    fn assignment() {
        let script = "
a = 1 * 3
a + 1
";
        let result = run_script(script);
        assert_eq!(result, Value::Number(4.0));
    }

    #[test]
    fn logic() {
        let script = "false or 1 < 2 < 3 and 3 > 2 > 1 or false";
        let result = run_script(script);
        assert_eq!(result, Value::Bool(true));

        let script = "
if 5 < 4
  42
else if 1 < 2
  -1
else
  99
";
        let result = run_script(script);
        assert_eq!(result, Value::Number(-1.0));

        let script = "1 + 1 == 2 and 2 + 2 != 5";
        let result = run_script(script);
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn global() {
        let script = "a = test_global";
        let result = run_script(script);
        assert_eq!(result, Value::Number(42.0));

        let script = "assert 1 + 1 == 2";
        let result = run_script(script);
        assert_eq!(result, Value::Empty);

        let script = "assert (1 + 1 == 2) (2 < 3)";
        let result = run_script(script);
        assert_eq!(result, Value::Empty);
    }

    #[test]
    fn functions() {
        let script = "
square = |x| x * x
square 8
";
        let result = run_script(script);
        assert_eq!(result, Value::Number(64.0));

        let script = "
add = |a b|
  a + b
add 5 6
";
        let result = run_script(script);
        assert_eq!(result, Value::Number(11.0));

        let script = "
add = |a b|
  add2 = |x y|
    x + y
  add2 a b
add 10 20
";
        let result = run_script(script);
        assert_eq!(result, Value::Number(30.0));
    }
}
