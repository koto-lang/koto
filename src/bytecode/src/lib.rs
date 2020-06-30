use {
    koto_parser::{ConstantPool, Span},
    num_enum::{IntoPrimitive, TryFromPrimitive},
    std::{path::PathBuf, sync::Arc},
};

mod compile;
mod instruction_reader;

pub use compile::*;
pub use instruction_reader::*;

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
    Import,           // register, constant
    ImportLong,       // register, constant[4]
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
    In,               // result, lhs, rhs
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
    Size,             // register
    Type,             // register
    Debug,            // register, constant[4]
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DebugInfo {
    source_map: Vec<(usize, Span)>,
    pub source: String,
}

impl DebugInfo {
    fn push(&mut self, ip: usize, span: &Span) {
        if let Some(entry) = self.source_map.last() {
            if entry.1 == *span {
                // Don't add entries with matching spans, a search is performed in
                // get_source_span which will find the correct span
                // for intermediate ips.
                return;
            }
        }
        self.source_map.push((ip, *span));
    }

    pub fn get_source_span(&self, ip: usize) -> Option<Span> {
        // Find the last entry with an ip less than or equal to the input
        // an upper_bound would nice here, but this isn't currently a performance sensitive function
        // so a scan through the entries will do.
        let mut result = None;
        for entry in self.source_map.iter() {
            if entry.0 <= ip {
                result = Some(entry.1);
            } else {
                break;
            }
        }
        result
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Chunk {
    pub bytes: Vec<u8>,
    pub constants: ConstantPool,
    pub source_path: Option<PathBuf>,
    pub debug_info: DebugInfo,
}

impl Chunk {
    pub fn new(
        bytes: Vec<u8>,
        constants: ConstantPool,
        source_path: Option<PathBuf>,
        debug_info: DebugInfo,
    ) -> Self {
        Self {
            bytes,
            constants,
            source_path,
            debug_info,
        }
    }
}

pub fn chunk_to_string(chunk: Arc<Chunk>) -> String {
    let mut result = String::new();
    let mut reader = InstructionReader::new(chunk);
    let mut ip = reader.ip;

    while let Some(instruction) = reader.next() {
        result += &format!("{}\t{}\n", ip, &instruction.to_string());
        ip = reader.ip;
    }

    result
}

pub fn chunk_to_string_annotated(chunk: Arc<Chunk>, source_lines: &[&str]) -> String {
    let mut result = String::new();
    let mut reader = InstructionReader::new(chunk);
    let mut ip = reader.ip;
    let mut span: Option<Span> = None;
    let mut first = true;

    while let Some(instruction) = reader.next() {
        let instruction_span = reader
            .chunk
            .debug_info
            .get_source_span(ip)
            .expect("Missing source span");

        let print_source_lines = if let Some(span) = span {
            instruction_span.start.line != span.start.line
        } else {
            true
        };

        if print_source_lines && !source_lines.is_empty() {
            if !first {
                result += "\n";
            }
            first = false;

            let line = instruction_span
                .start
                .line
                .min(source_lines.len() as u32)
                .max(1) as usize;
            result += &format!("|{}| {}\n", line.to_string(), source_lines[line - 1]);
            span = Some(instruction_span);
        }

        result += &format!("{}\t{}\n", ip, &instruction.to_string());
        ip = reader.ip;
    }

    result
}
