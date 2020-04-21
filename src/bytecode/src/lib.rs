use num_enum::{IntoPrimitive, TryFromPrimitive};

pub mod compile;

pub type Bytecode = Vec<u8>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum Op {
    Move,           // target, source
    SetEmpty,       // target
    SetTrue,        // target
    SetFalse,       // target
    Return,         // target
    LoadNumber,     // target, constant
    LoadNumberLong, // target, constant[4]
    LoadString,     // target, constant
    LoadStringLong, // target, constant[4]
    Add,            // target, lhs, rhs
    Multiply,       // target, lhs, rhs
    Less,           // target, lhs, rhs
    Greater,        // target, lhs, rhs
    Jump,           // offset[2]
    JumpTrue,       // register, offset[2]
    JumpFalse,      // register, offset[2]
}
