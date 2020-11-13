use {
    koto_parser::{ConstantPool, Span},
    std::{path::PathBuf, sync::Arc},
};

mod compile;
mod instruction_reader;

pub use compile::*;
pub use instruction_reader::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Op {
    Copy,             // target, source
    DeepCopy,         // target, source
    SetEmpty,         // register
    SetFalse,         // register
    SetTrue,          // register
    Set0,             // register
    Set1,             // register
    SetNumberU8,      // register, number
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
    MakeTuple,        // register, start register, count
    MakeTempTuple,    // register, start register, count
    MakeList,         // register, size hint
    MakeListLong,     // register, size hint[4]
    MakeMap,          // register, size hint
    MakeMapLong,      // register, size hint[4]
    MakeNum2,         // register, element count, first element
    MakeNum4,         // register, element count, first element
    MakeIterator,     // register, range
    Function,         // register, arg count, capture count, flags, size[2]
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
    Yield,            // register
    IterNext,         // output, iterator, jump offset[2]
    IterNextTemp,     // output, iterator, jump offset[2]
    IterNextQuiet,    // iterator, jump offset[2]
    ValueIndex,       // result, value register, signed index
    SliceFrom,        // result, value register, signed index
    SliceTo,          // result, value register, signed index
    ListPushValue,    // list, value
    ListPushValues,   // list, start register, count
    ListUpdate,       // list, index, value
    Index,            // TODO rename to ListIndex - result, list register, index register
    MapInsert,        // map register, value register, key constant
    MapInsertLong,    // map register, value register, key constant[4]
    MapAccess,        // register, map register, key
    MapAccessLong,    // register, map register, key[4]
    Type,             // register
    IsList,           // register, value
    IsTuple,          // register, value
    Size,             // register, value
    TryStart,         // catch arg register, catch body offset[2]
    TryEnd,           //
    Debug,            // register, constant[4]
    Unused79,
    Unused80,
    Unused81,
    Unused82,
    Unused83,
    Unused84,
    Unused85,
    Unused86,
    Unused87,
    Unused88,
    Unused89,
    Unused90,
    Unused91,
    Unused92,
    Unused93,
    Unused94,
    Unused95,
    Unused96,
    Unused97,
    Unused98,
    Unused99,
    Unused100,
    Unused101,
    Unused102,
    Unused103,
    Unused104,
    Unused105,
    Unused106,
    Unused107,
    Unused108,
    Unused109,
    Unused110,
    Unused111,
    Unused112,
    Unused113,
    Unused114,
    Unused115,
    Unused116,
    Unused117,
    Unused118,
    Unused119,
    Unused120,
    Unused121,
    Unused122,
    Unused123,
    Unused124,
    Unused125,
    Unused126,
    Unused127,
    Unused128,
    Unused129,
    Unused130,
    Unused131,
    Unused132,
    Unused133,
    Unused134,
    Unused135,
    Unused136,
    Unused137,
    Unused138,
    Unused139,
    Unused140,
    Unused141,
    Unused142,
    Unused143,
    Unused144,
    Unused145,
    Unused146,
    Unused147,
    Unused148,
    Unused149,
    Unused150,
    Unused151,
    Unused152,
    Unused153,
    Unused154,
    Unused155,
    Unused156,
    Unused157,
    Unused158,
    Unused159,
    Unused160,
    Unused161,
    Unused162,
    Unused163,
    Unused164,
    Unused165,
    Unused166,
    Unused167,
    Unused168,
    Unused169,
    Unused170,
    Unused171,
    Unused172,
    Unused173,
    Unused174,
    Unused175,
    Unused176,
    Unused177,
    Unused178,
    Unused179,
    Unused180,
    Unused181,
    Unused182,
    Unused183,
    Unused184,
    Unused185,
    Unused186,
    Unused187,
    Unused188,
    Unused189,
    Unused190,
    Unused191,
    Unused192,
    Unused193,
    Unused194,
    Unused195,
    Unused196,
    Unused197,
    Unused198,
    Unused199,
    Unused200,
    Unused201,
    Unused202,
    Unused203,
    Unused204,
    Unused205,
    Unused206,
    Unused207,
    Unused208,
    Unused209,
    Unused210,
    Unused211,
    Unused212,
    Unused213,
    Unused214,
    Unused215,
    Unused216,
    Unused217,
    Unused218,
    Unused219,
    Unused220,
    Unused221,
    Unused222,
    Unused223,
    Unused224,
    Unused225,
    Unused226,
    Unused227,
    Unused228,
    Unused229,
    Unused230,
    Unused231,
    Unused232,
    Unused233,
    Unused234,
    Unused235,
    Unused236,
    Unused237,
    Unused238,
    Unused239,
    Unused240,
    Unused241,
    Unused242,
    Unused243,
    Unused244,
    Unused245,
    Unused246,
    Unused247,
    Unused248,
    Unused249,
    Unused250,
    Unused251,
    Unused252,
    Unused253,
    Unused254,
    Unused255,
}

impl From<u8> for Op {
    fn from(op: u8) -> Op {
        // Safety:
        //  - Op is repr(u8)
        //  - All 256 possible values are represented in the enum
        unsafe { std::mem::transmute(op) }
    }
}

struct FunctionFlags {
    instance_function: bool,
    variadic: bool,
    generator: bool,
}

impl FunctionFlags {
    pub const INSTANCE: u8 = 0b0000001;
    pub const VARIADIC: u8 = 0b0000010;
    pub const GENERATOR: u8 = 0b0000100;

    pub fn from_byte(byte: u8) -> Self {
        Self {
            instance_function: byte & Self::INSTANCE == Self::INSTANCE,
            variadic: byte & Self::VARIADIC == Self::VARIADIC,
            generator: byte & Self::GENERATOR == Self::GENERATOR,
        }
    }

    pub fn as_byte(&self) -> u8 {
        let mut result = 0;
        if self.instance_function {
            result |= Self::INSTANCE;
        }
        if self.variadic {
            result |= Self::VARIADIC;
        }
        if self.generator {
            result |= Self::GENERATOR;
        }
        result
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DebugInfo {
    source_map: Vec<(usize, Span)>,
    pub source: String,
}

impl DebugInfo {
    fn push(&mut self, ip: usize, span: Span) {
        if let Some(entry) = self.source_map.last() {
            if entry.1 == span {
                // Don't add entries with matching spans, a search is performed in
                // get_source_span which will find the correct span
                // for intermediate ips.
                return;
            }
        }
        self.source_map.push((ip, span));
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

#[derive(Clone, Debug, PartialEq)]
pub struct Chunk {
    pub bytes: Vec<u8>,
    pub constants: ConstantPool,
    pub string_constants_arc: Arc<str>,
    pub source_path: Option<PathBuf>,
    pub debug_info: DebugInfo,
}

impl Default for Chunk {
    fn default() -> Self {
        Self {
            bytes: vec![],
            constants: ConstantPool::default(),
            string_constants_arc: String::default().into(),
            source_path: None,
            debug_info: DebugInfo::default(),
        }
    }
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
            string_constants_arc: constants.string_data().into(),
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

        result += &format!("{}\t{:?}\n", ip, &instruction);
        ip = reader.ip;
    }

    result
}
