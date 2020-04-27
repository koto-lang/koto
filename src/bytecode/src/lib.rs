use num_enum::{IntoPrimitive, TryFromPrimitive};

mod compile;
mod instruction_reader;

pub use compile::*;
pub use instruction_reader::*;

pub type Bytecode = Vec<u8>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum Op {
    Copy,             // target, source
    SetEmpty,         // register
    SetTrue,          // register
    SetFalse,         // register
    Return,           // register
    LoadNumber,       // register, constant
    LoadNumberLong,   // register, constant[4]
    LoadString,       // register, constant
    LoadStringLong,   // register, constant[4]
    LoadGlobal,       // register, constant
    LoadGlobalLong,   // register, constant[4]
    MakeList,         // register, size hint
    MakeListLong,     // register, size hint[4]
    MakeMap,          // register, size hint
    MakeMapLong,      // register, size hint[4]
    MakeIterator,     // register, range
    Function,         // register, arg count, size[2]
    InstanceFunction, // register, arg count, size[2]
    RangeExclusive,   // register, start, end
    RangeInclusive,   // register, start, end
    Negate,           // register, source
    Add,              // result, lhs, rhs
    Subtract,         // result, lhs, rhs
    Multiply,         // result, lhs, rhs
    Divide,           // result, lhs, rhs
    Modulo,           // result, lhs, rhs
    Less,             // result, lhs, rhs
    LessOrEqual,      // result, lhs, rhs
    Greater,          // result, lhs, rhs
    GreaterOrEqual,   // result, lhs, rhs
    Equal,            // result, lhs, rhs
    NotEqual,         // result, lhs, rhs
    Jump,             // offset[2]
    JumpTrue,         // condition, offset[2]
    JumpFalse,        // condition, offset[2]
    JumpBack,         // offset[2]
    JumpBackFalse,    // offset[2]
    Call,             // function, arg, arg count
    CallChild,        // function, parent, arg, arg count
    IteratorNext,     // output, iterator, jump offset[2]
    ExpressionIndex,  // register, multi_expression, index
    ListPush,         // list, value
    ListUpdate,       // list, index, value
    ListIndex,        // register, list, index
    MapInsert,        // map, key, value
    MapAccess,        // register, map, key
}

pub fn bytecode_to_string(bytecode: &Bytecode) -> String {
    let mut result = String::new();
    let mut reader = InstructionReader::new(bytecode);
    let mut ip = reader.ip;

    while let Some(instruction) = reader.next() {
        result += &format!("{}\t{}\n", ip, &instruction.to_string());
        ip = reader.ip;
    }

    result
}
