use {
    koto_parser::Span,
    num_enum::{IntoPrimitive, TryFromPrimitive},
};

mod compile;
mod instruction_reader;

pub use compile::*;
pub use instruction_reader::*;

pub type Bytecode = Vec<u8>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum Op {
    Copy,             // target, source
    DeepCopy,         // target, source
    SetEmpty,         // register
    SetFalse,         // register
    SetTrue,          // register
    Set0,             // register
    Set1,             // register
    LoadNumber,       // register, constant
    LoadNumberLong,   // register, constant[4]
    LoadString,       // register, constant
    LoadStringLong,   // register, constant[4]
    LoadGlobal,       // register, constant
    LoadGlobalLong,   // register, constant[4]
    SetGlobal,        // global, source
    SetGlobalLong,    // global[4], source
    MakeList,         // register, size hint
    MakeListLong,     // register, size hint[4]
    MakeMap,          // register, size hint
    MakeMapLong,      // register, size hint[4]
    MakeNum2,         // register, element count, first element
    MakeNum4,         // register, element count, first element
    MakeIterator,     // register, range
    Function,         // register, arg count, capture count, size[2]
    InstanceFunction, // register, arg count, capture count, size[2]
    Capture,          // function, target, source
    LoadCapture,      // register, capture
    SetCapture,       // capture, source
    Range,            // register, start, end
    RangeInclusive,   // register, start, end
    RangeTo,          // register, end
    RangeToInclusive, // register, end
    RangeFrom,        // register, start
    RangeFull,        // register
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
    Call,             // result, function, arg register, arg count
    CallChild,        // result, function, arg register, arg count, parent
    Return,           // register
    IteratorNext,     // output, iterator, jump offset[2]
    ExpressionIndex,  // register, multi_expression, index
    ListPush,         // list, value
    ListUpdate,       // list, index, value
    ListIndex,        // register, list, index
    MapInsert,        // map, key, value
    MapAccess,        // register, map, key
    Debug,            // register, constant[4]
}

pub fn bytecode_to_string(bytecode: &[u8]) -> String {
    let mut result = String::new();
    let mut reader = InstructionReader::new(bytecode);
    let mut ip = reader.ip;

    while let Some(instruction) = reader.next() {
        result += &format!("{}\t{}\n", ip, &instruction.to_string());
        ip = reader.ip;
    }

    result
}

pub fn bytecode_to_string_annotated(
    bytecode: &[u8],
    script_lines: &[&str],
    debug_info: &DebugInfo,
    start_position: Option<usize>,
) -> String {
    let mut result = String::new();
    let mut reader = InstructionReader::new(bytecode);
    reader.ip = start_position.unwrap_or(0);
    let mut ip = reader.ip;
    let mut span: Option<Span> = None;
    let mut first = true;

    while let Some(instruction) = reader.next() {
        let instruction_span = debug_info.get_source_span(ip).expect("Missing source span");

        let print_source_lines = if let Some(span) = span {
            instruction_span.start.line != span.start.line
        } else {
            true
        };

        if print_source_lines && !script_lines.is_empty() {
            if !first {
                result += "\n";
            }
            first = false;

            let line = instruction_span
                .start
                .line
                .min(script_lines.len() as u32)
                .max(1) as usize;
            result += &format!("|{}| {}\n", line.to_string(), script_lines[line - 1]);
            span = Some(instruction_span);
        }

        result += &format!("{}\t{}\n", ip, &instruction.to_string());
        ip = reader.ip;
    }

    result
}
