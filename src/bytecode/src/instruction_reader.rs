use {
    crate::{Chunk, Op},
    koto_parser::{ConstantIndex, MetaKeyId},
    std::{fmt, rc::Rc},
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
    /// True if the function has a single argument which is an unpacked tuple
    pub arg_is_unpacked_tuple: bool,
}

impl FunctionFlags {
    /// Corresponding to [FunctionFlags::instance_function]
    pub const INSTANCE: u8 = 1 << 0;
    /// Corresponding to [FunctionFlags::variadic]
    pub const VARIADIC: u8 = 1 << 1;
    /// Corresponding to [FunctionFlags::generator]
    pub const GENERATOR: u8 = 1 << 2;
    /// Corresponding to [FunctionFlags::arg_is_unpacked_tuple]
    pub const ARG_IS_UNPACKED_TUPLE: u8 = 1 << 3;

    /// Initializes a flags struct from a byte
    pub fn from_byte(byte: u8) -> Self {
        Self {
            instance_function: byte & Self::INSTANCE != 0,
            variadic: byte & Self::VARIADIC != 0,
            generator: byte & Self::GENERATOR != 0,
            arg_is_unpacked_tuple: byte & Self::ARG_IS_UNPACKED_TUPLE != 0,
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
        if self.arg_is_unpacked_tuple {
            result |= Self::ARG_IS_UNPACKED_TUPLE;
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
    SetNull {
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
    TempTupleToTuple {
        register: u8,
        source: u8,
    },
    MakeMap {
        register: u8,
        size_hint: usize,
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
        arg_is_unpacked_tuple: bool,
        size: usize,
    },
    Capture {
        function: u8,
        target: u8,
        source: u8,
    },
    Negate {
        register: u8,
        value: u8,
    },
    Not {
        register: u8,
        value: u8,
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
    Remainder {
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
    JumpBack {
        offset: usize,
    },
    JumpIfTrue {
        register: u8,
        offset: usize,
    },
    JumpIfFalse {
        register: u8,
        offset: usize,
    },
    Call {
        result: u8,
        function: u8,
        frame_base: u8,
        arg_count: u8,
    },
    CallInstance {
        result: u8,
        function: u8,
        frame_base: u8,
        arg_count: u8,
        instance: u8,
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
    TempIndex {
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
        key: ConstantIndex,
    },
    AccessString {
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
        size_hint: usize,
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
            Error { message } => unreachable!("{message}"),
            Copy { .. } => write!(f, "Copy"),
            SetNull { .. } => write!(f, "SetNull"),
            SetBool { .. } => write!(f, "SetBool"),
            SetNumber { .. } => write!(f, "SetNumber"),
            LoadFloat { .. } => write!(f, "LoadFloat"),
            LoadInt { .. } => write!(f, "LoadInt"),
            LoadString { .. } => write!(f, "LoadString"),
            LoadNonLocal { .. } => write!(f, "LoadNonLocal"),
            ValueExport { .. } => write!(f, "ValueExport"),
            Import { .. } => write!(f, "Import"),
            MakeTempTuple { .. } => write!(f, "MakeTempTuple"),
            TempTupleToTuple { .. } => write!(f, "TempTupleToTuple"),
            MakeMap { .. } => write!(f, "MakeMap"),
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
            Not { .. } => write!(f, "Not"),
            Add { .. } => write!(f, "Add"),
            Subtract { .. } => write!(f, "Subtract"),
            Multiply { .. } => write!(f, "Multiply"),
            Divide { .. } => write!(f, "Divide"),
            Remainder { .. } => write!(f, "Remainder"),
            Less { .. } => write!(f, "Less"),
            LessOrEqual { .. } => write!(f, "LessOrEqual"),
            Greater { .. } => write!(f, "Greater"),
            GreaterOrEqual { .. } => write!(f, "GreaterOrEqual"),
            Equal { .. } => write!(f, "Equal"),
            NotEqual { .. } => write!(f, "NotEqual"),
            Jump { .. } => write!(f, "Jump"),
            JumpBack { .. } => write!(f, "JumpBack"),
            JumpIfTrue { .. } => write!(f, "JumpIfTrue"),
            JumpIfFalse { .. } => write!(f, "JumpIfFalse"),
            Call { .. } => write!(f, "Call"),
            CallInstance { .. } => write!(f, "CallInstance"),
            Return { .. } => write!(f, "Return"),
            Yield { .. } => write!(f, "Yield"),
            Throw { .. } => write!(f, "Throw"),
            Size { .. } => write!(f, "Size"),
            IterNext { .. } => write!(f, "IterNext"),
            IterNextTemp { .. } => write!(f, "IterNextTemp"),
            IterNextQuiet { .. } => write!(f, "IterNextQuiet"),
            TempIndex { .. } => write!(f, "TempIndex"),
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
            AccessString { .. } => write!(f, "AccessString"),
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
            Error { message } => unreachable!("{message}"),
            Copy { target, source } => write!(f, "Copy\t\tresult: {target}\tsource: {source}"),
            SetNull { register } => write!(f, "SetNull\t\tresult: {register}"),
            SetBool { register, value } => {
                write!(f, "SetBool\t\tresult: {register}\tvalue: {value}")
            }
            SetNumber { register, value } => {
                write!(f, "SetNumber\tresult: {register}\tvalue: {value}")
            }
            LoadFloat { register, constant } => {
                write!(f, "LoadFloat\tresult: {register}\tconstant: {constant}")
            }
            LoadInt { register, constant } => {
                write!(f, "LoadInt\t\tresult: {register}\tconstant: {constant}")
            }
            LoadString { register, constant } => {
                write!(f, "LoadString\tresult: {register}\tconstant: {constant}",)
            }
            LoadNonLocal { register, constant } => {
                write!(f, "LoadNonLocal\tresult: {register}\tconstant: {constant}",)
            }
            ValueExport { name, value } => {
                write!(f, "ValueExport\tname: {name}\t\tvalue: {value}")
            }
            Import { register } => write!(f, "Import\t\tregister: {register}"),
            MakeTempTuple {
                register,
                start,
                count,
            } => write!(
                f,
                "MakeTempTuple\tresult: {register}\tstart: {start}\tcount: {count}"
            ),
            TempTupleToTuple { register, source } => {
                write!(f, "TempTupleToTuple\tresult: {register}\tsource: {source}")
            }
            MakeMap {
                register,
                size_hint,
            } => write!(f, "MakeMap\t\tresult: {register}\tsize_hint: {size_hint}"),
            SequenceStart {
                register,
                size_hint,
            } => write!(
                f,
                "SequenceStart\tresult: {register}\tsize_hint: {size_hint}"
            ),
            SequencePush { sequence, value } => {
                write!(f, "SequencePush\tsequence: {sequence}\tvalue: {value}")
            }
            SequencePushN {
                sequence,
                start,
                count,
            } => write!(
                f,
                "SequencePushN\tsequence: {sequence}\tstart: {start}\tcount: {count}",
            ),
            SequenceToList { sequence } => write!(f, "SequenceToList\tsequence: {sequence}"),
            SequenceToTuple { sequence } => write!(f, "SequenceToTuple\tsequence: {sequence}"),
            Range {
                register,
                start,
                end,
            } => write!(f, "Range\t\tresult: {register}\tstart: {start}\tend: {end}",),
            RangeInclusive {
                register,
                start,
                end,
            } => write!(
                f,
                "RangeInclusive\tresult: {register}\tstart: {start}\tend: {end}",
            ),
            RangeTo { register, end } => write!(f, "RangeTo\t\tresult: {register}\tend: {end}"),
            RangeToInclusive { register, end } => {
                write!(f, "RangeToIncl\tresult: {register}\tend: {end}")
            }
            RangeFrom { register, start } => {
                write!(f, "RangeFrom\tresult: {register}\tstart: {start}")
            }
            RangeFull { register } => write!(f, "RangeFull\tresult: {register}"),
            MakeIterator { register, iterable } => {
                write!(f, "MakeIterator\tresult: {register}\titerable: {iterable}",)
            }
            SimpleFunction {
                register,
                arg_count,
                size,
            } => write!(
                f,
                "SimpleFunction\tresult: {register}\targs: {arg_count}\t\tsize: {size}",
            ),
            Function {
                register,
                arg_count,
                capture_count,
                instance_function,
                variadic,
                generator,
                arg_is_unpacked_tuple,
                size,
            } => write!(
                f,
                "Function\tresult: {register}\targs: {arg_count}\
                 \t\tcaptures: {capture_count}\tsize: {size}
                 \t\t\tinstance: {instance_function}\tgenerator: {generator}
                 \t\t\tvariadic: {variadic}\targ_is_unpacked_tuple: {arg_is_unpacked_tuple}",
            ),
            Capture {
                function,
                target,
                source,
            } => write!(
                f,
                "Capture\t\tfunction: {function}\ttarget: {target}\tsource: {source}",
            ),
            Negate { register, value } => {
                write!(f, "Negate\t\tresult: {register}\tsource: {value}")
            }
            Not { register, value } => {
                write!(f, "Not\t\tresult: {register}\tsource: {value}")
            }
            Add { register, lhs, rhs } => {
                write!(f, "Add\t\tresult: {register}\tlhs: {lhs}\t\trhs: {rhs}")
            }
            Subtract { register, lhs, rhs } => {
                write!(f, "Subtract\tresult: {register}\tlhs: {lhs}\t\trhs: {rhs}")
            }
            Multiply { register, lhs, rhs } => {
                write!(f, "Multiply\tresult: {register}\tlhs: {lhs}\t\trhs: {rhs}")
            }
            Divide { register, lhs, rhs } => {
                write!(f, "Divide\t\tresult: {register}\tlhs: {lhs}\t\trhs: {rhs}")
            }
            Remainder { register, lhs, rhs } => {
                write!(
                    f,
                    "Remainder\t\tresult: {register}\tlhs: {lhs}\t\trhs: {rhs}"
                )
            }
            Less { register, lhs, rhs } => {
                write!(f, "Less\t\tresult: {register}\tlhs: {lhs}\t\trhs: {rhs}")
            }
            LessOrEqual { register, lhs, rhs } => write!(
                f,
                "LessOrEqual\tresult: {register}\tlhs: {lhs}\t\trhs: {}",
                rhs
            ),
            Greater { register, lhs, rhs } => {
                write!(f, "Greater\t\tresult: {register}\tlhs: {lhs}\t\trhs: {rhs}")
            }
            GreaterOrEqual { register, lhs, rhs } => write!(
                f,
                "GreaterOrEqual\tresult: {register}\tlhs: {lhs}\t\trhs: {rhs}",
            ),
            Equal { register, lhs, rhs } => {
                write!(f, "Equal\t\tresult: {register}\tlhs: {lhs}\t\trhs: {rhs}")
            }
            NotEqual { register, lhs, rhs } => {
                write!(f, "NotEqual\tresult: {register}\tlhs: {lhs}\t\trhs: {rhs}")
            }
            Jump { offset } => write!(f, "Jump\t\toffset: {offset}"),
            JumpBack { offset } => write!(f, "JumpBack\toffset: {offset}"),
            JumpIfTrue { register, offset } => {
                write!(f, "JumpIfTrue\tresult: {register}\toffset: {offset}")
            }
            JumpIfFalse { register, offset } => {
                write!(f, "JumpIfFalse\tresult: {register}\toffset: {offset}")
            }
            Call {
                result,
                function,
                frame_base,
                arg_count,
            } => write!(
                f,
                "Call\t\tresult: {result}\tfunction: {function}\t\
                 frame base: {frame_base}\targs: {arg_count}",
            ),
            CallInstance {
                result,
                function,
                frame_base,
                arg_count,
                instance,
            } => write!(
                f,
                "CallInstance\tresult: {result}\tfunction: {function}\tframe_base: {frame_base}
                 \t\t\targs: {arg_count}\t\tinstance: {instance}",
            ),
            Return { register } => write!(f, "Return\t\tresult: {register}"),
            Yield { register } => write!(f, "Yield\t\tresult: {register}"),
            Throw { register } => write!(f, "Throw\t\tresult: {register}"),
            Size { register, value } => write!(f, "Size\t\tresult: {register}\tvalue: {value}"),
            IterNext {
                register,
                iterator,
                jump_offset,
            } => write!(
                f,
                "IterNext\tresult: {register}\titerator: {iterator}\tjump offset: {jump_offset}",
            ),
            IterNextTemp {
                register,
                iterator,
                jump_offset,
            } => write!(
                f,
                "IterNextTemp\tresult: {register}\
                 \titerator: {iterator}\tjump offset: {jump_offset}",
            ),
            IterNextQuiet {
                iterator,
                jump_offset,
            } => write!(
                f,
                "IterNextQuiet\titerator: {iterator}\tjump offset: {jump_offset}",
            ),
            TempIndex {
                register,
                value,
                index,
            } => write!(
                f,
                "TempIndex\tresult: {register}\tvalue: {value}\tindex: {index}",
            ),
            SliceFrom {
                register,
                value,
                index,
            } => write!(
                f,
                "SliceFrom\tresult: {register}\tvalue: {value}\tindex: {index}",
            ),
            SliceTo {
                register,
                value,
                index,
            } => write!(
                f,
                "SliceTo\t\tresult: {register}\tvalue: {value}\tindex: {index}"
            ),
            IsTuple { register, value } => {
                write!(f, "IsTuple\t\tresult: {register}\tvalue: {value}")
            }
            IsList { register, value } => {
                write!(f, "IsList\t\tresult: {register}\tvalue: {value}")
            }
            Index {
                register,
                value,
                index,
            } => write!(
                f,
                "Index\t\tresult: {register}\tvalue: {value}\tindex: {index}"
            ),
            SetIndex {
                register,
                index,
                value,
            } => write!(
                f,
                "SetIndex\tregister: {register}\tindex: {index}\tvalue: {value}"
            ),
            MapInsert {
                register,
                value,
                key,
            } => write!(
                f,
                "MapInsert\tmap: {register}\t\tvalue: {value}\tkey: {key}"
            ),
            MetaInsert {
                register,
                value,
                id,
            } => write!(
                f,
                "MetaInsert\tmap: {register}\t\tid: {id:?}\tvalue: {value}",
            ),
            MetaInsertNamed {
                register,
                id,
                name,
                value,
            } => write!(
                f,
                "MetaInsertNamed\tmap: {register}\t\tid: {id:?}\tname: {name}\t\tvalue: {value}",
            ),
            MetaExport { id, value } => write!(f, "MetaExport\tid: {id:?}\tvalue: {value}"),
            MetaExportNamed { id, name, value } => write!(
                f,
                "MetaExportNamed\tid: {id:?}\tname: {name}\tvalue: {value}",
            ),
            Access {
                register,
                value,
                key,
            } => write!(
                f,
                "Access\t\tresult: {register}\tvalue: {value}\tkey: {key}"
            ),
            AccessString {
                register,
                value,
                key,
            } => write!(
                f,
                "AccessString\tresult: {register}\tvalue: {value}\tkey: {key}"
            ),
            TryStart {
                arg_register,
                catch_offset,
            } => write!(
                f,
                "TryStart\targ register: {arg_register}\tcatch offset: {catch_offset}",
            ),
            TryEnd => write!(f, "TryEnd"),
            Debug { register, constant } => {
                write!(f, "Debug\t\tregister: {register}\tconstant: {constant}")
            }
            CheckType { register, type_id } => {
                write!(f, "CheckType\tregister: {register}\ttype: {type_id:?}")
            }
            CheckSize { register, size } => {
                write!(f, "CheckSize\tregister: {register}\tsize: {size}")
            }
            StringStart {
                register,
                size_hint,
            } => {
                write!(
                    f,
                    "StringStart\tregister: {register}\tsize hint: {size_hint}"
                )
            }
            StringPush { register, value } => {
                write!(f, "StringPush\tregister: {register}\tvalue: {value}")
            }
            StringFinish { register } => {
                write!(f, "StringFinish\tregister: {register}")
            }
        }
    }
}

/// An iterator that converts bytecode into a series of [Instruction]s
#[derive(Clone, Default)]
pub struct InstructionReader {
    /// The chunk that the reader is reading from
    pub chunk: Rc<Chunk>,
    /// The reader's instruction pointer
    pub ip: usize,
}

impl InstructionReader {
    /// Initializes a reader with the given chunk
    pub fn new(chunk: Rc<Chunk>) -> Self {
        Self { chunk, ip: 0 }
    }
}

impl Iterator for InstructionReader {
    type Item = Instruction;

    fn next(&mut self) -> Option<Self::Item> {
        use Instruction::*;

        macro_rules! get_u8 {
            () => {{
                #[cfg(not(debug_assertions))]
                {
                    let byte = unsafe { self.chunk.bytes.get_unchecked(self.ip) };
                    self.ip += 1;
                    *byte
                }

                #[cfg(debug_assertions)]
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
                #[cfg(not(debug_assertions))]
                {
                    let bytes = unsafe { self.chunk.bytes.get_unchecked(self.ip..self.ip + 2) };
                    self.ip += 2;
                    u16::from_le_bytes(bytes.try_into().unwrap())
                }

                #[cfg(debug_assertions)]
                {
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
                }
            }};
        }

        macro_rules! get_u32 {
            () => {{
                #[cfg(not(debug_assertions))]
                {
                    let bytes = unsafe { self.chunk.bytes.get_unchecked(self.ip..self.ip + 4) };
                    self.ip += 4;
                    u32::from_le_bytes(bytes.try_into().unwrap())
                }

                #[cfg(debug_assertions)]
                {
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
                }
            }};
        }

        macro_rules! get_u16_constant {
            () => {{
                #[cfg(not(debug_assertions))]
                {
                    let bytes = unsafe { self.chunk.bytes.get_unchecked(self.ip..self.ip + 2) };
                    self.ip += 2;
                    let bytes: [u8; 2] = bytes.try_into().unwrap();
                    ConstantIndex::from(bytes)
                }

                #[cfg(debug_assertions)]
                {
                    match self.chunk.bytes.get(self.ip..self.ip + 2) {
                        Some(bytes) => {
                            self.ip += 2;
                            let bytes: [u8; 2] = bytes.try_into().unwrap();
                            ConstantIndex::from(bytes)
                        }
                        None => {
                            return Some(Error {
                                message: format!("Expected 2 bytes at position {}", self.ip),
                            });
                        }
                    }
                }
            }};
        }

        macro_rules! get_u24_constant {
            () => {{
                #[cfg(not(debug_assertions))]
                {
                    let bytes = unsafe { self.chunk.bytes.get_unchecked(self.ip..self.ip + 3) };
                    self.ip += 3;
                    let bytes: [u8; 3] = bytes.try_into().unwrap();
                    ConstantIndex::from(bytes)
                }

                #[cfg(debug_assertions)]
                {
                    match self.chunk.bytes.get(self.ip..self.ip + 3) {
                        Some(bytes) => {
                            self.ip += 3;
                            let bytes: [u8; 3] = bytes.try_into().unwrap();
                            ConstantIndex::from(bytes)
                        }
                        None => {
                            return Some(Error {
                                message: format!("Expected 3 bytes at position {}", self.ip),
                            });
                        }
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
            Op::SetNull => Some(SetNull {
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
                constant: ConstantIndex::from(get_u8!()),
            }),
            Op::LoadFloat16 => Some(LoadFloat {
                register: get_u8!(),
                constant: get_u16_constant!(),
            }),
            Op::LoadFloat24 => Some(LoadFloat {
                register: get_u8!(),
                constant: get_u24_constant!(),
            }),
            Op::LoadInt => Some(LoadInt {
                register: get_u8!(),
                constant: ConstantIndex::from(get_u8!()),
            }),
            Op::LoadInt16 => Some(LoadInt {
                register: get_u8!(),
                constant: get_u16_constant!(),
            }),
            Op::LoadInt24 => Some(LoadInt {
                register: get_u8!(),
                constant: get_u24_constant!(),
            }),
            Op::LoadString => Some(LoadString {
                register: get_u8!(),
                constant: ConstantIndex::from(get_u8!()),
            }),
            Op::LoadString16 => Some(LoadString {
                register: get_u8!(),
                constant: get_u16_constant!(),
            }),
            Op::LoadString24 => Some(LoadString {
                register: get_u8!(),
                constant: get_u24_constant!(),
            }),
            Op::LoadNonLocal => Some(LoadNonLocal {
                register: get_u8!(),
                constant: ConstantIndex::from(get_u8!()),
            }),
            Op::LoadNonLocal16 => Some(LoadNonLocal {
                register: get_u8!(),
                constant: get_u16_constant!(),
            }),
            Op::LoadNonLocal24 => Some(LoadNonLocal {
                register: get_u8!(),
                constant: get_u24_constant!(),
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
            Op::TempTupleToTuple => Some(TempTupleToTuple {
                register: get_u8!(),
                source: get_u8!(),
            }),
            Op::MakeMap => Some(MakeMap {
                register: get_u8!(),
                size_hint: get_u8!() as usize,
            }),
            Op::MakeMap32 => Some(MakeMap {
                register: get_u8!(),
                size_hint: get_u32!() as usize,
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
                    arg_is_unpacked_tuple: flags.arg_is_unpacked_tuple,
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
                value: get_u8!(),
            }),
            Op::Not => Some(Not {
                register: get_u8!(),
                value: get_u8!(),
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
            Op::Remainder => Some(Remainder {
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
            Op::JumpBack => Some(JumpBack {
                offset: get_u16!() as usize,
            }),
            Op::JumpIfTrue => Some(JumpIfTrue {
                register: get_u8!(),
                offset: get_u16!() as usize,
            }),
            Op::JumpIfFalse => Some(JumpIfFalse {
                register: get_u8!(),
                offset: get_u16!() as usize,
            }),
            Op::Call => Some(Call {
                result: get_u8!(),
                function: get_u8!(),
                frame_base: get_u8!(),
                arg_count: get_u8!(),
            }),
            Op::CallInstance => Some(CallInstance {
                result: get_u8!(),
                function: get_u8!(),
                frame_base: get_u8!(),
                arg_count: get_u8!(),
                instance: get_u8!(),
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
            Op::TempIndex => Some(TempIndex {
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
                            "Unexpected meta id {meta_id} found at instruction {op_ip}",
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
                            "Unexpected meta id {meta_id} found at instruction {op_ip}",
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
                            "Unexpected meta id {meta_id} found at instruction {op_ip}",
                        ),
                    })
                }
            }
            Op::Access => Some(Access {
                register: get_u8!(),
                value: get_u8!(),
                key: ConstantIndex::from(get_u8!()),
            }),
            Op::Access16 => Some(Access {
                register: get_u8!(),
                value: get_u8!(),
                key: get_u16_constant!(),
            }),
            Op::Access24 => Some(Access {
                register: get_u8!(),
                value: get_u8!(),
                key: get_u24_constant!(),
            }),
            Op::AccessString => Some(AccessString {
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
                constant: get_u24_constant!(),
            }),
            Op::CheckType => {
                let register = get_u8!();
                match TypeId::from_byte(get_u8!()) {
                    Ok(type_id) => Some(CheckType { register, type_id }),
                    Err(byte) => Some(Error {
                        message: format!("Unexpected value for CheckType id: {byte}"),
                    }),
                }
            }
            Op::CheckSize => Some(CheckSize {
                register: get_u8!(),
                size: get_u8!() as usize,
            }),
            Op::StringStart => Some(StringStart {
                register: get_u8!(),
                size_hint: get_u8!() as usize,
            }),
            Op::StringStart32 => Some(StringStart {
                register: get_u8!(),
                size_hint: get_u32!() as usize,
            }),
            Op::StringPush => Some(StringPush {
                register: get_u8!(),
                value: get_u8!(),
            }),
            Op::StringFinish => Some(StringFinish {
                register: get_u8!(),
            }),
            _ => Some(Error {
                message: format!("Unexpected opcode {op:?} found at instruction {op_ip}"),
            }),
        }
    }
}
