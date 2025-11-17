use std::fmt;

use koto_parser::{ConstantIndex, MetaKeyId, StringAlignment, StringFormatOptions};

/// Decoded instructions produced by an [InstructionReader](crate::InstructionReader) for execution
/// in the runtime
///
/// For descriptions of each instruction's purpose, see corresponding [Op](crate::Op) entries.
#[allow(missing_docs)]
pub enum Instruction {
    Error {
        message: String,
    },
    NewFrame {
        register_count: u8,
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
    ExportValue {
        key: u8,
        value: u8,
    },
    ExportEntry {
        entry: u8,
    },
    Import {
        register: u8,
    },
    ImportAll {
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
        size_hint: u32,
    },
    SequenceStart {
        size_hint: u32,
    },
    SequencePush {
        value: u8,
    },
    SequencePushN {
        start: u8,
        count: u8,
    },
    SequenceToList {
        register: u8,
    },
    SequenceToTuple {
        register: u8,
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
    Function {
        register: u8,
        arg_count: u8,
        optional_arg_count: u8,
        capture_count: u8,
        flags: FunctionFlags,
        size: u16,
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
    Power {
        register: u8,
        lhs: u8,
        rhs: u8,
    },
    AddAssign {
        lhs: u8,
        rhs: u8,
    },
    SubtractAssign {
        lhs: u8,
        rhs: u8,
    },
    MultiplyAssign {
        lhs: u8,
        rhs: u8,
    },
    DivideAssign {
        lhs: u8,
        rhs: u8,
    },
    RemainderAssign {
        lhs: u8,
        rhs: u8,
    },
    PowerAssign {
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
        offset: u16,
    },
    JumpBack {
        offset: u16,
    },
    JumpIfTrue {
        register: u8,
        offset: u16,
    },
    JumpIfFalse {
        register: u8,
        offset: u16,
    },
    JumpIfNull {
        register: u8,
        offset: u16,
    },
    Call {
        result: u8,
        function: u8,
        frame_base: u8,
        arg_count: u8,
        packed_arg_count: u8,
    },
    CallInstance {
        result: u8,
        function: u8,
        instance: u8,
        frame_base: u8,
        arg_count: u8,
        packed_arg_count: u8,
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
        result: Option<u8>,
        iterator: u8,
        jump_offset: u16,
        temporary_output: bool,
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
    Index {
        register: u8,
        value: u8,
        index: u8,
    },
    IndexMut {
        register: u8,
        index: u8,
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
    TryAccess {
        register: u8,
        value: u8,
        key: ConstantIndex,
        jump_offset: u16,
    },
    AccessString {
        register: u8,
        value: u8,
        key: u8,
    },
    TryAccessString {
        register: u8,
        value: u8,
        key: u8,
        jump_offset: u16,
    },
    AccessAssign {
        register: u8,
        key: u8,
        value: u8,
    },
    TryStart {
        arg_register: u8,
        catch_offset: u16,
    },
    TryEnd,
    Debug {
        register: u8,
        constant: ConstantIndex,
    },
    CheckSizeEqual {
        register: u8,
        size: usize,
    },
    CheckSizeMin {
        register: u8,
        size: usize,
    },
    AssertType {
        value: u8,
        allow_null: bool,
        type_string: ConstantIndex,
    },
    CheckType {
        value: u8,
        allow_null: bool,
        type_string: ConstantIndex,
        jump_offset: u16,
    },
    StringStart {
        size_hint: u32,
    },
    StringPush {
        value: u8,
        format_options: Option<StringFormatOptions>,
    },
    StringFinish {
        register: u8,
    },
}

/// Flags used to define the properties of a Function
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct FunctionFlags(u8);

impl FunctionFlags {
    const VARIADIC: u8 = 1 << 0;
    const GENERATOR: u8 = 1 << 1;
    const ARG_IS_UNPACKED_TUPLE: u8 = 1 << 2;
    const NON_LOCAL_ACCESS: u8 = 1 << 3;

    /// Returns a new [FunctionFlags] with the given flags set
    pub fn new(
        variadic: bool,
        generator: bool,
        arg_is_unpacked_tuple: bool,
        non_local_access: bool,
    ) -> Self {
        let mut flags = 0;
        if variadic {
            flags |= Self::VARIADIC;
        }
        if generator {
            flags |= Self::GENERATOR;
        }
        if arg_is_unpacked_tuple {
            flags |= Self::ARG_IS_UNPACKED_TUPLE;
        }
        if non_local_access {
            flags |= Self::NON_LOCAL_ACCESS;
        }
        Self(flags)
    }

    /// True if the function has a variadic argument
    ///
    /// If the function is variadic, then extra args will be captured in a tuple.
    pub fn is_variadic(self) -> bool {
        self.0 & Self::VARIADIC != 0
    }

    /// True if the function is a generator
    ///
    /// If the function is a generator, then calling the function will yield an iterator that
    /// executes the function's body for each iteration step, pausing when a yield instruction is
    /// encountered.
    pub fn is_generator(self) -> bool {
        self.0 & Self::GENERATOR != 0
    }

    /// True if the function has a single argument which is an unpacked tuple
    ///
    /// This is used to optimize calls where the caller has a series of args that might be unpacked
    /// by the function, and it would be wasteful to create a Tuple when it's going to be
    /// immediately unpacked and discarded.
    pub fn arg_is_unpacked_tuple(self) -> bool {
        self.0 & Self::ARG_IS_UNPACKED_TUPLE != 0
    }

    /// True if the function accesses a non-local value
    ///
    /// Functions that access a non-local value need to carry module exports and wildcard imports
    /// with them, if no non-locals are accessed then the creation of the non-local context can be
    /// skipped.
    pub fn non_local_access(self) -> bool {
        self.0 & Self::NON_LOCAL_ACCESS != 0
    }
}

impl TryFrom<u8> for FunctionFlags {
    type Error = String;

    fn try_from(byte: u8) -> Result<Self, Self::Error> {
        if byte <= 0b1111 {
            Ok(Self(byte))
        } else {
            Err(format!("Invalid function flags: {byte:#010b}"))
        }
    }
}

impl From<FunctionFlags> for u8 {
    fn from(value: FunctionFlags) -> Self {
        value.0
    }
}

/// Format flags used by the [StringPush][crate::Op::StringPush] op
///
/// This is the bytecode counterpart to [koto_parser::StringFormatOptions].
#[derive(Clone, Copy, Default)]
#[repr(transparent)]
pub struct StringFormatFlags(u8);

impl StringFormatFlags {
    /// Set to true when a minimum width is defined
    pub const MIN_WIDTH: u8 = 1 << 2; // The first two bits correspond to values of StringAlignment
    /// Set to true when precision is defined
    pub const PRECISION: u8 = 1 << 3;
    /// Set to true when a fill character is defined
    pub const FILL_CHARACTER: u8 = 1 << 4;
    /// Set to true when a format style is defined
    pub const REPRESENTATION: u8 = 1 << 5;

    /// Returns the flag's string alignment
    pub fn alignment(&self) -> StringAlignment {
        use StringAlignment as Align;
        let bits = self.0 & 0b11;
        if bits == Align::Default as u8 {
            Align::Default
        } else if bits == Align::Left as u8 {
            Align::Left
        } else if bits == Align::Center as u8 {
            Align::Center
        } else {
            Align::Right
        }
    }

    /// True if a minimum width has been defined
    pub fn has_min_width(&self) -> bool {
        self.0 & Self::MIN_WIDTH != 0
    }

    /// True if a precision has been defined
    pub fn has_precision(&self) -> bool {
        self.0 & Self::PRECISION != 0
    }

    /// True if a fill character has been defined
    pub fn has_fill_character(&self) -> bool {
        self.0 & Self::FILL_CHARACTER != 0
    }

    /// True if an alternative representation has been defined
    pub fn has_representation(&self) -> bool {
        self.0 & Self::REPRESENTATION != 0
    }
}

impl From<StringFormatOptions> for StringFormatFlags {
    fn from(value: StringFormatOptions) -> Self {
        let mut flags = value.alignment as u8;

        if value.min_width.is_some() {
            flags |= Self::MIN_WIDTH;
        }
        if value.precision.is_some() {
            flags |= Self::PRECISION;
        }
        if value.fill_character.is_some() {
            flags |= Self::FILL_CHARACTER;
        }
        if value.representation.is_some() {
            flags |= Self::REPRESENTATION;
        }

        Self(flags)
    }
}

impl TryFrom<u8> for StringFormatFlags {
    type Error = String;

    fn try_from(byte: u8) -> Result<Self, Self::Error> {
        if byte <= 0b1111111 {
            Ok(Self(byte))
        } else {
            Err(format!("Invalid string format flags: {byte:#010b}"))
        }
    }
}

impl From<StringFormatFlags> for u8 {
    fn from(value: StringFormatFlags) -> Self {
        value.0
    }
}

impl fmt::Debug for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Instruction::*;
        match self {
            Error { message } => unreachable!("{message}"),
            NewFrame { register_count } => {
                write!(f, "NewFrame        registers: {register_count}")
            }
            Copy { target, source } => {
                write!(f, "Copy            result: {target:<7} source: {source}")
            }
            SetNull { register } => write!(f, "SetNull         result: {register}"),
            SetBool { register, value } => {
                write!(f, "SetBool         result: {register:<7} value: {value}")
            }
            SetNumber { register, value } => {
                write!(f, "SetNumber       result: {register:<7} value: {value}")
            }
            LoadFloat { register, constant } => {
                write!(
                    f,
                    "LoadFloat       result: {register:<7} constant: {constant}"
                )
            }
            LoadInt { register, constant } => {
                write!(
                    f,
                    "LoadInt         result: {register:<7} constant: {constant}"
                )
            }
            LoadString { register, constant } => {
                write!(
                    f,
                    "LoadString      result: {register:<7} constant: {constant}"
                )
            }
            LoadNonLocal { register, constant } => {
                write!(
                    f,
                    "LoadNonLocal    result: {register:<7} constant: {constant}"
                )
            }
            ExportValue { key, value } => {
                write!(f, "ExportValue     key: {key:<10} value: {value}")
            }
            ExportEntry { entry } => write!(f, "ExportEntry     entry: {entry}"),
            Import { register } => write!(f, "Import          register: {register}"),
            ImportAll { register } => write!(f, "ImportAll       register: {register}"),
            MakeTempTuple {
                register,
                start,
                count,
            } => write!(
                f,
                "MakeTempTuple   result: {register:<7} start: {start:<8} count: {count}"
            ),
            TempTupleToTuple { register, source } => {
                write!(f, "TempTupleToTuple result: {register:<7} source: {source}")
            }
            MakeMap {
                register,
                size_hint,
            } => write!(
                f,
                "MakeMap         result: {register:<7} size_hint: {size_hint}"
            ),
            SequenceStart { size_hint } => write!(f, "SequenceStart   size_hint: {size_hint}"),
            SequencePush { value } => {
                write!(f, "SequencePush    value: {value}")
            }
            SequencePushN { start, count } => {
                write!(f, "SequencePushN   start: {start:<8} count: {count}",)
            }
            SequenceToList { register } => write!(f, "SequenceToList  result: {register}"),
            SequenceToTuple { register } => write!(f, "SequenceToTuple result: {register}"),
            Range {
                register,
                start,
                end,
            } => write!(
                f,
                "Range           result: {register:<7} start: {start:<8} end: {end}",
            ),
            RangeInclusive {
                register,
                start,
                end,
            } => write!(
                f,
                "RangeInclusive  result: {register:<7} start: {start:<8} end: {end}",
            ),
            RangeTo { register, end } => write!(f, "RangeTo result: {register:<7} end: {end}"),
            RangeToInclusive { register, end } => {
                write!(f, "RangeToInclu... result: {register:<7} end: {end}")
            }
            RangeFrom { register, start } => {
                write!(f, "RangeFrom       result: {register:<7} start: {start}")
            }
            RangeFull { register } => write!(f, "RangeFull     result: {register}"),
            MakeIterator { register, iterable } => {
                write!(
                    f,
                    "MakeIterator    result: {register:<7} iterable: {iterable}",
                )
            }
            Function {
                register,
                arg_count,
                optional_arg_count,
                capture_count,
                flags,
                size,
            } => write!(
                f,
                "Function        \
                result: {register:<7} args: {arg_count:<9} \
                optional: {optional_arg_count:<5} captures: {capture_count}
                size: {size:<9} flags: {:<#05b}",
                u8::from(*flags)
            ),
            Capture {
                function,
                target,
                source,
            } => write!(
                f,
                "Capture         function: {function:<5} target: {target:<7} source: {source}",
            ),
            Negate { register, value } => {
                write!(f, "Negate          result: {register:<7} source: {value}")
            }
            Not { register, value } => {
                write!(f, "Not             result: {register:<7} source: {value}")
            }
            Add { register, lhs, rhs } => {
                write!(
                    f,
                    "Add             result: {register:<7} lhs: {lhs:<10} rhs: {rhs}"
                )
            }
            Subtract { register, lhs, rhs } => {
                write!(
                    f,
                    "Subtract        result: {register:<7} lhs: {lhs:<10} rhs: {rhs}"
                )
            }
            Multiply { register, lhs, rhs } => {
                write!(
                    f,
                    "Multiply        result: {register:<7} lhs: {lhs:<10} rhs: {rhs}"
                )
            }
            Divide { register, lhs, rhs } => {
                write!(
                    f,
                    "Divide          result: {register:<7} lhs: {lhs:<10} rhs: {rhs}"
                )
            }
            Remainder { register, lhs, rhs } => {
                write!(
                    f,
                    "Remainder       result: {register:<7} lhs: {lhs:<10} rhs: {rhs}"
                )
            }
            Power { register, lhs, rhs } => {
                write!(
                    f,
                    "Power           result: {register:<7} lhs: {lhs:<10} rhs: {rhs}"
                )
            }
            AddAssign { lhs, rhs } => {
                write!(f, "AddAssign       lhs: {lhs:<10} rhs: {rhs}")
            }
            SubtractAssign { lhs, rhs } => {
                write!(f, "SubAssign       lhs: {lhs:<10} rhs: {rhs}")
            }
            MultiplyAssign { lhs, rhs } => {
                write!(f, "MulAssign       lhs: {lhs:<10} rhs: {rhs}")
            }
            DivideAssign { lhs, rhs } => {
                write!(f, "DivAssign       lhs: {lhs:<10} rhs: {rhs}")
            }
            RemainderAssign { lhs, rhs } => {
                write!(f, "RemAssign       lhs: {lhs:<10} rhs: {rhs}")
            }
            PowerAssign { lhs, rhs } => {
                write!(f, "PowAssign       lhs: {lhs:<10} rhs: {rhs}")
            }
            Less { register, lhs, rhs } => {
                write!(
                    f,
                    "Less            result: {register:<7} lhs: {lhs:<10} rhs: {rhs}"
                )
            }
            LessOrEqual { register, lhs, rhs } => write!(
                f,
                "LessOrEqual     result: {register:<7} lhs: {lhs:<10} rhs: {rhs}",
            ),
            Greater { register, lhs, rhs } => {
                write!(
                    f,
                    "Greater         result: {register:<7} lhs: {lhs:<10} rhs: {rhs}"
                )
            }
            GreaterOrEqual { register, lhs, rhs } => write!(
                f,
                "GreaterOrEqual  result: {register:<7} lhs: {lhs:<10} rhs: {rhs}",
            ),
            Equal { register, lhs, rhs } => {
                write!(
                    f,
                    "Equal           result: {register:<7} lhs: {lhs:<10} rhs: {rhs}"
                )
            }
            NotEqual { register, lhs, rhs } => {
                write!(
                    f,
                    "NotEqual        result: {register:<7} lhs: {lhs:<10} rhs: {rhs}"
                )
            }
            Jump { offset } => write!(f, "Jump            offset: {offset}"),
            JumpBack { offset } => write!(f, "JumpBack        offset: {offset}"),
            JumpIfTrue { register, offset } => {
                write!(
                    f,
                    "JumpIfTrue      register: {register:<5} offset: {offset}"
                )
            }
            JumpIfFalse { register, offset } => {
                write!(
                    f,
                    "JumpIfFalse     register: {register:<5} offset: {offset}"
                )
            }
            JumpIfNull { register, offset } => {
                write!(
                    f,
                    "JumpIfNull      register: {register:<5} offset: {offset}"
                )
            }
            Call {
                result,
                function,
                frame_base,
                arg_count,
                packed_arg_count,
            } => write!(
                f,
                "Call            \
                 result: {result:<7} function: {function:<5} \
                 frame base: {frame_base}
                args: {arg_count:<9} packed args: {packed_arg_count}",
            ),
            CallInstance {
                result,
                function,
                instance,
                frame_base,
                arg_count,
                packed_arg_count,
            } => write!(
                f,
                "CallInstance    \
                result: {result:<7} function: {function:<5} \
                frame base: {frame_base:<3} args: {arg_count}
                instance: {instance:<5} packed args: {packed_arg_count}",
            ),
            Return { register } => write!(f, "Return          register: {register}"),
            Yield { register } => write!(f, "Yield           register: {register}"),
            Throw { register } => write!(f, "Throw           register: {register}"),
            Size { register, value } => {
                write!(f, "Size            result: {register:<7} value: {value}")
            }
            IterNext {
                result,
                iterator,
                jump_offset,
                temporary_output,
            } => write!(
                f,
                "IterNext       {} iterator: {iterator:<6}\
                jump: {jump_offset:<9} temp: {temporary_output}",
                result.map_or(String::new(), |result| format!(" result: {result:<7}")),
            ),
            TempIndex {
                register,
                value,
                index,
            } => write!(
                f,
                "TempIndex       result: {register:<7} value: {value:<8} index: {index}",
            ),
            SliceFrom {
                register,
                value,
                index,
            } => write!(
                f,
                "SliceFrom       result: {register:<7} value: {value:<8} index: {index}",
            ),
            SliceTo {
                register,
                value,
                index,
            } => write!(
                f,
                "SliceTo         result: {register:<7} value: {value:<8} index: {index}"
            ),
            Index {
                register,
                value,
                index,
            } => write!(
                f,
                "Index           result: {register:<7} value: {value:<8} index: {index}"
            ),
            IndexMut {
                register,
                index,
                value,
            } => write!(
                f,
                "IndexMut        register: {register:<5} index: {index:<8} value: {value}"
            ),
            AccessAssign {
                register,
                value,
                key,
            } => write!(
                f,
                "AccessAssign    register: {register:<5} value: {value:<8} key: {key}"
            ),
            MetaInsert {
                register,
                value,
                id,
            } => write!(
                f,
                "MetaInsert      map: {register:<10} id: {:<11} value: {value}",
                format!("{id}")
            ),
            MetaInsertNamed {
                register,
                id,
                name,
                value,
            } => write!(
                f,
                "MetaInsertNamed map: {register:<10} id: {:<11} name: {name:<9} value: {value}",
                format!("{id}")
            ),
            MetaExport { id, value } => {
                write!(
                    f,
                    "MetaExport      id: {:<11} value: {value}",
                    format!("{id}")
                )
            }
            MetaExportNamed { id, name, value } => write!(
                f,
                "MetaExportNamed id: {:<11} name: {name:<9} value: {value}",
                format!("{id}")
            ),
            Access {
                register,
                value,
                key,
            } => write!(
                f,
                "Access          result: {register:<7} source: {value:<7} key: {key}"
            ),
            TryAccess {
                register,
                value,
                key,
                jump_offset,
            } => write!(
                f,
                "TryAccess       result: {register:<7} source: {value:<7} key: {key} offset: {jump_offset}"
            ),
            AccessString {
                register,
                value,
                key,
            } => write!(
                f,
                "AccessString    result: {register:<7} source: {value:<7} key: {key}"
            ),
            TryAccessString {
                register,
                value,
                key,
                jump_offset,
            } => write!(
                f,
                "TryAccessString result: {register:<7} source: {value:<7} key: {key} offset: {jump_offset}"
            ),
            TryStart {
                arg_register,
                catch_offset,
            } => write!(
                f,
                "TryStart        arg register: {arg_register:<5} catch offset: {catch_offset}",
            ),
            TryEnd => write!(f, "TryEnd"),
            Debug { register, constant } => {
                write!(
                    f,
                    "Debug           value: {register:<8} constant: {constant}"
                )
            }
            CheckSizeEqual { register, size } => {
                write!(f, "CheckSizeEqual  value: {register:<8} size: {size}")
            }
            CheckSizeMin { register, size } => {
                write!(f, "CheckSizeMin    value: {register:<8} size: {size}")
            }
            AssertType {
                value,
                type_string,
                allow_null,
            } => {
                write!(
                    f,
                    "AssertType      value: {value:<8} type: {type_string:<9} \
                     allow null: {allow_null}"
                )
            }
            CheckType {
                value,
                jump_offset,
                type_string,
                allow_null,
            } => {
                write!(
                    f,
                    "CheckType       value: {value:<7} type: {type_string:<8} \
                    allow null: {allow_null:<6} offset: {jump_offset}"
                )
            }
            StringStart { size_hint } => {
                write!(f, "StringStart     size hint: {size_hint}")
            }
            StringPush {
                value,
                format_options,
            } => {
                write!(f, "StringPush      value: {value:<8}")?;
                if let Some(opts) = format_options {
                    write!(f, " align: {:<8}", format!("{:?}", opts.alignment))?;
                    if let Some(min_width) = opts.min_width {
                        write!(f, " min_width: {min_width:<4}")?;
                    }
                    if let Some(precision) = opts.precision {
                        write!(f, " precision: {precision:<4}")?;
                    }
                    if let Some(fill_character) = opts.fill_character {
                        write!(f, " fill_character: {fill_character}")?;
                    }
                }
                Ok(())
            }
            StringFinish { register } => {
                write!(f, "StringFinish    result: {register:<7}")
            }
        }
    }
}
