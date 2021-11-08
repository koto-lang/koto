use {
    crate::{Chunk, Op},
    koto_parser::{ConstantIndex, MetaKeyId},
    std::{convert::TryInto, fmt, sync::Arc},
};

#[derive(Debug)]
#[repr(u8)]
#[allow(missing_docs)]
pub enum TypeId {
    List,
    Tuple,
}

impl TypeId {
    fn from_byte(byte: u8) -> Result<Self, u8> {
        if byte == Self::List as u8 {
            Ok(Self::List)
        } else if byte == Self::Tuple as u8 {
            Ok(Self::Tuple)
        } else {
            Err(byte)
        }
    }
}

/// Flags used to define the properties of a Function
pub struct FunctionFlags {
    /// True if the function is an instance function
    pub instance_function: bool,
    /// True if the function has a variadic argument
    pub variadic: bool,
    /// True if the function is a generator
    pub generator: bool,
}

impl FunctionFlags {
    /// Corresponding to [FunctionFlags::instance_function]
    pub const INSTANCE: u8 = 1 << 0;
    /// Corresponding to [FunctionFlags::variadic]
    pub const VARIADIC: u8 = 1 << 1;
    /// Corresponding to [FunctionFlags::generator]
    pub const GENERATOR: u8 = 1 << 2;

    /// Initializes a flags struct from a byte
    pub fn from_byte(byte: u8) -> Self {
        Self {
            instance_function: byte & Self::INSTANCE == Self::INSTANCE,
            variadic: byte & Self::VARIADIC == Self::VARIADIC,
            generator: byte & Self::GENERATOR == Self::GENERATOR,
        }
    }

    /// Returns a byte containing the packed flags
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

/// Decoded instructions produced by an [InstructionReader] for execution in the runtime
///
/// For descriptions of each instruction's purpose, see corresponding [Op] entries.
#[allow(missing_docs)]
pub enum Instruction {
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
    SetBool {
        register: u8,
        value: bool,
    },
    SetNumber {
        register: u8,
        value: i64,
    },
    LoadFloat {
        register: u8,
        constant: ConstantIndex,
    },
    LoadInt {
        register: u8,
        constant: ConstantIndex,
    },
    LoadString {
        register: u8,
        constant: ConstantIndex,
    },
    LoadNonLocal {
        register: u8,
        constant: ConstantIndex,
    },
    ValueExport {
        name: u8,
        value: u8,
    },
    Import {
        register: u8,
    },
    MakeTempTuple {
        register: u8,
        start: u8,
        count: u8,
    },
    MakeMap {
        register: u8,
        size_hint: usize,
    },
    MakeNum2 {
        register: u8,
        count: u8,
        element_register: u8,
    },
    MakeNum4 {
        register: u8,
        count: u8,
        element_register: u8,
    },
    SequenceStart {
        register: u8,
        size_hint: usize,
    },
    SequencePush {
        sequence: u8,
        value: u8,
    },
    SequencePushN {
        sequence: u8,
        start: u8,
        count: u8,
    },
    SequenceToList {
        sequence: u8,
    },
    SequenceToTuple {
        sequence: u8,
    },
    Range {
        register: u8,
        start: u8,
        end: u8,
    },
    RangeInclusive {
        register: u8,
        start: u8,
        end: u8,
    },
    RangeTo {
        register: u8,
        end: u8,
    },
    RangeToInclusive {
        register: u8,
        end: u8,
    },
    RangeFrom {
        register: u8,
        start: u8,
    },
    RangeFull {
        register: u8,
    },
    MakeIterator {
        register: u8,
        iterable: u8,
    },
    SimpleFunction {
        register: u8,
        arg_count: u8,
        size: usize,
    },
    Function {
        register: u8,
        arg_count: u8,
        capture_count: u8,
        instance_function: bool,
        variadic: bool,
        generator: bool,
        size: usize,
    },
    Capture {
        function: u8,
        target: u8,
        source: u8,
    },
    Negate {
        register: u8,
        source: u8,
    },
    Add {
        register: u8,
        lhs: u8,
        rhs: u8,
    },
    Subtract {
        register: u8,
        lhs: u8,
        rhs: u8,
    },
    Multiply {
        register: u8,
        lhs: u8,
        rhs: u8,
    },
    Divide {
        register: u8,
        lhs: u8,
        rhs: u8,
    },
    Modulo {
        register: u8,
        lhs: u8,
        rhs: u8,
    },
    Less {
        register: u8,
        lhs: u8,
        rhs: u8,
    },
    LessOrEqual {
        register: u8,
        lhs: u8,
        rhs: u8,
    },
    Greater {
        register: u8,
        lhs: u8,
        rhs: u8,
    },
    GreaterOrEqual {
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
    JumpBack {
        offset: usize,
    },
    JumpBackIf {
        register: u8,
        offset: usize,
        jump_condition: bool,
    },
    Call {
        result: u8,
        function: u8,
        frame_base: u8,
        arg_count: u8,
    },
    CallChild {
        result: u8,
        function: u8,
        frame_base: u8,
        arg_count: u8,
        parent: u8,
    },
    Return {
        register: u8,
    },
    Yield {
        register: u8,
    },
    Throw {
        register: u8,
    },
    Size {
        register: u8,
        value: u8,
    },
    IterNext {
        register: u8,
        iterator: u8,
        jump_offset: usize,
    },
    IterNextTemp {
        register: u8,
        iterator: u8,
        jump_offset: usize,
    },
    IterNextQuiet {
        iterator: u8,
        jump_offset: usize,
    },
    ValueIndex {
        register: u8,
        value: u8,
        index: i8,
    },
    SliceFrom {
        register: u8,
        value: u8,
        index: i8,
    },
    SliceTo {
        register: u8,
        value: u8,
        index: i8,
    },
    IsTuple {
        register: u8,
        value: u8,
    },
    IsList {
        register: u8,
        value: u8,
    },
    Index {
        register: u8,
        value: u8,
        index: u8,
    },
    SetIndex {
        register: u8,
        index: u8,
        value: u8,
    },
    MapInsert {
        register: u8,
        key: u8,
        value: u8,
    },
    MetaInsert {
        register: u8,
        value: u8,
        id: MetaKeyId,
    },
    MetaInsertNamed {
        register: u8,
        value: u8,
        id: MetaKeyId,
        name: u8,
    },
    MetaExport {
        id: MetaKeyId,
        value: u8,
    },
    MetaExportNamed {
        id: MetaKeyId,
        name: u8,
        value: u8,
    },
    Access {
        register: u8,
        value: u8,
        key: u8,
    },
    TryStart {
        arg_register: u8,
        catch_offset: usize,
    },
    TryEnd,
    Debug {
        register: u8,
        constant: ConstantIndex,
    },
    CheckType {
        register: u8,
        type_id: TypeId,
    },
    CheckSize {
        register: u8,
        size: usize,
    },
    StringStart {
        register: u8,
    },
    StringPush {
        register: u8,
        value: u8,
    },
    StringFinish {
        register: u8,
    },
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Instruction::*;
        match self {
            Error { message } => unreachable!(message),
            Copy { .. } => write!(f, "Copy"),
            SetEmpty { .. } => write!(f, "SetEmpty"),
            SetBool { .. } => write!(f, "SetBool"),
            SetNumber { .. } => write!(f, "SetNumber"),
            LoadFloat { .. } => write!(f, "LoadFloat"),
            LoadInt { .. } => write!(f, "LoadInt"),
            LoadString { .. } => write!(f, "LoadString"),
            LoadNonLocal { .. } => write!(f, "LoadNonLocal"),
            ValueExport { .. } => write!(f, "ValueExport"),
            Import { .. } => write!(f, "Import"),
            MakeTempTuple { .. } => write!(f, "MakeTempTuple"),
            MakeMap { .. } => write!(f, "MakeMap"),
            MakeNum2 { .. } => write!(f, "MakeNum2"),
            MakeNum4 { .. } => write!(f, "MakeNum4"),
            SequenceStart { .. } => write!(f, "SequenceStart"),
            SequencePush { .. } => write!(f, "SequencePush"),
            SequencePushN { .. } => write!(f, "SequencePushN"),
            SequenceToList { .. } => write!(f, "SequenceToList"),
            SequenceToTuple { .. } => write!(f, "SequenceToTuple"),
            StringStart { .. } => write!(f, "StringStart"),
            StringPush { .. } => write!(f, "StringPush"),
            StringFinish { .. } => write!(f, "StringFinish"),
            Range { .. } => write!(f, "Range"),
            RangeInclusive { .. } => write!(f, "RangeInclusive"),
            RangeTo { .. } => write!(f, "RangeTo"),
            RangeToInclusive { .. } => write!(f, "RangeToInclusive"),
            RangeFrom { .. } => write!(f, "RangeFrom"),
            RangeFull { .. } => write!(f, "RangeFull"),
            MakeIterator { .. } => write!(f, "MakeIterator"),
            SimpleFunction { .. } => write!(f, "SimpleFunction"),
            Function { .. } => write!(f, "Function"),
            Capture { .. } => write!(f, "Capture"),
            Negate { .. } => write!(f, "Negate"),
            Add { .. } => write!(f, "Add"),
            Subtract { .. } => write!(f, "Subtract"),
            Multiply { .. } => write!(f, "Multiply"),
            Divide { .. } => write!(f, "Divide"),
            Modulo { .. } => write!(f, "Modulo"),
            Less { .. } => write!(f, "Less"),
            LessOrEqual { .. } => write!(f, "LessOrEqual"),
            Greater { .. } => write!(f, "Greater"),
            GreaterOrEqual { .. } => write!(f, "GreaterOrEqual"),
            Equal { .. } => write!(f, "Equal"),
            NotEqual { .. } => write!(f, "NotEqual"),
            Jump { .. } => write!(f, "Jump"),
            JumpIf { .. } => write!(f, "JumpIf"),
            JumpBack { .. } => write!(f, "JumpBack"),
            JumpBackIf { .. } => write!(f, "JumpBackIf"),
            Call { .. } => write!(f, "Call"),
            CallChild { .. } => write!(f, "CallChild"),
            Return { .. } => write!(f, "Return"),
            Yield { .. } => write!(f, "Yield"),
            Throw { .. } => write!(f, "Throw"),
            Size { .. } => write!(f, "Size"),
            IterNext { .. } => write!(f, "IterNext"),
            IterNextTemp { .. } => write!(f, "IterNextTemp"),
            IterNextQuiet { .. } => write!(f, "IterNextQuiet"),
            ValueIndex { .. } => write!(f, "ValueIndex"),
            SliceFrom { .. } => write!(f, "SliceFrom"),
            SliceTo { .. } => write!(f, "SliceTo"),
            IsTuple { .. } => write!(f, "IsTuple"),
            IsList { .. } => write!(f, "IsList"),
            Index { .. } => write!(f, "Index"),
            SetIndex { .. } => write!(f, "SetIndex"),
            MapInsert { .. } => write!(f, "MapInsert"),
            MetaInsert { .. } => write!(f, "MetaInsert"),
            MetaInsertNamed { .. } => write!(f, "MetaInsertNamed"),
            MetaExport { .. } => write!(f, "MetaExport"),
            MetaExportNamed { .. } => write!(f, "MetaExportNamed"),
            Access { .. } => write!(f, "Access"),
            TryStart { .. } => write!(f, "TryStart"),
            TryEnd => write!(f, "TryEnd"),
            Debug { .. } => write!(f, "Debug"),
            CheckType { .. } => write!(f, "CheckType"),
            CheckSize { .. } => write!(f, "CheckSize"),
        }
    }
}

impl fmt::Debug for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Instruction::*;
        match self {
            Error { message } => unreachable!(message),
            Copy { target, source } => write!(f, "Copy\t\tresult: {}\tsource: {}", target, source),
            SetEmpty { register } => write!(f, "SetEmpty\tresult: {}", register),
            SetBool { register, value } => {
                write!(f, "SetBool\t\tresult: {}\tvalue: {}", register, value)
            }
            SetNumber { register, value } => {
                write!(f, "SetNumber\tresult: {}\tvalue: {}", register, value)
            }
            LoadFloat { register, constant } => {
                write!(f, "LoadFloat\tresult: {}\tconstant: {}", register, constant)
            }
            LoadInt { register, constant } => {
                write!(f, "LoadInt\t\tresult: {}\tconstant: {}", register, constant)
            }
            LoadString { register, constant } => write!(
                f,
                "LoadString\tresult: {}\tconstant: {}",
                register, constant
            ),
            LoadNonLocal { register, constant } => write!(
                f,
                "LoadNonLocal\tresult: {}\tconstant: {}",
                register, constant
            ),
            ValueExport { name, value } => {
                write!(f, "ValueExport\tname: {}\t\tvalue: {}", name, value)
            }
            Import { register } => write!(f, "Import\t\tregister: {}", register),
            MakeTempTuple {
                register,
                start,
                count,
            } => write!(
                f,
                "MakeTempTuple\tresult: {}\tstart: {}\tcount: {}",
                register, start, count
            ),
            MakeMap {
                register,
                size_hint,
            } => write!(
                f,
                "MakeMap\t\tresult: {}\tsize_hint: {}",
                register, size_hint
            ),
            MakeNum2 {
                register,
                count,
                element_register,
            } => write!(
                f,
                "MakeNum2\tresult: {}\tcount: {}\telement reg: {}",
                register, count, element_register
            ),
            MakeNum4 {
                register,
                count,
                element_register,
            } => write!(
                f,
                "MakeNum4\tresult: {}\tcount: {}\telement reg: {}",
                register, count, element_register
            ),
            SequenceStart {
                register,
                size_hint,
            } => write!(
                f,
                "SequenceStart\tresult: {}\tsize_hint: {}",
                register, size_hint
            ),
            SequencePush { sequence, value } => {
                write!(f, "SequencePush\tsequence: {}\tvalue: {}", sequence, value)
            }
            SequencePushN {
                sequence,
                start,
                count,
            } => write!(
                f,
                "SequencePushN\tsequence: {}\tstart: {}\tcount: {}",
                sequence, start, count
            ),
            SequenceToList { sequence } => write!(f, "SequenceToList\tsequence: {}", sequence),
            SequenceToTuple { sequence } => write!(f, "SequenceToTuple\tsequence: {}", sequence),
            Range {
                register,
                start,
                end,
            } => write!(
                f,
                "Range\t\tresult: {}\tstart: {}\tend: {}",
                register, start, end
            ),
            RangeInclusive {
                register,
                start,
                end,
            } => write!(
                f,
                "RangeInclusive\tresult: {}\tstart: {}\tend: {}",
                register, start, end
            ),
            RangeTo { register, end } => write!(f, "RangeTo\t\tresult: {}\tend: {}", register, end),
            RangeToInclusive { register, end } => {
                write!(f, "RangeToIncl\tresult: {}\tend: {}", register, end)
            }
            RangeFrom { register, start } => {
                write!(f, "RangeFrom\tresult: {}\tstart: {}", register, start)
            }
            RangeFull { register } => write!(f, "RangeFull\tresult: {}", register),
            MakeIterator { register, iterable } => write!(
                f,
                "MakeIterator\tresult: {}\titerable: {}",
                register, iterable
            ),
            SimpleFunction {
                register,
                arg_count,
                size,
            } => write!(
                f,
                "SimpleFunction\tresult: {}\targs: {}\t\tsize: {}",
                register, arg_count, size,
            ),
            Function {
                register,
                arg_count,
                capture_count,
                instance_function,
                variadic,
                generator,
                size,
            } => write!(
                f,
                "Function\tresult: {}\targs: {}\t\tcaptures: {}\tsize: {}\n\
                     \t\t\tinstance: {}\tvariadic: {}\tgenerator: {}",
                register, arg_count, capture_count, size, instance_function, variadic, generator,
            ),
            Capture {
                function,
                target,
                source,
            } => write!(
                f,
                "Capture\t\tfunction: {}\ttarget: {}\tsource: {}",
                function, target, source
            ),
            Negate { register, source } => {
                write!(f, "Negate\t\tresult: {}\tsource: {}", register, source)
            }
            Add { register, lhs, rhs } => write!(
                f,
                "Add\t\tresult: {}\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            Subtract { register, lhs, rhs } => write!(
                f,
                "Subtract\tresult: {}\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            Multiply { register, lhs, rhs } => write!(
                f,
                "Multiply\tresult: {}\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            Divide { register, lhs, rhs } => write!(
                f,
                "Divide\t\tresult: {}\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            Modulo { register, lhs, rhs } => write!(
                f,
                "Modulo\t\tresult: {}\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            Less { register, lhs, rhs } => write!(
                f,
                "Less\t\tresult: {}\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            LessOrEqual { register, lhs, rhs } => write!(
                f,
                "LessOrEqual\tresult: {}\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            Greater { register, lhs, rhs } => write!(
                f,
                "Greater\t\tresult: {}\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            GreaterOrEqual { register, lhs, rhs } => write!(
                f,
                "GreaterOrEqual\tresult: {}\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            Equal { register, lhs, rhs } => write!(
                f,
                "Equal\t\tresult: {}\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            NotEqual { register, lhs, rhs } => write!(
                f,
                "NotEqual\tresult: {}\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            Jump { offset } => write!(f, "Jump\t\toffset: {}", offset),
            JumpIf {
                register,
                offset,
                jump_condition,
            } => write!(
                f,
                "JumpIf\t\tresult: {}\toffset: {}\tcondition: {}",
                register, offset, jump_condition
            ),
            JumpBack { offset } => write!(f, "JumpBack\toffset: {}", offset),
            JumpBackIf {
                register,
                offset,
                jump_condition,
            } => write!(
                f,
                "JumpBackIf\tresult: {}\toffset: {}\tcondition: {}",
                register, offset, jump_condition
            ),
            Call {
                result,
                function,
                frame_base,
                arg_count,
            } => write!(
                f,
                "Call\t\tresult: {}\tfunction: {}\tframe base: {}\targs: {}",
                result, function, frame_base, arg_count
            ),
            CallChild {
                result,
                function,
                parent,
                frame_base,
                arg_count,
            } => write!(
                f,
                "CallChild\tresult: {}\tfunction: {}\tframe_base: {}\n\t\t\targs: {}\t\tparent: {}",
                result, function, frame_base, arg_count, parent
            ),
            Return { register } => write!(f, "Return\t\tresult: {}", register),
            Yield { register } => write!(f, "Yield\t\tresult: {}", register),
            Throw { register } => write!(f, "Throw\t\tresult: {}", register),
            Size { register, value } => write!(f, "Size\t\tresult: {}\tvalue: {}", register, value),
            IterNext {
                register,
                iterator,
                jump_offset,
            } => write!(
                f,
                "IterNext\tresult: {}\titerator: {}\tjump offset: {}",
                register, iterator, jump_offset
            ),
            IterNextTemp {
                register,
                iterator,
                jump_offset,
            } => write!(
                f,
                "IterNextTemp\tresult: {}\titerator: {}\tjump offset: {}",
                register, iterator, jump_offset
            ),
            IterNextQuiet {
                iterator,
                jump_offset,
            } => write!(
                f,
                "IterNextQuiet\titerator: {}\tjump offset: {}",
                iterator, jump_offset
            ),
            ValueIndex {
                register,
                value,
                index,
            } => write!(
                f,
                "ValueIndex\tresult: {}\tvalue: {}\tindex: {}",
                register, value, index
            ),
            SliceFrom {
                register,
                value,
                index,
            } => write!(
                f,
                "SliceFrom\tresult: {}\tvalue: {}\tindex: {}",
                register, value, index
            ),
            SliceTo {
                register,
                value,
                index,
            } => write!(
                f,
                "SliceTo\t\tresult: {}\tvalue: {}\tindex: {}",
                register, value, index
            ),
            IsTuple { register, value } => {
                write!(f, "IsTuple\t\tresult: {}\tvalue: {}", register, value)
            }
            IsList { register, value } => {
                write!(f, "IsList\t\tresult: {}\tvalue: {}", register, value)
            }
            Index {
                register,
                value,
                index,
            } => write!(
                f,
                "Index\t\tresult: {}\tvalue: {}\tindex: {}",
                register, value, index
            ),
            SetIndex {
                register,
                index,
                value,
            } => write!(
                f,
                "SetIndex\tregister: {}\t\tindex: {}\tvalue: {}",
                register, index, value
            ),
            MapInsert {
                register,
                value,
                key,
            } => write!(
                f,
                "MapInsert\tmap: {}\t\tvalue: {}\tkey: {}",
                register, value, key
            ),
            MetaInsert {
                register,
                value,
                id,
            } => write!(
                f,
                "MetaInsert\tmap: {}\t\tid: {:?}\tvalue: {}",
                register, id, value
            ),
            MetaInsertNamed {
                register,
                id,
                name,
                value,
            } => write!(
                f,
                "MetaInsertNamed\tmap: {}\t\tid: {:?}\tname: {}\t\tvalue: {}",
                register, id, name, value
            ),
            MetaExport { id, value } => write!(f, "MetaExport\tid: {:?}\tvalue: {}", id, value),
            MetaExportNamed { id, name, value } => write!(
                f,
                "MetaExportNamed\tid: {:?}\tname: {}\tvalue: {}",
                id, name, value,
            ),
            Access {
                register,
                value,
                key,
            } => write!(
                f,
                "Access\t\tresult: {}\tvalue: {}\tkey: {}",
                register, value, key
            ),
            TryStart {
                arg_register,
                catch_offset,
            } => write!(
                f,
                "TryStart\targ register: {}\tcatch offset: {}",
                arg_register, catch_offset
            ),
            TryEnd => write!(f, "TryEnd"),
            Debug { register, constant } => {
                write!(f, "Debug\t\tregister: {}\tconstant: {}", register, constant)
            }
            CheckType { register, type_id } => {
                write!(f, "CheckType\tregister: {}\ttype: {:?}", register, type_id)
            }
            CheckSize { register, size } => {
                write!(f, "CheckSize\tregister: {}\tsize: {}", register, size)
            }
            StringStart { register } => {
                write!(f, "StringStart\tregister: {}", register)
            }
            StringPush { register, value } => {
                write!(f, "StringPush\tregister: {}\tvalue: {}", register, value)
            }
            StringFinish { register } => {
                write!(f, "StringFinish\tregister: {}", register)
            }
        }
    }
}

/// An iterator that converts bytecode into a series of [Instruction]s
#[derive(Clone, Default)]
pub struct InstructionReader {
    /// The chunk that the reader is reading from
    pub chunk: Arc<Chunk>,
    /// The reader's instruction pointer
    pub ip: usize,
}

impl InstructionReader {
    /// Initializes a reader with the given chunk
    pub fn new(chunk: Arc<Chunk>) -> Self {
        Self { chunk, ip: 0 }
    }
}

impl Iterator for InstructionReader {
    type Item = Instruction;

    fn next(&mut self) -> Option<Self::Item> {
        use Instruction::*;

        macro_rules! get_u8 {
            () => {{
                match self.chunk.bytes.get(self.ip) {
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
                match self.chunk.bytes.get(self.ip..self.ip + 2) {
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
                match self.chunk.bytes.get(self.ip..self.ip + 4) {
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

        let op = match self.chunk.bytes.get(self.ip) {
            Some(byte) => Op::from(*byte),
            None => return None,
        };
        let op_ip = self.ip;

        self.ip += 1;

        match op {
            Op::Copy => Some(Copy {
                target: get_u8!(),
                source: get_u8!(),
            }),
            Op::SetEmpty => Some(SetEmpty {
                register: get_u8!(),
            }),
            Op::SetFalse => Some(SetBool {
                register: get_u8!(),
                value: false,
            }),
            Op::SetTrue => Some(SetBool {
                register: get_u8!(),
                value: true,
            }),
            Op::Set0 => Some(SetNumber {
                register: get_u8!(),
                value: 0,
            }),
            Op::Set1 => Some(SetNumber {
                register: get_u8!(),
                value: 1,
            }),
            Op::SetNumberU8 => Some(SetNumber {
                register: get_u8!(),
                value: get_u8!() as i64,
            }),
            Op::LoadFloat => Some(LoadFloat {
                register: get_u8!(),
                constant: ConstantIndex(get_u8!(), 0, 0),
            }),
            Op::LoadFloat16 => Some(LoadFloat {
                register: get_u8!(),
                constant: ConstantIndex(get_u8!(), get_u8!(), 0),
            }),
            Op::LoadFloat24 => Some(LoadFloat {
                register: get_u8!(),
                constant: ConstantIndex(get_u8!(), get_u8!(), get_u8!()),
            }),
            Op::LoadInt => Some(LoadInt {
                register: get_u8!(),
                constant: ConstantIndex(get_u8!(), 0, 0),
            }),
            Op::LoadInt16 => Some(LoadInt {
                register: get_u8!(),
                constant: ConstantIndex(get_u8!(), get_u8!(), 0),
            }),
            Op::LoadInt24 => Some(LoadInt {
                register: get_u8!(),
                constant: ConstantIndex(get_u8!(), get_u8!(), get_u8!()),
            }),
            Op::LoadString => Some(LoadString {
                register: get_u8!(),
                constant: ConstantIndex(get_u8!(), 0, 0),
            }),
            Op::LoadString16 => Some(LoadString {
                register: get_u8!(),
                constant: ConstantIndex(get_u8!(), get_u8!(), 0),
            }),
            Op::LoadString24 => Some(LoadString {
                register: get_u8!(),
                constant: ConstantIndex(get_u8!(), get_u8!(), get_u8!()),
            }),
            Op::LoadNonLocal => Some(LoadNonLocal {
                register: get_u8!(),
                constant: ConstantIndex(get_u8!(), 0, 0),
            }),
            Op::LoadNonLocal16 => Some(LoadNonLocal {
                register: get_u8!(),
                constant: ConstantIndex(get_u8!(), get_u8!(), 0),
            }),
            Op::LoadNonLocal24 => Some(LoadNonLocal {
                register: get_u8!(),
                constant: ConstantIndex(get_u8!(), get_u8!(), get_u8!()),
            }),
            Op::ValueExport => Some(ValueExport {
                name: get_u8!(),
                value: get_u8!(),
            }),
            Op::Import => Some(Import {
                register: get_u8!(),
            }),
            Op::MakeTempTuple => Some(MakeTempTuple {
                register: get_u8!(),
                start: get_u8!(),
                count: get_u8!(),
            }),
            Op::MakeMap => Some(MakeMap {
                register: get_u8!(),
                size_hint: get_u8!() as usize,
            }),
            Op::MakeMap32 => Some(MakeMap {
                register: get_u8!(),
                size_hint: get_u32!() as usize,
            }),
            Op::MakeNum2 => Some(MakeNum2 {
                register: get_u8!(),
                count: get_u8!(),
                element_register: get_u8!(),
            }),
            Op::MakeNum4 => Some(MakeNum4 {
                register: get_u8!(),
                count: get_u8!(),
                element_register: get_u8!(),
            }),
            Op::SequenceStart => Some(SequenceStart {
                register: get_u8!(),
                size_hint: get_u8!() as usize,
            }),
            Op::SequenceStart32 => Some(SequenceStart {
                register: get_u8!(),
                size_hint: get_u32!() as usize,
            }),
            Op::SequencePush => Some(SequencePush {
                sequence: get_u8!(),
                value: get_u8!(),
            }),
            Op::SequencePushN => Some(SequencePushN {
                sequence: get_u8!(),
                start: get_u8!(),
                count: get_u8!(),
            }),
            Op::SequenceToList => Some(SequenceToList {
                sequence: get_u8!(),
            }),
            Op::SequenceToTuple => Some(SequenceToTuple {
                sequence: get_u8!(),
            }),
            Op::Range => Some(Range {
                register: get_u8!(),
                start: get_u8!(),
                end: get_u8!(),
            }),
            Op::RangeInclusive => Some(RangeInclusive {
                register: get_u8!(),
                start: get_u8!(),
                end: get_u8!(),
            }),
            Op::RangeTo => Some(RangeTo {
                register: get_u8!(),
                end: get_u8!(),
            }),
            Op::RangeToInclusive => Some(RangeToInclusive {
                register: get_u8!(),
                end: get_u8!(),
            }),
            Op::RangeFrom => Some(RangeFrom {
                register: get_u8!(),
                start: get_u8!(),
            }),
            Op::RangeFull => Some(RangeFull {
                register: get_u8!(),
            }),
            Op::MakeIterator => Some(MakeIterator {
                register: get_u8!(),
                iterable: get_u8!(),
            }),
            Op::SimpleFunction => {
                let register = get_u8!();
                let arg_count = get_u8!();
                let size = get_u16!() as usize;

                Some(SimpleFunction {
                    register,
                    arg_count,
                    size,
                })
            }
            Op::Function => {
                let register = get_u8!();
                let arg_count = get_u8!();
                let capture_count = get_u8!();
                let flags = FunctionFlags::from_byte(get_u8!());
                let size = get_u16!() as usize;

                Some(Function {
                    register,
                    arg_count,
                    capture_count,
                    instance_function: flags.instance_function,
                    variadic: flags.variadic,
                    generator: flags.generator,
                    size,
                })
            }
            Op::Capture => Some(Capture {
                function: get_u8!(),
                target: get_u8!(),
                source: get_u8!(),
            }),
            Op::Negate => Some(Negate {
                register: get_u8!(),
                source: get_u8!(),
            }),
            Op::Add => Some(Add {
                register: get_u8!(),
                lhs: get_u8!(),
                rhs: get_u8!(),
            }),
            Op::Subtract => Some(Subtract {
                register: get_u8!(),
                lhs: get_u8!(),
                rhs: get_u8!(),
            }),
            Op::Multiply => Some(Multiply {
                register: get_u8!(),
                lhs: get_u8!(),
                rhs: get_u8!(),
            }),
            Op::Divide => Some(Divide {
                register: get_u8!(),
                lhs: get_u8!(),
                rhs: get_u8!(),
            }),
            Op::Modulo => Some(Modulo {
                register: get_u8!(),
                lhs: get_u8!(),
                rhs: get_u8!(),
            }),
            Op::Less => Some(Less {
                register: get_u8!(),
                lhs: get_u8!(),
                rhs: get_u8!(),
            }),
            Op::LessOrEqual => Some(LessOrEqual {
                register: get_u8!(),
                lhs: get_u8!(),
                rhs: get_u8!(),
            }),
            Op::Greater => Some(Greater {
                register: get_u8!(),
                lhs: get_u8!(),
                rhs: get_u8!(),
            }),
            Op::GreaterOrEqual => Some(GreaterOrEqual {
                register: get_u8!(),
                lhs: get_u8!(),
                rhs: get_u8!(),
            }),
            Op::Equal => Some(Equal {
                register: get_u8!(),
                lhs: get_u8!(),
                rhs: get_u8!(),
            }),
            Op::NotEqual => Some(NotEqual {
                register: get_u8!(),
                lhs: get_u8!(),
                rhs: get_u8!(),
            }),
            Op::Jump => Some(Jump {
                offset: get_u16!() as usize,
            }),
            Op::JumpTrue => Some(JumpIf {
                register: get_u8!(),
                offset: get_u16!() as usize,
                jump_condition: true,
            }),
            Op::JumpFalse => Some(JumpIf {
                register: get_u8!(),
                offset: get_u16!() as usize,
                jump_condition: false,
            }),
            Op::JumpBack => Some(JumpBack {
                offset: get_u16!() as usize,
            }),
            Op::JumpBackFalse => Some(JumpBackIf {
                register: get_u8!(),
                offset: get_u16!() as usize,
                jump_condition: false,
            }),
            Op::Call => Some(Call {
                result: get_u8!(),
                function: get_u8!(),
                frame_base: get_u8!(),
                arg_count: get_u8!(),
            }),
            Op::CallChild => Some(CallChild {
                result: get_u8!(),
                function: get_u8!(),
                frame_base: get_u8!(),
                arg_count: get_u8!(),
                parent: get_u8!(),
            }),
            Op::Return => Some(Return {
                register: get_u8!(),
            }),
            Op::Yield => Some(Yield {
                register: get_u8!(),
            }),
            Op::Throw => Some(Throw {
                register: get_u8!(),
            }),
            Op::Size => Some(Size {
                register: get_u8!(),
                value: get_u8!(),
            }),
            Op::IterNext => Some(IterNext {
                register: get_u8!(),
                iterator: get_u8!(),
                jump_offset: get_u16!() as usize,
            }),
            Op::IterNextTemp => Some(IterNextTemp {
                register: get_u8!(),
                iterator: get_u8!(),
                jump_offset: get_u16!() as usize,
            }),
            Op::IterNextQuiet => Some(IterNextQuiet {
                iterator: get_u8!(),
                jump_offset: get_u16!() as usize,
            }),
            Op::ValueIndex => Some(ValueIndex {
                register: get_u8!(),
                value: get_u8!(),
                index: get_u8!() as i8,
            }),
            Op::SliceFrom => Some(SliceFrom {
                register: get_u8!(),
                value: get_u8!(),
                index: get_u8!() as i8,
            }),
            Op::SliceTo => Some(SliceTo {
                register: get_u8!(),
                value: get_u8!(),
                index: get_u8!() as i8,
            }),
            Op::IsTuple => Some(IsTuple {
                register: get_u8!(),
                value: get_u8!(),
            }),
            Op::IsList => Some(IsList {
                register: get_u8!(),
                value: get_u8!(),
            }),
            Op::Index => Some(Index {
                register: get_u8!(),
                value: get_u8!(),
                index: get_u8!(),
            }),
            Op::SetIndex => Some(SetIndex {
                register: get_u8!(),
                index: get_u8!(),
                value: get_u8!(),
            }),
            Op::MapInsert => Some(MapInsert {
                register: get_u8!(),
                key: get_u8!(),
                value: get_u8!(),
            }),
            Op::MetaInsert => {
                let register = get_u8!();
                let meta_id = get_u8!();
                let value = get_u8!();
                if let Ok(id) = meta_id.try_into() {
                    Some(MetaInsert {
                        register,
                        value,
                        id,
                    })
                } else {
                    Some(Error {
                        message: format!(
                            "Unexpected meta id {} found at instruction {}",
                            meta_id, op_ip
                        ),
                    })
                }
            }
            Op::MetaInsertNamed => {
                let register = get_u8!();
                let meta_id = get_u8!();
                let name = get_u8!();
                let value = get_u8!();
                if let Ok(id) = meta_id.try_into() {
                    Some(MetaInsertNamed {
                        register,
                        value,
                        id,
                        name,
                    })
                } else {
                    Some(Error {
                        message: format!(
                            "Unexpected meta id {} found at instruction {}",
                            meta_id, op_ip
                        ),
                    })
                }
            }
            Op::MetaExport => {
                let meta_id = get_u8!();
                let value = get_u8!();
                if let Ok(id) = meta_id.try_into() {
                    Some(MetaExport { id, value })
                } else {
                    Some(Error {
                        message: format!(
                            "Unexpected meta id {} found at instruction {}",
                            meta_id, op_ip
                        ),
                    })
                }
            }
            Op::MetaExportNamed => {
                let meta_id = get_u8!();
                let name = get_u8!();
                let value = get_u8!();
                if let Ok(id) = meta_id.try_into() {
                    Some(MetaExportNamed { id, value, name })
                } else {
                    Some(Error {
                        message: format!(
                            "Unexpected meta id {} found at instruction {}",
                            meta_id, op_ip
                        ),
                    })
                }
            }
            Op::Access => Some(Access {
                register: get_u8!(),
                value: get_u8!(),
                key: get_u8!(),
            }),
            Op::TryStart => Some(TryStart {
                arg_register: get_u8!(),
                catch_offset: get_u16!() as usize,
            }),
            Op::TryEnd => Some(TryEnd),
            Op::Debug => Some(Debug {
                register: get_u8!(),
                constant: ConstantIndex(get_u8!(), get_u8!(), get_u8!()),
            }),
            Op::CheckType => {
                let register = get_u8!();
                match TypeId::from_byte(get_u8!()) {
                    Ok(type_id) => Some(CheckType { register, type_id }),
                    Err(byte) => Some(Error {
                        message: format!("Unexpected value for CheckType id: {}", byte),
                    }),
                }
            }
            Op::CheckSize => Some(CheckSize {
                register: get_u8!(),
                size: get_u8!() as usize,
            }),
            Op::StringStart => Some(StringStart {
                register: get_u8!(),
            }),
            Op::StringPush => Some(StringPush {
                register: get_u8!(),
                value: get_u8!(),
            }),
            Op::StringFinish => Some(StringFinish {
                register: get_u8!(),
            }),
            _ => Some(Error {
                message: format!("Unexpected opcode {:?} found at instruction {}", op, op_ip),
            }),
        }
    }
}
