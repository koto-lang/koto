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
                    None => return out_of_bounds_access_error(self.ip),
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
                    None => return out_of_bounds_access_error(self.ip),
                }
            }};
        }

        macro_rules! get_var_u32 {
            () => {{
                let mut result = 0;
                let mut shift_amount = 0;
                loop {
                    let Some(&byte) = self.chunk.bytes.get(self.ip) else {
                        return out_of_bounds_access_error(self.ip);
                    };
                    self.ip += 1;
                    result |= (byte as u32 & 0x7f) << shift_amount;
                    if byte & 0x80 == 0 {
                        break;
                    } else {
                        shift_amount += 7;
                    }
                }
                result
            }};
        }

        macro_rules! get_var_u32_with_first_byte {
            ($first_byte:expr) => {{
                let mut byte = $first_byte;
                let mut result = (byte as u32 & 0x7f);
                let mut shift_amount = 0;
                while byte & 0x80 != 0 {
                    let Some(&next_byte) = self.chunk.bytes.get(self.ip) else {
                        return out_of_bounds_access_error(self.ip);
                    };

                    byte = next_byte;
                    self.ip += 1;
                    shift_amount += 7;

                    result |= (byte as u32 & 0x7f) << shift_amount;
                }
                result
            }};
        }

        // Each op consists of at least two bytes
        let op_ip = self.ip;
        let (op, byte_a) = match self.chunk.bytes.get(op_ip..op_ip + 2) {
            Some(&[op, byte]) => (Op::from(op), byte),
            _ => return None,
        };
        self.ip += 2;

        match op {
            Op::Copy => Some(Copy {
                target: byte_a,
                source: get_u8!(),
            }),
            Op::SetNull => Some(SetNull { register: byte_a }),
            Op::SetFalse => Some(SetBool {
                register: byte_a,
                value: false,
            }),
            Op::SetTrue => Some(SetBool {
                register: byte_a,
                value: true,
            }),
            Op::Set0 => Some(SetNumber {
                register: byte_a,
                value: 0,
            }),
            Op::Set1 => Some(SetNumber {
                register: byte_a,
                value: 1,
            }),
            Op::SetNumberU8 => Some(SetNumber {
                register: byte_a,
                value: get_u8!() as i64,
            }),
            Op::SetNumberNegU8 => Some(SetNumber {
                register: byte_a,
                value: -(get_u8!() as i64),
            }),
            Op::LoadFloat => Some(LoadFloat {
                register: byte_a,
                constant: get_var_u32!().into(),
            }),
            Op::LoadInt => Some(LoadInt {
                register: byte_a,
                constant: get_var_u32!().into(),
            }),
            Op::LoadString => Some(LoadString {
                register: byte_a,
                constant: get_var_u32!().into(),
            }),
            Op::LoadNonLocal => Some(LoadNonLocal {
                register: byte_a,
                constant: get_var_u32!().into(),
            }),
            Op::ValueExport => Some(ValueExport {
                name: byte_a,
                value: get_u8!(),
            }),
            Op::Import => Some(Import { register: byte_a }),
            Op::MakeTempTuple => Some(MakeTempTuple {
                register: byte_a,
                start: get_u8!(),
                count: get_u8!(),
            }),
            Op::TempTupleToTuple => Some(TempTupleToTuple {
                register: byte_a,
                source: get_u8!(),
            }),
            Op::MakeMap => Some(MakeMap {
                register: byte_a,
                size_hint: get_var_u32!(),
            }),
            Op::SequenceStart => Some(SequenceStart {
                size_hint: get_var_u32_with_first_byte!(byte_a),
            }),
            Op::SequencePush => Some(SequencePush { value: byte_a }),
            Op::SequencePushN => Some(SequencePushN {
                start: byte_a,
                count: get_u8!(),
            }),
            Op::SequenceToList => Some(SequenceToList { register: byte_a }),
            Op::SequenceToTuple => Some(SequenceToTuple { register: byte_a }),
            Op::Range => Some(Range {
                register: byte_a,
                start: get_u8!(),
                end: get_u8!(),
            }),
            Op::RangeInclusive => Some(RangeInclusive {
                register: byte_a,
                start: get_u8!(),
                end: get_u8!(),
            }),
            Op::RangeTo => Some(RangeTo {
                register: byte_a,
                end: get_u8!(),
            }),
            Op::RangeToInclusive => Some(RangeToInclusive {
                register: byte_a,
                end: get_u8!(),
            }),
            Op::RangeFrom => Some(RangeFrom {
                register: byte_a,
                start: get_u8!(),
            }),
            Op::RangeFull => Some(RangeFull { register: byte_a }),
            Op::MakeIterator => Some(MakeIterator {
                register: byte_a,
                iterable: get_u8!(),
            }),
            Op::Function => {
                let register = byte_a;
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
                function: byte_a,
                target: get_u8!(),
                source: get_u8!(),
            }),
            Op::Negate => Some(Negate {
                register: byte_a,
                value: get_u8!(),
            }),
            Op::Not => Some(Not {
                register: byte_a,
                value: get_u8!(),
            }),
            Op::Add => Some(Add {
                register: byte_a,
                lhs: get_u8!(),
                rhs: get_u8!(),
            }),
            Op::Subtract => Some(Subtract {
                register: byte_a,
                lhs: get_u8!(),
                rhs: get_u8!(),
            }),
            Op::Multiply => Some(Multiply {
                register: byte_a,
                lhs: get_u8!(),
                rhs: get_u8!(),
            }),
            Op::Divide => Some(Divide {
                register: byte_a,
                lhs: get_u8!(),
                rhs: get_u8!(),
            }),
            Op::Remainder => Some(Remainder {
                register: byte_a,
                lhs: get_u8!(),
                rhs: get_u8!(),
            }),
            Op::AddAssign => Some(AddAssign {
                lhs: byte_a,
                rhs: get_u8!(),
            }),
            Op::SubtractAssign => Some(SubtractAssign {
                lhs: byte_a,
                rhs: get_u8!(),
            }),
            Op::MultiplyAssign => Some(MultiplyAssign {
                lhs: byte_a,
                rhs: get_u8!(),
            }),
            Op::DivideAssign => Some(DivideAssign {
                lhs: byte_a,
                rhs: get_u8!(),
            }),
            Op::RemainderAssign => Some(RemainderAssign {
                lhs: byte_a,
                rhs: get_u8!(),
            }),
            Op::Less => Some(Less {
                register: byte_a,
                lhs: get_u8!(),
                rhs: get_u8!(),
            }),
            Op::LessOrEqual => Some(LessOrEqual {
                register: byte_a,
                lhs: get_u8!(),
                rhs: get_u8!(),
            }),
            Op::Greater => Some(Greater {
                register: byte_a,
                lhs: get_u8!(),
                rhs: get_u8!(),
            }),
            Op::GreaterOrEqual => Some(GreaterOrEqual {
                register: byte_a,
                lhs: get_u8!(),
                rhs: get_u8!(),
            }),
            Op::Equal => Some(Equal {
                register: byte_a,
                lhs: get_u8!(),
                rhs: get_u8!(),
            }),
            Op::NotEqual => Some(NotEqual {
                register: byte_a,
                lhs: get_u8!(),
                rhs: get_u8!(),
            }),
            Op::Jump => Some(Jump {
                offset: u16::from_le_bytes([byte_a, get_u8!()]),
            }),
            Op::JumpBack => Some(JumpBack {
                offset: u16::from_le_bytes([byte_a, get_u8!()]),
            }),
            Op::JumpIfTrue => Some(JumpIfTrue {
                register: byte_a,
                offset: get_u16!(),
            }),
            Op::JumpIfFalse => Some(JumpIfFalse {
                register: byte_a,
                offset: get_u16!(),
            }),
            Op::JumpIfNull => Some(JumpIfNull {
                register: byte_a,
                offset: get_u16!(),
            }),
            Op::Call => Some(Call {
                result: byte_a,
                function: get_u8!(),
                frame_base: get_u8!(),
                arg_count: get_u8!(),
            }),
            Op::CallInstance => Some(CallInstance {
                result: byte_a,
                function: get_u8!(),
                instance: get_u8!(),
                frame_base: get_u8!(),
                arg_count: get_u8!(),
            }),
            Op::Return => Some(Return { register: byte_a }),
            Op::Yield => Some(Yield { register: byte_a }),
            Op::Throw => Some(Throw { register: byte_a }),
            Op::Size => Some(Size {
                register: byte_a,
                value: get_u8!(),
            }),
            Op::IterNext => Some(IterNext {
                result: Some(byte_a),
                iterator: get_u8!(),
                jump_offset: get_u16!(),
                temporary_output: false,
            }),
            Op::IterNextTemp => Some(IterNext {
                result: Some(byte_a),
                iterator: get_u8!(),
                jump_offset: get_u16!(),
                temporary_output: true,
            }),
            Op::IterNextQuiet => Some(IterNext {
                result: None,
                iterator: byte_a,
                jump_offset: get_u16!(),
                temporary_output: false,
            }),
            Op::IterUnpack => Some(IterNext {
                result: Some(byte_a),
                iterator: get_u8!(),
                jump_offset: 0,
                temporary_output: false,
            }),
            Op::TempIndex => Some(TempIndex {
                register: byte_a,
                value: get_u8!(),
                index: get_u8!() as i8,
            }),
            Op::SliceFrom => Some(SliceFrom {
                register: byte_a,
                value: get_u8!(),
                index: get_u8!() as i8,
            }),
            Op::SliceTo => Some(SliceTo {
                register: byte_a,
                value: get_u8!(),
                index: get_u8!() as i8,
            }),
            Op::Index => Some(Index {
                register: byte_a,
                value: get_u8!(),
                index: get_u8!(),
            }),
            Op::IndexMut => Some(IndexMut {
                register: byte_a,
                index: get_u8!(),
                value: get_u8!(),
            }),
            Op::MapInsert => Some(MapInsert {
                register: byte_a,
                key: get_u8!(),
                value: get_u8!(),
            }),
            Op::MetaInsert => {
                let register = byte_a;
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
                let register = byte_a;
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
                let meta_id = byte_a;
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
                let meta_id = byte_a;
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
                register: byte_a,
                value: get_u8!(),
                key: get_var_u32!().into(),
            }),
            Op::AccessString => Some(AccessString {
                register: byte_a,
                value: get_u8!(),
                key: get_u8!(),
            }),
            Op::TryStart => Some(TryStart {
                arg_register: byte_a,
                catch_offset: get_u16!(),
            }),
            Op::TryEnd => Some(TryEnd),
            Op::Debug => Some(Debug {
                register: byte_a,
                constant: get_var_u32!().into(),
            }),
            Op::CheckSizeEqual => Some(CheckSizeEqual {
                register: byte_a,
                size: get_u8!() as usize,
            }),
            Op::CheckSizeMin => Some(CheckSizeMin {
                register: byte_a,
                size: get_u8!() as usize,
            }),
            Op::AssertType => Some(AssertType {
                value: byte_a,
                type_string: get_var_u32!().into(),
            }),
            Op::CheckType => Some(CheckType {
                value: byte_a,
                type_string: get_var_u32!().into(),
                jump_offset: get_u16!(),
            }),
            Op::StringStart => Some(StringStart {
                size_hint: get_var_u32_with_first_byte!(byte_a),
            }),
            Op::StringPush => {
                let value = byte_a;
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
            Op::StringFinish => Some(StringFinish { register: byte_a }),
            _ => Some(Error {
                message: format!("Unexpected opcode {op:?} found at instruction {op_ip}"),
            }),
        }
    }
}

#[inline(never)]
fn out_of_bounds_access_error(ip: usize) -> Option<Instruction> {
    Some(Instruction::Error {
        message: format!("Instruction access out of bounds at {ip}"),
    })
}
