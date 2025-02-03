use crate::{Chunk, FunctionFlags, Instruction, Op, StringFormatFlags};
use koto_memory::Ptr;
use koto_parser::StringFormatOptions;
use std::mem::MaybeUninit;

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

        let bytes = self.chunk.bytes.as_slice();

        macro_rules! get_u8 {
            () => {{
                match bytes.get(self.ip) {
                    Some(byte) => {
                        self.ip += 1;
                        *byte
                    }
                    None => return out_of_bounds_access_error(self.ip),
                }
            }};
        }

        macro_rules! get_u8_array {
            ($n:expr) => {{
                if bytes.len() >= self.ip + $n {
                    // Safety:
                    // - The size of `bytes` has been checked so we know its safe to access ip + $n
                    // - `result` is fully initialized as a result of the copy_nonoverlapping call
                    //   so it's safe to transmute.
                    // Todo: Simplify once https://github.com/rust-lang/rust/issues/96097 is stable.
                    unsafe {
                        let mut result: [MaybeUninit<u8>; $n] = MaybeUninit::uninit().assume_init();
                        std::ptr::copy_nonoverlapping(
                            bytes.as_ptr().add(self.ip),
                            result.as_mut_ptr() as *mut u8,
                            $n,
                        );
                        self.ip += $n;
                        // Convert `MaybeUninit<[u8; $n]>` to `[u8; $n]`.
                        std::mem::transmute::<[MaybeUninit<u8>; $n], [u8; $n]>(result)
                    }
                } else {
                    return out_of_bounds_access_error(self.ip);
                }
            }};
        }
        macro_rules! get_u8x2 {
            () => {{
                get_u8_array!(2)
            }};
        }
        macro_rules! get_u8x3 {
            () => {{
                get_u8_array!(3)
            }};
        }
        macro_rules! get_u8x4 {
            () => {{
                get_u8_array!(4)
            }};
        }
        macro_rules! get_u8x5 {
            () => {{
                get_u8_array!(5)
            }};
        }
        macro_rules! get_u8x6 {
            () => {{
                get_u8_array!(6)
            }};
        }

        macro_rules! get_u16 {
            () => {{
                let [a, b] = get_u8x2!();
                u16::from_le_bytes([a, b])
            }};
        }

        macro_rules! get_var_u32 {
            () => {{
                let mut result = 0;
                let mut shift_amount = 0;
                loop {
                    let Some(&byte) = bytes.get(self.ip) else {
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
                    let Some(&next_byte) = bytes.get(self.ip) else {
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
        let (op, byte_a) = match bytes.get(op_ip..op_ip + 2) {
            Some(&[op, byte]) => (Op::from(op), byte),
            _ => return None,
        };
        self.ip += 2;

        match op {
            Op::NewFrame => Some(NewFrame {
                register_count: byte_a,
            }),
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
            Op::MakeTempTuple => {
                let [byte_b, byte_c] = get_u8x2!();
                Some(MakeTempTuple {
                    register: byte_a,
                    start: byte_b,
                    count: byte_c,
                })
            }
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
            Op::Range => {
                let [byte_b, byte_c] = get_u8x2!();
                Some(Range {
                    register: byte_a,
                    start: byte_b,
                    end: byte_c,
                })
            }
            Op::RangeInclusive => {
                let [byte_b, byte_c] = get_u8x2!();
                Some(RangeInclusive {
                    register: byte_a,
                    start: byte_b,
                    end: byte_c,
                })
            }
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
                let [arg_count, optional_arg_count, capture_count, flags, size_a, size_b] =
                    get_u8x6!();
                match FunctionFlags::try_from(flags) {
                    Ok(flags) => {
                        let size = u16::from_le_bytes([size_a, size_b]);

                        Some(Function {
                            register,
                            arg_count,
                            optional_arg_count,
                            capture_count,
                            flags,
                            size,
                        })
                    }
                    Err(e) => Some(Error { message: e }),
                }
            }
            Op::Capture => {
                let [byte_b, byte_c] = get_u8x2!();
                Some(Capture {
                    function: byte_a,
                    target: byte_b,
                    source: byte_c,
                })
            }
            Op::Negate => Some(Negate {
                register: byte_a,
                value: get_u8!(),
            }),
            Op::Not => Some(Not {
                register: byte_a,
                value: get_u8!(),
            }),
            Op::Add => {
                let [byte_b, byte_c] = get_u8x2!();
                Some(Add {
                    register: byte_a,
                    lhs: byte_b,
                    rhs: byte_c,
                })
            }
            Op::Subtract => {
                let [byte_b, byte_c] = get_u8x2!();
                Some(Subtract {
                    register: byte_a,
                    lhs: byte_b,
                    rhs: byte_c,
                })
            }
            Op::Multiply => {
                let [byte_b, byte_c] = get_u8x2!();
                Some(Multiply {
                    register: byte_a,
                    lhs: byte_b,
                    rhs: byte_c,
                })
            }
            Op::Divide => {
                let [byte_b, byte_c] = get_u8x2!();
                Some(Divide {
                    register: byte_a,
                    lhs: byte_b,
                    rhs: byte_c,
                })
            }
            Op::Remainder => {
                let [byte_b, byte_c] = get_u8x2!();
                Some(Remainder {
                    register: byte_a,
                    lhs: byte_b,
                    rhs: byte_c,
                })
            }
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
            Op::Less => {
                let [lhs, rhs] = get_u8x2!();
                Some(Less {
                    register: byte_a,
                    lhs,
                    rhs,
                })
            }
            Op::LessOrEqual => {
                let [lhs, rhs] = get_u8x2!();
                Some(LessOrEqual {
                    register: byte_a,
                    lhs,
                    rhs,
                })
            }
            Op::Greater => {
                let [lhs, rhs] = get_u8x2!();
                Some(Greater {
                    register: byte_a,
                    lhs,
                    rhs,
                })
            }
            Op::GreaterOrEqual => {
                let [lhs, rhs] = get_u8x2!();
                Some(GreaterOrEqual {
                    register: byte_a,
                    lhs,
                    rhs,
                })
            }
            Op::Equal => {
                let [lhs, rhs] = get_u8x2!();
                Some(Equal {
                    register: byte_a,
                    lhs,
                    rhs,
                })
            }
            Op::NotEqual => {
                let [byte_b, byte_c] = get_u8x2!();
                Some(NotEqual {
                    register: byte_a,
                    lhs: byte_b,
                    rhs: byte_c,
                })
            }
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
            Op::Call => {
                let [function, frame_base, arg_count, unpacked_arg_count] = get_u8x4!();
                Some(Call {
                    result: byte_a,
                    function,
                    frame_base,
                    arg_count,
                    packed_arg_count: unpacked_arg_count,
                })
            }
            Op::CallInstance => {
                let [function, instance, frame_base, arg_count, unpacked_arg_count] = get_u8x5!();
                Some(CallInstance {
                    result: byte_a,
                    function,
                    instance,
                    frame_base,
                    arg_count,
                    packed_arg_count: unpacked_arg_count,
                })
            }
            Op::Return => Some(Return { register: byte_a }),
            Op::Yield => Some(Yield { register: byte_a }),
            Op::Throw => Some(Throw { register: byte_a }),
            Op::Size => Some(Size {
                register: byte_a,
                value: get_u8!(),
            }),
            Op::IterNext => {
                let [byte_b, byte_c, byte_d] = get_u8x3!();
                Some(IterNext {
                    result: Some(byte_a),
                    iterator: byte_b,
                    jump_offset: u16::from_le_bytes([byte_c, byte_d]),
                    temporary_output: false,
                })
            }
            Op::IterNextTemp => {
                let [byte_b, byte_c, byte_d] = get_u8x3!();
                Some(IterNext {
                    result: Some(byte_a),
                    iterator: byte_b,
                    jump_offset: u16::from_le_bytes([byte_c, byte_d]),
                    temporary_output: true,
                })
            }
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
            Op::TempIndex => {
                let [byte_b, byte_c] = get_u8x2!();
                Some(TempIndex {
                    register: byte_a,
                    value: byte_b,
                    index: byte_c as i8,
                })
            }
            Op::SliceFrom => {
                let [byte_b, byte_c] = get_u8x2!();
                Some(SliceFrom {
                    register: byte_a,
                    value: byte_b,
                    index: byte_c as i8,
                })
            }
            Op::SliceTo => {
                let [byte_b, byte_c] = get_u8x2!();
                Some(SliceTo {
                    register: byte_a,
                    value: byte_b,
                    index: byte_c as i8,
                })
            }
            Op::Index => {
                let [value, index] = get_u8x2!();
                Some(Index {
                    register: byte_a,
                    value,
                    index,
                })
            }
            Op::IndexMut => {
                let [index, value] = get_u8x2!();
                Some(IndexMut {
                    register: byte_a,
                    index,
                    value,
                })
            }
            Op::MapInsert => {
                let [key, value] = get_u8x2!();
                Some(MapInsert {
                    register: byte_a,
                    key,
                    value,
                })
            }
            Op::MetaInsert => {
                let register = byte_a;
                let [meta_id, value] = get_u8x2!();
                if let Ok(id) = meta_id.try_into() {
                    {
                        Some(MetaInsert {
                            register,
                            value,
                            id,
                        })
                    }
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
                let [meta_id, name, value] = get_u8x3!();
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
                let [name, value] = get_u8x2!();
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
            Op::Access => {
                let [value, key_a] = get_u8x2!();
                Some(Access {
                    register: byte_a,
                    value,
                    key: get_var_u32_with_first_byte!(key_a).into(),
                })
            }
            Op::AccessString => {
                let [byte_b, byte_c] = get_u8x2!();
                Some(AccessString {
                    register: byte_a,
                    value: byte_b,
                    key: byte_c,
                })
            }
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
                allow_null: false,
                type_string: get_var_u32!().into(),
            }),
            Op::CheckType => Some(CheckType {
                value: byte_a,
                allow_null: false,
                type_string: get_var_u32!().into(),
                jump_offset: get_u16!(),
            }),
            Op::AssertOptionalType => Some(AssertType {
                value: byte_a,
                allow_null: true,
                type_string: get_var_u32!().into(),
            }),
            Op::CheckOptionalType => Some(CheckType {
                value: byte_a,
                allow_null: true,
                type_string: get_var_u32!().into(),
                jump_offset: get_u16!(),
            }),
            Op::StringStart => Some(StringStart {
                size_hint: get_var_u32_with_first_byte!(byte_a),
            }),
            Op::StringPush => {
                let value = byte_a;
                let flags = get_u8!();

                if flags != 0 {
                    match StringFormatFlags::try_from(flags) {
                        Ok(flags) => {
                            let mut options = StringFormatOptions {
                                alignment: flags.alignment(),
                                ..Default::default()
                            };
                            if flags.has_min_width() {
                                options.min_width = Some(get_var_u32!());
                            }
                            if flags.has_precision() {
                                options.precision = Some(get_var_u32!());
                            }
                            if flags.has_fill_character() {
                                options.fill_character = Some(get_var_u32!().into());
                            }
                            Some(StringPush {
                                value,
                                format_options: Some(options),
                            })
                        }
                        Err(e) => Some(Error { message: e }),
                    }
                } else {
                    Some(StringPush {
                        value,
                        format_options: None,
                    })
                }
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
