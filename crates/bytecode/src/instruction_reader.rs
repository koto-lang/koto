use crate::{Chunk, FunctionFlags, Instruction, Op, StringFormatFlags};
use koto_memory::Ptr;
use koto_parser::StringFormatOptions;

/// An iterator that converts bytecode into a series of [Instruction]s
#[derive(Clone, Default)]
pub struct InstructionReader {
    /// The chunk that the reader is reading from
    pub chunk: Ptr<Chunk>,
    /// The reader's instruction pointer
    pub ip: usize,
}

impl InstructionReader {
    /// Initializes a reader with the given chunk
    pub fn new(chunk: Ptr<Chunk>) -> Self {
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

        macro_rules! get_var_u32 {
            () => {{
                let mut result = 0;
                let mut shift_amount = 0;
                loop {
                    match self.chunk.bytes.get(self.ip) {
                        Some(byte) => {
                            self.ip += 1;
                            result |= (*byte as u32 & 0x7f) << shift_amount;
                            if byte & 0x80 == 0 {
                                break;
                            } else {
                                shift_amount += 7;
                            }
                        }
                        None => {
                            return Some(Error {
                                message: format!("Expected byte at position {}", self.ip),
                            });
                        }
                    }
                }
                result
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
            Op::SetNumberNegU8 => Some(SetNumber {
                register: get_u8!(),
                value: -(get_u8!() as i64),
            }),
            Op::LoadFloat => Some(LoadFloat {
                register: get_u8!(),
                constant: get_var_u32!().into(),
            }),
            Op::LoadInt => Some(LoadInt {
                register: get_u8!(),
                constant: get_var_u32!().into(),
            }),
            Op::LoadString => Some(LoadString {
                register: get_u8!(),
                constant: get_var_u32!().into(),
            }),
            Op::LoadNonLocal => Some(LoadNonLocal {
                register: get_u8!(),
                constant: get_var_u32!().into(),
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
                size_hint: get_var_u32!(),
            }),
            Op::SequenceStart => Some(SequenceStart {
                size_hint: get_var_u32!(),
            }),
            Op::SequencePush => Some(SequencePush { value: get_u8!() }),
            Op::SequencePushN => Some(SequencePushN {
                start: get_u8!(),
                count: get_u8!(),
            }),
            Op::SequenceToList => Some(SequenceToList {
                register: get_u8!(),
            }),
            Op::SequenceToTuple => Some(SequenceToTuple {
                register: get_u8!(),
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
            Op::Function => {
                let register = get_u8!();
                let arg_count = get_u8!();
                let capture_count = get_u8!();
                let flags = FunctionFlags::from_byte(get_u8!());
                let size = get_u16!();

                Some(Function {
                    register,
                    arg_count,
                    capture_count,
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
            Op::AddAssign => Some(AddAssign {
                lhs: get_u8!(),
                rhs: get_u8!(),
            }),
            Op::SubtractAssign => Some(SubtractAssign {
                lhs: get_u8!(),
                rhs: get_u8!(),
            }),
            Op::MultiplyAssign => Some(MultiplyAssign {
                lhs: get_u8!(),
                rhs: get_u8!(),
            }),
            Op::DivideAssign => Some(DivideAssign {
                lhs: get_u8!(),
                rhs: get_u8!(),
            }),
            Op::RemainderAssign => Some(RemainderAssign {
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
            Op::Jump => Some(Jump { offset: get_u16!() }),
            Op::JumpBack => Some(JumpBack { offset: get_u16!() }),
            Op::JumpIfTrue => Some(JumpIfTrue {
                register: get_u8!(),
                offset: get_u16!(),
            }),
            Op::JumpIfFalse => Some(JumpIfFalse {
                register: get_u8!(),
                offset: get_u16!(),
            }),
            Op::Call => Some(Call {
                result: get_u8!(),
                function: get_u8!(),
                frame_base: get_u8!(),
                arg_count: get_u8!(),
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
                result: Some(get_u8!()),
                iterator: get_u8!(),
                jump_offset: get_u16!(),
                temporary_output: false,
            }),
            Op::IterNextTemp => Some(IterNext {
                result: Some(get_u8!()),
                iterator: get_u8!(),
                jump_offset: get_u16!(),
                temporary_output: true,
            }),
            Op::IterNextQuiet => Some(IterNext {
                result: None,
                iterator: get_u8!(),
                jump_offset: get_u16!(),
                temporary_output: false,
            }),
            Op::IterUnpack => Some(IterNext {
                result: Some(get_u8!()),
                iterator: get_u8!(),
                jump_offset: 0,
                temporary_output: false,
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
                            "Unexpected meta id {meta_id} found at instruction {op_ip}",
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
                key: get_var_u32!().into(),
            }),
            Op::AccessString => Some(AccessString {
                register: get_u8!(),
                value: get_u8!(),
                key: get_u8!(),
            }),
            Op::TryStart => Some(TryStart {
                arg_register: get_u8!(),
                catch_offset: get_u16!(),
            }),
            Op::TryEnd => Some(TryEnd),
            Op::Debug => Some(Debug {
                register: get_u8!(),
                constant: get_var_u32!().into(),
            }),
            Op::CheckSizeEqual => Some(CheckSizeEqual {
                register: get_u8!(),
                size: get_u8!() as usize,
            }),
            Op::CheckSizeMin => Some(CheckSizeMin {
                register: get_u8!(),
                size: get_u8!() as usize,
            }),
            Op::AssertType => Some(AssertType {
                value: get_u8!(),
                type_string: get_var_u32!().into(),
            }),
            Op::CheckType => Some(CheckType {
                value: get_u8!(),
                type_string: get_var_u32!().into(),
                jump_offset: get_u16!(),
            }),
            Op::StringStart => Some(StringStart {
                size_hint: get_var_u32!(),
            }),
            Op::StringPush => {
                let value = get_u8!();
                let flags = get_u8!();

                let format_options = if flags != 0 {
                    let flags = StringFormatFlags::from_byte(flags);

                    let mut options = StringFormatOptions {
                        alignment: flags.alignment,
                        ..Default::default()
                    };
                    if flags.min_width {
                        options.min_width = Some(get_var_u32!());
                    }
                    if flags.precision {
                        options.precision = Some(get_var_u32!());
                    }
                    if flags.fill_character {
                        options.fill_character = Some(get_var_u32!().into());
                    }

                    Some(options)
                } else {
                    None
                };

                Some(StringPush {
                    value,
                    format_options,
                })
            }
            Op::StringFinish => Some(StringFinish {
                register: get_u8!(),
            }),
            _ => Some(Error {
                message: format!("Unexpected opcode {op:?} found at instruction {op_ip}"),
            }),
        }
    }
}
