use {
    crate::{Chunk, FunctionFlags, Op},
    koto_parser::ConstantIndex,
    std::{convert::TryInto, fmt, sync::Arc},
};

pub enum Instruction {
    Error {
        message: String,
    },
    Copy {
        target: u8,
        source: u8,
    },
    DeepCopy {
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
        value: f64,
    },
    LoadNumber {
        register: u8,
        constant: ConstantIndex,
    },
    LoadString {
        register: u8,
        constant: ConstantIndex,
    },
    LoadGlobal {
        register: u8,
        constant: ConstantIndex,
    },
    SetGlobal {
        global: ConstantIndex,
        source: u8,
    },
    Import {
        register: u8,
        constant: ConstantIndex,
    },
    MakeTuple {
        register: u8,
        start: u8,
        count: u8,
    },
    MakeTempTuple {
        register: u8,
        start: u8,
        count: u8,
    },
    MakeList {
        register: u8,
        size_hint: usize,
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
    LoadCapture {
        register: u8,
        capture: u8,
    },
    SetCapture {
        capture: u8,
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
    Size {
        register: u8,
        value: u8,
    },
    Type {
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
    ListPushValue {
        list: u8,
        value: u8,
    },
    ListPushValues {
        list: u8,
        values_start: u8,
        count: u8,
    },
    ListUpdate {
        list: u8,
        index: u8,
        value: u8,
    },
    Index {
        register: u8,
        value: u8,
        index: u8,
    },
    MapInsert {
        register: u8,
        value: u8,
        key: ConstantIndex,
    },
    MapAccess {
        register: u8,
        map: u8,
        key: ConstantIndex,
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
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Instruction::*;
        match self {
            Error { message } => unreachable!(message),
            Copy { .. } => write!(f, "Copy"),
            DeepCopy { .. } => write!(f, "DeepCopy"),
            SetEmpty { .. } => write!(f, "SetEmpty"),
            SetBool { .. } => write!(f, "SetBool"),
            SetNumber { .. } => write!(f, "SetNumber"),
            LoadNumber { .. } => write!(f, "LoadNumber"),
            LoadString { .. } => write!(f, "LoadString"),
            LoadGlobal { .. } => write!(f, "LoadGlobal"),
            SetGlobal { .. } => write!(f, "SetGlobal"),
            Import { .. } => write!(f, "Import"),
            MakeTuple { .. } => write!(f, "MakeTuple"),
            MakeTempTuple { .. } => write!(f, "MakeTempTuple"),
            MakeList { .. } => write!(f, "MakeList"),
            MakeMap { .. } => write!(f, "MakeMap"),
            MakeNum2 { .. } => write!(f, "MakeNum2"),
            MakeNum4 { .. } => write!(f, "MakeNum4"),
            Range { .. } => write!(f, "Range"),
            RangeInclusive { .. } => write!(f, "RangeInclusive"),
            RangeTo { .. } => write!(f, "RangeTo"),
            RangeToInclusive { .. } => write!(f, "RangeToInclusive"),
            RangeFrom { .. } => write!(f, "RangeFrom"),
            RangeFull { .. } => write!(f, "RangeFull"),
            MakeIterator { .. } => write!(f, "MakeIterator"),
            Function { .. } => write!(f, "Function"),
            Capture { .. } => write!(f, "Capture"),
            LoadCapture { .. } => write!(f, "LoadCapture"),
            SetCapture { .. } => write!(f, "SetCapture"),
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
            Size { .. } => write!(f, "Size"),
            Type { .. } => write!(f, "Type"),
            IterNext { .. } => write!(f, "IterNext"),
            IterNextTemp { .. } => write!(f, "IterNextTemp"),
            IterNextQuiet { .. } => write!(f, "IterNextQuiet"),
            ValueIndex { .. } => write!(f, "ValueIndex"),
            SliceFrom { .. } => write!(f, "SliceFrom"),
            SliceTo { .. } => write!(f, "SliceTo"),
            IsTuple { .. } => write!(f, "IsTuple"),
            IsList { .. } => write!(f, "IsList"),
            ListPushValue { .. } => write!(f, "ListPushValue"),
            ListPushValues { .. } => write!(f, "ListPushValues"),
            ListUpdate { .. } => write!(f, "ListUpdate"),
            Index { .. } => write!(f, "Index"),
            MapInsert { .. } => write!(f, "MapInsert"),
            MapAccess { .. } => write!(f, "MapAccess"),
            TryStart { .. } => write!(f, "TryStart"),
            TryEnd => write!(f, "TryEnd"),
            Debug { .. } => write!(f, "Debug"),
        }
    }
}

impl fmt::Debug for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Instruction::*;
        match self {
            Error { message } => unreachable!(message),
            Copy { target, source } => write!(f, "Copy\t\tresult: {}\tsource: {}", target, source),
            DeepCopy { target, source } => {
                write!(f, "DeepCopy\tresult: {}\tsource: {}", target, source)
            }
            SetEmpty { register } => write!(f, "SetEmpty\tresult: {}", register),
            SetBool { register, value } => {
                write!(f, "SetBool\t\tresult: {}\tvalue: {}", register, value)
            }
            SetNumber { register, value } => {
                write!(f, "SetNumber\tresult: {}\tvalue: {}", register, value)
            }
            LoadNumber { register, constant } => write!(
                f,
                "LoadNumber\tresult: {}\tconstant: {}",
                register, constant
            ),
            LoadString { register, constant } => write!(
                f,
                "LoadString\tresult: {}\tconstant: {}",
                register, constant
            ),
            LoadGlobal { register, constant } => write!(
                f,
                "LoadGlobal\tresult: {}\tconstant: {}",
                register, constant
            ),
            SetGlobal { global, source } => {
                write!(f, "SetGlobal\tglobal: {}\tsource: {}", global, source)
            }
            Import { register, constant } => {
                write!(f, "Import\t\tresult: {}\tconstant: {}", register, constant)
            }
            MakeTuple {
                register,
                start,
                count,
            } => write!(
                f,
                "MakeTuple\tresult: {}\tstart: {}\tcount: {}",
                register, start, count
            ),
            MakeTempTuple {
                register,
                start,
                count,
            } => write!(
                f,
                "MakeTempTuple\tresult: {}\tstart: {}\tcount: {}",
                register, start, count
            ),
            MakeList {
                register,
                size_hint,
            } => write!(
                f,
                "MakeList\tresult: {}\tsize_hint: {}",
                register, size_hint
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
            LoadCapture { register, capture } => {
                write!(f, "LoadCapture\tresult: {}\tcapture: {}", register, capture)
            }
            SetCapture { capture, source } => {
                write!(f, "SetCapture\tcapture: {}\tsource {}", capture, source)
            }
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
                "CallChild\tresult: {}\tfunction: {}\tframe_base: {}\targs: {}\t\tparent: {}",
                result, function, frame_base, arg_count, parent
            ),
            Return { register } => write!(f, "Return\t\tresult: {}", register),
            Yield { register } => write!(f, "Yield\t\tresult: {}", register),
            Size { register, value } => write!(f, "Size\t\tresult: {}\tvalue: {}", register, value),
            Type { register, value } => write!(f, "Type\t\tresult: {}\tvalue: {}", register, value),
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
            ListPushValue { list, value } => {
                write!(f, "ListPushValue\tlist: {}\t\tvalue: {}", list, value)
            }
            ListPushValues {
                list,
                values_start,
                count,
            } => write!(
                f,
                "ListPushValues\tlist: {}\t\tstart: {}\tcount: {}",
                list, values_start, count
            ),
            ListUpdate { list, index, value } => write!(
                f,
                "ListUpdate\tlist: {}\t\tindex: {}\tvalue: {}",
                list, index, value
            ),
            Index {
                register,
                value,
                index,
            } => write!(
                f,
                "Index\tresult: {}\tvalue: {}\t\tindex: {}",
                register, value, index
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
            MapAccess { register, map, key } => write!(
                f,
                "MapAccess\tresult: {}\tmap: {}\t\tkey: {}",
                register, map, key
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
        }
    }
}

#[derive(Clone, Default)]
pub struct InstructionReader {
    pub chunk: Arc<Chunk>,
    pub ip: usize,
}

impl InstructionReader {
    pub fn new(chunk: Arc<Chunk>) -> Self {
        Self { chunk, ip: 0 }
    }
}

impl Iterator for InstructionReader {
    type Item = Instruction;

    fn next(&mut self) -> Option<Self::Item> {
        use Instruction::*;

        macro_rules! get_byte {
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
                target: get_byte!(),
                source: get_byte!(),
            }),
            Op::DeepCopy => Some(DeepCopy {
                target: get_byte!(),
                source: get_byte!(),
            }),
            Op::SetEmpty => Some(SetEmpty {
                register: get_byte!(),
            }),
            Op::SetFalse => Some(SetBool {
                register: get_byte!(),
                value: false,
            }),
            Op::SetTrue => Some(SetBool {
                register: get_byte!(),
                value: true,
            }),
            Op::Set0 => Some(SetNumber {
                register: get_byte!(),
                value: 0.0,
            }),
            Op::Set1 => Some(SetNumber {
                register: get_byte!(),
                value: 1.0,
            }),
            Op::SetNumberU8 => Some(SetNumber {
                register: get_byte!(),
                value: get_byte!() as f64,
            }),
            Op::LoadNumber => Some(LoadNumber {
                register: get_byte!(),
                constant: get_byte!() as ConstantIndex,
            }),
            Op::LoadNumberLong => Some(LoadNumber {
                register: get_byte!(),
                constant: get_u32!() as ConstantIndex,
            }),
            Op::LoadString => Some(LoadString {
                register: get_byte!(),
                constant: get_byte!() as ConstantIndex,
            }),
            Op::LoadStringLong => Some(LoadString {
                register: get_byte!(),
                constant: get_u32!() as ConstantIndex,
            }),
            Op::LoadGlobal => Some(LoadGlobal {
                register: get_byte!(),
                constant: get_byte!() as ConstantIndex,
            }),
            Op::LoadGlobalLong => Some(LoadGlobal {
                register: get_byte!(),
                constant: get_u32!() as ConstantIndex,
            }),
            Op::SetGlobal => Some(SetGlobal {
                global: get_byte!() as ConstantIndex,
                source: get_byte!(),
            }),
            Op::SetGlobalLong => Some(SetGlobal {
                global: get_u32!() as ConstantIndex,
                source: get_byte!(),
            }),
            Op::Import => Some(Import {
                register: get_byte!(),
                constant: get_byte!() as ConstantIndex,
            }),
            Op::ImportLong => Some(Import {
                register: get_byte!(),
                constant: get_u32!() as ConstantIndex,
            }),
            Op::MakeTuple => Some(MakeTuple {
                register: get_byte!(),
                start: get_byte!(),
                count: get_byte!(),
            }),
            Op::MakeTempTuple => Some(MakeTempTuple {
                register: get_byte!(),
                start: get_byte!(),
                count: get_byte!(),
            }),
            Op::MakeList => Some(MakeList {
                register: get_byte!(),
                size_hint: get_byte!() as usize,
            }),
            Op::MakeListLong => Some(MakeList {
                register: get_byte!(),
                size_hint: get_u32!() as usize,
            }),
            Op::MakeMap => Some(MakeMap {
                register: get_byte!(),
                size_hint: get_byte!() as usize,
            }),
            Op::MakeMapLong => Some(MakeMap {
                register: get_byte!(),
                size_hint: get_u32!() as usize,
            }),
            Op::MakeNum2 => Some(MakeNum2 {
                register: get_byte!(),
                count: get_byte!(),
                element_register: get_byte!(),
            }),
            Op::MakeNum4 => Some(MakeNum4 {
                register: get_byte!(),
                count: get_byte!(),
                element_register: get_byte!(),
            }),
            Op::Range => Some(Range {
                register: get_byte!(),
                start: get_byte!(),
                end: get_byte!(),
            }),
            Op::RangeInclusive => Some(RangeInclusive {
                register: get_byte!(),
                start: get_byte!(),
                end: get_byte!(),
            }),
            Op::RangeTo => Some(RangeTo {
                register: get_byte!(),
                end: get_byte!(),
            }),
            Op::RangeToInclusive => Some(RangeToInclusive {
                register: get_byte!(),
                end: get_byte!(),
            }),
            Op::RangeFrom => Some(RangeFrom {
                register: get_byte!(),
                start: get_byte!(),
            }),
            Op::RangeFull => Some(RangeFull {
                register: get_byte!(),
            }),
            Op::MakeIterator => Some(MakeIterator {
                register: get_byte!(),
                iterable: get_byte!(),
            }),
            Op::Function => {
                let register = get_byte!();
                let arg_count = get_byte!();
                let capture_count = get_byte!();
                let flags = FunctionFlags::from_byte(get_byte!());
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
                function: get_byte!(),
                target: get_byte!(),
                source: get_byte!(),
            }),
            Op::LoadCapture => Some(LoadCapture {
                register: get_byte!(),
                capture: get_byte!(),
            }),
            Op::SetCapture => Some(SetCapture {
                capture: get_byte!(),
                source: get_byte!(),
            }),
            Op::Negate => Some(Negate {
                register: get_byte!(),
                source: get_byte!(),
            }),
            Op::Add => Some(Add {
                register: get_byte!(),
                lhs: get_byte!(),
                rhs: get_byte!(),
            }),
            Op::Subtract => Some(Subtract {
                register: get_byte!(),
                lhs: get_byte!(),
                rhs: get_byte!(),
            }),
            Op::Multiply => Some(Multiply {
                register: get_byte!(),
                lhs: get_byte!(),
                rhs: get_byte!(),
            }),
            Op::Divide => Some(Divide {
                register: get_byte!(),
                lhs: get_byte!(),
                rhs: get_byte!(),
            }),
            Op::Modulo => Some(Modulo {
                register: get_byte!(),
                lhs: get_byte!(),
                rhs: get_byte!(),
            }),
            Op::Less => Some(Less {
                register: get_byte!(),
                lhs: get_byte!(),
                rhs: get_byte!(),
            }),
            Op::LessOrEqual => Some(LessOrEqual {
                register: get_byte!(),
                lhs: get_byte!(),
                rhs: get_byte!(),
            }),
            Op::Greater => Some(Greater {
                register: get_byte!(),
                lhs: get_byte!(),
                rhs: get_byte!(),
            }),
            Op::GreaterOrEqual => Some(GreaterOrEqual {
                register: get_byte!(),
                lhs: get_byte!(),
                rhs: get_byte!(),
            }),
            Op::Equal => Some(Equal {
                register: get_byte!(),
                lhs: get_byte!(),
                rhs: get_byte!(),
            }),
            Op::NotEqual => Some(NotEqual {
                register: get_byte!(),
                lhs: get_byte!(),
                rhs: get_byte!(),
            }),
            Op::Jump => Some(Jump {
                offset: get_u16!() as usize,
            }),
            Op::JumpTrue => Some(JumpIf {
                register: get_byte!(),
                offset: get_u16!() as usize,
                jump_condition: true,
            }),
            Op::JumpFalse => Some(JumpIf {
                register: get_byte!(),
                offset: get_u16!() as usize,
                jump_condition: false,
            }),
            Op::JumpBack => Some(JumpBack {
                offset: get_u16!() as usize,
            }),
            Op::JumpBackFalse => Some(JumpBackIf {
                register: get_byte!(),
                offset: get_u16!() as usize,
                jump_condition: false,
            }),
            Op::Call => Some(Call {
                result: get_byte!(),
                function: get_byte!(),
                frame_base: get_byte!(),
                arg_count: get_byte!(),
            }),
            Op::CallChild => Some(CallChild {
                result: get_byte!(),
                function: get_byte!(),
                frame_base: get_byte!(),
                arg_count: get_byte!(),
                parent: get_byte!(),
            }),
            Op::Return => Some(Return {
                register: get_byte!(),
            }),
            Op::Yield => Some(Yield {
                register: get_byte!(),
            }),
            Op::Size => Some(Size {
                register: get_byte!(),
                value: get_byte!(),
            }),
            Op::Type => Some(Type {
                register: get_byte!(),
                value: get_byte!(),
            }),
            Op::IterNext => Some(IterNext {
                register: get_byte!(),
                iterator: get_byte!(),
                jump_offset: get_u16!() as usize,
            }),
            Op::IterNextTemp => Some(IterNextTemp {
                register: get_byte!(),
                iterator: get_byte!(),
                jump_offset: get_u16!() as usize,
            }),
            Op::IterNextQuiet => Some(IterNextQuiet {
                iterator: get_byte!(),
                jump_offset: get_u16!() as usize,
            }),
            Op::ValueIndex => Some(ValueIndex {
                register: get_byte!(),
                value: get_byte!(),
                index: get_byte!() as i8,
            }),
            Op::SliceFrom => Some(SliceFrom {
                register: get_byte!(),
                value: get_byte!(),
                index: get_byte!() as i8,
            }),
            Op::SliceTo => Some(SliceTo {
                register: get_byte!(),
                value: get_byte!(),
                index: get_byte!() as i8,
            }),
            Op::IsTuple => Some(IsTuple {
                register: get_byte!(),
                value: get_byte!(),
            }),
            Op::IsList => Some(IsList {
                register: get_byte!(),
                value: get_byte!(),
            }),
            Op::ListPushValue => Some(ListPushValue {
                list: get_byte!(),
                value: get_byte!(),
            }),
            Op::ListPushValues => Some(ListPushValues {
                list: get_byte!(),
                values_start: get_byte!(),
                count: get_byte!(),
            }),
            Op::ListUpdate => Some(ListUpdate {
                list: get_byte!(),
                index: get_byte!(),
                value: get_byte!(),
            }),
            Op::Index => Some(Index {
                register: get_byte!(),
                value: get_byte!(),
                index: get_byte!(),
            }),
            Op::MapInsert => Some(MapInsert {
                register: get_byte!(),
                value: get_byte!(),
                key: get_byte!() as ConstantIndex,
            }),
            Op::MapInsertLong => Some(MapInsert {
                register: get_byte!(),
                value: get_byte!(),
                key: get_u32!() as ConstantIndex,
            }),
            Op::MapAccess => Some(MapAccess {
                register: get_byte!(),
                map: get_byte!(),
                key: get_byte!() as ConstantIndex,
            }),
            Op::MapAccessLong => Some(MapAccess {
                register: get_byte!(),
                map: get_byte!(),
                key: get_u32!() as ConstantIndex,
            }),
            Op::TryStart => Some(TryStart {
                arg_register: get_byte!(),
                catch_offset: get_u16!() as usize,
            }),
            Op::TryEnd => Some(TryEnd),
            Op::Debug => Some(Debug {
                register: get_byte!(),
                constant: get_u32!() as ConstantIndex,
            }),
            _ => Some(Error {
                message: format!("Unexpected opcode {:?} found at instruction {}", op, op_ip),
            }),
        }
    }
}
