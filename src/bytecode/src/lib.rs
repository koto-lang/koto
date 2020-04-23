use num_enum::{IntoPrimitive, TryFromPrimitive};

mod compile;
mod instruction_reader;

pub use compile::*;
pub use instruction_reader::*;

pub type Bytecode = Vec<u8>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum Op {
    Copy,           // register, source
    SetEmpty,       // register
    SetTrue,        // register
    SetFalse,       // register
    Return,         // register
    LoadNumber,     // register, constant
    LoadNumberLong, // register, constant[4]
    LoadString,     // register, constant
    LoadStringLong, // register, constant[4]
    LoadGlobal,     // register, constant
    LoadGlobalLong, // register, constant[4]
    MakeFunction,   // register, arg count, size[2]
    Add,            // register, lhs, rhs
    Multiply,       // register, lhs, rhs
    Less,           // register, lhs, rhs
    Greater,        // register, lhs, rhs
    Equal,          // register, lhs, rhs
    NotEqual,       // register, lhs, rhs
    Jump,           // offset[2]
    JumpTrue,       // register, offset[2]
    JumpFalse,      // register, offset[2]
    Call,           // function register, arg register, arg count
}

pub fn bytecode_to_string(bytecode: &Bytecode) -> String {
    let mut result = String::new();
    let mut reader = InstructionReader::new(bytecode);
    let mut ip = reader.position();

    while let Some(instruction) = reader.next() {
        result += &format!("{}\t{}\n", ip, &instruction.to_string());
        ip = reader.position();
    }

    result
}
