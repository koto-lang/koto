#![allow(dead_code)]

use crate::{runtime_error, type_as_string, RuntimeResult, Value, ValueMap};
use koto_bytecode::{Bytecode, Op};
use koto_parser::{AstNode, ConstantPool};
use rustc_hash::FxHashMap;
use std::{
    convert::{TryFrom, TryInto},
    fmt,
    sync::Arc,
};

#[derive(Debug)]
enum Instruction {
    Error(String),
    Move(u8, u8),
    SetEmpty(u8),
    SetTrue(u8),
    SetFalse(u8),
    Return(u8),
    LoadNumber(u8, usize),
    LoadString(u8, usize),
    LoadGlobal(u8, usize),
    Add(u8, u8, u8),
    Multiply(u8, u8, u8),
    Less(u8, u8, u8),
    Greater(u8, u8, u8),
    Equal(u8, u8, u8),
    NotEqual(u8, u8, u8),
    Jump(usize),
    JumpIf(u8, usize, bool),
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Instruction::*;
        match self {
            Error(_) => unreachable!(),
            Move(target, source) => write!(f, "Move\t\ttarget: {}\tsource: {}", target, source),
            SetEmpty(register) => write!(f, "SetEmpty\tregister: {}", register),
            SetTrue(register) => write!(f, "SetTrue\tregister: {}", register),
            SetFalse(register) => write!(f, "SetFalse\tregister: {}", register),
            Return(register) => write!(f, "Return\t\tregister: {}", register),
            LoadNumber(register, constant) => write!(
                f,
                "LoadNumber\tregister: {}\tconstant: {}",
                register, constant
            ),
            LoadString(register, constant) => write!(
                f,
                "LoadString\tregister: {}\tconstant: {}",
                register, constant
            ),
            LoadGlobal(register, constant) => write!(
                f,
                "LoadGlobal\tregister: {}\tconstant: {}",
                register, constant
            ),
            Add(register, lhs, rhs) => write!(
                f,
                "Add\tregister: {}\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            Multiply(register, lhs, rhs) => write!(
                f,
                "Multiply\tregister: {}\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            Less(register, lhs, rhs) => write!(
                f,
                "Less\t\tregister: {}\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            Greater(register, lhs, rhs) => write!(
                f,
                "Greater\tregister: {}\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            Equal(register, lhs, rhs) => write!(
                f,
                "Equal\tregister: {}\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            NotEqual(register, lhs, rhs) => write!(
                f,
                "NotEqual\tregister: {}\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            Jump(offset) => write!(f, "Jump\t\toffset: {}", offset),
            JumpIf(register, offset, jump_condition) => write!(
                f,
                "JumpIf\t\tregister: {}\toffset: {}\tcondition: {}",
                register, offset, jump_condition
            ),
        }
    }
}

struct InstructionReader<'a> {
    bytes: &'a [u8],
    ip: usize,
}

impl<'a> InstructionReader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, ip: 0 }
    }

    pub fn jump(&mut self, offset: usize) {
        self.ip += offset;
    }

    pub fn position(&self) -> usize {
        self.ip
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
                        return Some(Error(format!("Expected byte at position {}", self.ip)));
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
                        return Some(Error(format!("Expected 2 bytes at position {}", self.ip)));
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
                        return Some(Error(format!("Expected 4 bytes at position {}", self.ip)));
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
                return Some(Error(format!(
                    "Unexpected opcode {} found at instruction {}",
                    byte, self.ip
                )));
            }
        };

        self.ip += 1;

        match op {
            Op::Move => Some(Move(get_byte!(), get_byte!())),
            Op::SetEmpty => Some(SetEmpty(get_byte!())),
            Op::SetTrue => Some(SetTrue(get_byte!())),
            Op::SetFalse => Some(SetFalse(get_byte!())),
            Op::Return => Some(Return(get_byte!())),
            Op::LoadNumber => Some(LoadNumber(get_byte!(), get_byte!() as usize)),
            Op::LoadNumberLong => Some(LoadNumber(get_byte!(), get_u32!() as usize)),
            Op::LoadString => Some(LoadString(get_byte!(), get_byte!() as usize)),
            Op::LoadStringLong => Some(LoadString(get_byte!(), get_u32!() as usize)),
            Op::LoadGlobal => Some(LoadGlobal(get_byte!(), get_byte!() as usize)),
            Op::LoadGlobalLong => Some(LoadGlobal(get_byte!(), get_u32!() as usize)),
            Op::Add => Some(Add(get_byte!(), get_byte!(), get_byte!())),
            Op::Multiply => Some(Multiply(get_byte!(), get_byte!(), get_byte!())),
            Op::Less => Some(Less(get_byte!(), get_byte!(), get_byte!())),
            Op::Greater => Some(Greater(get_byte!(), get_byte!(), get_byte!())),
            Op::Equal => Some(Equal(get_byte!(), get_byte!(), get_byte!())),
            Op::NotEqual => Some(NotEqual(get_byte!(), get_byte!(), get_byte!())),
            Op::Jump => Some(Jump(get_u16!() as usize)),
            Op::JumpTrue => Some(JumpIf(get_byte!(), get_u16!() as usize, true)),
            Op::JumpFalse => Some(JumpIf(get_byte!(), get_u16!() as usize, false)),
        }
    }
}

fn bytecode_to_string(bytecode: &Bytecode) -> String {
    let mut result = String::new();
    let mut instruction_reader = InstructionReader::new(bytecode);
    let mut position = instruction_reader.position();
    while let Some(instruction) = instruction_reader.next() {
        result += &format!("{}\t{}\n", position, &instruction.to_string());
        position = instruction_reader.position();
    }
    result
}

#[derive(Default)]
pub struct Vm {
    global: ValueMap,
    constants: ConstantPool,
    string_constants: FxHashMap<usize, Arc<String>>,
    stack: Vec<Value>,
    base: usize,
    result: Value,
}

impl Vm {
    pub fn new() -> Self {
        Self {
            stack: Vec::with_capacity(32),
            ..Default::default()
        }
    }

    pub fn run(&mut self, bytecode: &Bytecode) -> RuntimeResult {
        use {Instruction::*, Value::*};

        self.base = 0;
        self.stack.resize(64, Value::Empty);

        let mut byte_reader = InstructionReader::new(bytecode);

        while let Some(instruction) = byte_reader.next() {
            match instruction {
                Error(error) => {
                    return runtime_error!(AstNode::default(), "{}", error);
                }
                Move(target, source_register) => {
                    let source = self.load_register(source_register);
                    self.set_register(target, source);
                }
                SetEmpty(register) => self.set_register(register, Empty),
                SetTrue(register) => self.set_register(register, Bool(true)),
                SetFalse(register) => self.set_register(register, Bool(false)),
                Return(register) => self.result = self.load_register(register),
                LoadNumber(register, constant) => {
                    self.set_register(register, Number(self.constants.get_f64(constant as usize)))
                }
                LoadString(register, constant) => {
                    let string = self.arc_string_from_constant(constant);
                    self.set_register(register, Str(string))
                }
                LoadGlobal(register, constant) => {
                    let global_name = self.get_constant_string(constant as usize);
                    let global = self.global.data().get(global_name).cloned();
                    match global {
                        Some(value) => self.set_register(register, value),
                        None => {
                            return runtime_error!(
                                AstNode::default(),
                                "'{}' not found",
                                global_name
                            );
                        }
                    }
                }
                Add(result_register, lhs_register, rhs_register) => {
                    let lhs = self.load_register(lhs_register);
                    let rhs = self.load_register(rhs_register);
                    let result = match (&lhs, &rhs) {
                        (Number(a), Number(b)) => Number(a + b),
                        _ => {
                            return binary_op_error(instruction, lhs, rhs);
                        }
                    };
                    self.set_register(result_register, result);
                }
                Multiply(result_register, lhs_register, rhs_register) => {
                    let lhs = self.load_register(lhs_register);
                    let rhs = self.load_register(rhs_register);
                    let result = match (&lhs, &rhs) {
                        (Number(a), Number(b)) => Number(a * b),
                        _ => {
                            return binary_op_error(instruction, lhs, rhs);
                        }
                    };
                    self.set_register(result_register, result);
                }
                Less(result_register, lhs_register, rhs_register) => {
                    let lhs = self.load_register(lhs_register);
                    let rhs = self.load_register(rhs_register);
                    let result = match (&lhs, &rhs) {
                        (Number(a), Number(b)) => Bool(a < b),
                        _ => {
                            return binary_op_error(instruction, lhs, rhs);
                        }
                    };
                    self.set_register(result_register, result);
                }
                Greater(result_register, lhs_register, rhs_register) => {
                    let lhs = self.load_register(lhs_register);
                    let rhs = self.load_register(rhs_register);
                    let result = match (&lhs, &rhs) {
                        (Number(a), Number(b)) => Bool(a > b),
                        _ => {
                            return binary_op_error(instruction, lhs, rhs);
                        }
                    };
                    self.set_register(result_register, result);
                }
                Equal(result_register, lhs_register, rhs_register) => {
                    let lhs = self.load_register(lhs_register);
                    let rhs = self.load_register(rhs_register);
                    let result = (lhs == rhs).into();
                    self.set_register(result_register, result);
                }
                NotEqual(result_register, lhs_register, rhs_register) => {
                    let lhs = self.load_register(lhs_register);
                    let rhs = self.load_register(rhs_register);
                    let result = (lhs != rhs).into();
                    self.set_register(result_register, result);
                }
                Jump(offset) => {
                    byte_reader.jump(offset);
                }
                JumpIf(register, offset, jump_condition) => match self.load_register(register) {
                    Bool(b) => {
                        if b == jump_condition {
                            byte_reader.jump(offset);
                        }
                    }
                    unexpected => {
                        return runtime_error!(
                            AstNode::default(),
                            "Expected Bool, found '{}'",
                            type_as_string(&unexpected)
                        );
                    }
                },
            }
        }

        Ok(self.result.clone())
    }

    fn set_register(&mut self, index: u8, value: Value) {
        self.result = value.clone(); //temp
        self.stack[self.base + index as usize] = value;
    }

    fn load_register(&self, index: u8) -> Value {
        self.stack[self.base + index as usize].clone()
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

fn binary_op_error(op: Instruction, lhs: Value, rhs: Value) -> RuntimeResult {
    runtime_error!(
        AstNode::default(), // TODO
        "Unable to perform operation {:?} with lhs: '{}' and rhs: '{}'",
        op,
        lhs,
        rhs
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use koto_bytecode::compile::Compiler;
    use koto_parser::KotoParser;

    fn run_script(script: &str) -> Value {
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

        match vm.run(&bytecode) {
            Ok(result) => {
                return result;
            }
            Err(e) => {
                eprintln!("{}", script);
                eprintln!("{}", bytecode_to_string(&bytecode));
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
    }

    // #[test]
    // fn functions() {
    //     let script = "assert 1 + 1 == 2";
    //     let result = run_script(script);
    //     assert_eq!(result, Value::Empty);
    // }
}
