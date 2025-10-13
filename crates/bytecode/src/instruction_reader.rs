use crate::{Chunk, FunctionFlags, Instruction, Op, StringFormatFlags};
use koto_memory::Ptr;
use koto_parser::{StringFormatOptions, StringFormatRepresentation};
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
            () => {{ get_u8_array!(2) }};
        }
        macro_rules! get_u8x3 {
            () => {{ get_u8_array!(3) }};
        }
        macro_rules! get_u8x4 {
            () => {{ get_u8_array!(4) }};
        }
        macro_rules! get_u8x5 {
            () => {{ get_u8_array!(5) }};
        }
        macro_rules! get_u8x6 {
            () => {{ get_u8_array!(6) }};
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

        let instruction = match op {
            Op::NewFrame => NewFrame {
                register_count: byte_a,
            },
            Op::Copy => Copy {
                target: byte_a,
                source: get_u8!(),
            },
            Op::SetNull => SetNull { register: byte_a },
            Op::SetFalse => SetBool {
                register: byte_a,
                value: false,
            },
            Op::SetTrue => SetBool {
                register: byte_a,
                value: true,
            },
            Op::Set0 => SetNumber {
                register: byte_a,
                value: 0,
            },
            Op::Set1 => SetNumber {
                register: byte_a,
                value: 1,
            },
            Op::SetNumberU8 => SetNumber {
                register: byte_a,
                value: get_u8!() as i64,
            },
            Op::SetNumberNegU8 => SetNumber {
                register: byte_a,
                value: -(get_u8!() as i64),
            },
            Op::LoadFloat => LoadFloat {
                register: byte_a,
                constant: get_var_u32!().into(),
            },
            Op::LoadInt => LoadInt {
                register: byte_a,
                constant: get_var_u32!().into(),
            },
            Op::LoadString => LoadString {
                register: byte_a,
                constant: get_var_u32!().into(),
            },
            Op::LoadNonLocal => LoadNonLocal {
                register: byte_a,
                constant: get_var_u32!().into(),
            },
            Op::ExportValue => ExportValue {
                key: byte_a,
                value: get_u8!(),
            },
            Op::ExportEntry => ExportEntry { entry: byte_a },
            Op::Import => Import { register: byte_a },
            Op::ImportAll => ImportAll { register: byte_a },
            Op::MakeTempTuple => {
                let [byte_b, byte_c] = get_u8x2!();
                MakeTempTuple {
                    register: byte_a,
                    start: byte_b,
                    count: byte_c,
                }
            }
            Op::TempTupleToTuple => TempTupleToTuple {
                register: byte_a,
                source: get_u8!(),
            },
            Op::MakeMap => MakeMap {
                register: byte_a,
                size_hint: get_var_u32!(),
            },
            Op::SequenceStart => SequenceStart {
                size_hint: get_var_u32_with_first_byte!(byte_a),
            },
            Op::SequencePush => SequencePush { value: byte_a },
            Op::SequencePushN => SequencePushN {
                start: byte_a,
                count: get_u8!(),
            },
            Op::SequenceToList => SequenceToList { register: byte_a },
            Op::SequenceToTuple => SequenceToTuple { register: byte_a },
            Op::Range => {
                let [byte_b, byte_c] = get_u8x2!();
                Range {
                    register: byte_a,
                    start: byte_b,
                    end: byte_c,
                }
            }
            Op::RangeInclusive => {
                let [byte_b, byte_c] = get_u8x2!();
                RangeInclusive {
                    register: byte_a,
                    start: byte_b,
                    end: byte_c,
                }
            }
            Op::RangeTo => RangeTo {
                register: byte_a,
                end: get_u8!(),
            },
            Op::RangeToInclusive => RangeToInclusive {
                register: byte_a,
                end: get_u8!(),
            },
            Op::RangeFrom => RangeFrom {
                register: byte_a,
                start: get_u8!(),
            },
            Op::RangeFull => RangeFull { register: byte_a },
            Op::MakeIterator => MakeIterator {
                register: byte_a,
                iterable: get_u8!(),
            },
            Op::Function => {
                let register = byte_a;
                let [
                    arg_count,
                    optional_arg_count,
                    capture_count,
                    flags,
                    size_a,
                    size_b,
                ] = get_u8x6!();
                match FunctionFlags::try_from(flags) {
                    Ok(flags) => {
                        let size = u16::from_le_bytes([size_a, size_b]);

                        Function {
                            register,
                            arg_count,
                            optional_arg_count,
                            capture_count,
                            flags,
                            size,
                        }
                    }
                    Err(e) => Error { message: e },
                }
            }
            Op::Capture => {
                let [byte_b, byte_c] = get_u8x2!();
                Capture {
                    function: byte_a,
                    target: byte_b,
                    source: byte_c,
                }
            }
            Op::Negate => Negate {
                register: byte_a,
                value: get_u8!(),
            },
            Op::Not => Not {
                register: byte_a,
                value: get_u8!(),
            },
            Op::Add => {
                let [byte_b, byte_c] = get_u8x2!();
                Add {
                    register: byte_a,
                    lhs: byte_b,
                    rhs: byte_c,
                }
            }
            Op::Subtract => {
                let [byte_b, byte_c] = get_u8x2!();
                Subtract {
                    register: byte_a,
                    lhs: byte_b,
                    rhs: byte_c,
                }
            }
            Op::Multiply => {
                let [byte_b, byte_c] = get_u8x2!();
                Multiply {
                    register: byte_a,
                    lhs: byte_b,
                    rhs: byte_c,
                }
            }
            Op::Divide => {
                let [byte_b, byte_c] = get_u8x2!();
                Divide {
                    register: byte_a,
                    lhs: byte_b,
                    rhs: byte_c,
                }
            }
            Op::Remainder => {
                let [byte_b, byte_c] = get_u8x2!();
                Remainder {
                    register: byte_a,
                    lhs: byte_b,
                    rhs: byte_c,
                }
            }
            Op::Power => {
                let [byte_b, byte_c] = get_u8x2!();
                Power {
                    register: byte_a,
                    lhs: byte_b,
                    rhs: byte_c,
                }
            }
            Op::AddAssign => AddAssign {
                lhs: byte_a,
                rhs: get_u8!(),
            },
            Op::SubtractAssign => SubtractAssign {
                lhs: byte_a,
                rhs: get_u8!(),
            },
            Op::MultiplyAssign => MultiplyAssign {
                lhs: byte_a,
                rhs: get_u8!(),
            },
            Op::DivideAssign => DivideAssign {
                lhs: byte_a,
                rhs: get_u8!(),
            },
            Op::RemainderAssign => RemainderAssign {
                lhs: byte_a,
                rhs: get_u8!(),
            },
            Op::PowerAssign => PowerAssign {
                lhs: byte_a,
                rhs: get_u8!(),
            },
            Op::Less => {
                let [lhs, rhs] = get_u8x2!();
                Less {
                    register: byte_a,
                    lhs,
                    rhs,
                }
            }
            Op::LessOrEqual => {
                let [lhs, rhs] = get_u8x2!();
                LessOrEqual {
                    register: byte_a,
                    lhs,
                    rhs,
                }
            }
            Op::Greater => {
                let [lhs, rhs] = get_u8x2!();
                Greater {
                    register: byte_a,
                    lhs,
                    rhs,
                }
            }
            Op::GreaterOrEqual => {
                let [lhs, rhs] = get_u8x2!();
                GreaterOrEqual {
                    register: byte_a,
                    lhs,
                    rhs,
                }
            }
            Op::Equal => {
                let [lhs, rhs] = get_u8x2!();
                Equal {
                    register: byte_a,
                    lhs,
                    rhs,
                }
            }
            Op::NotEqual => {
                let [byte_b, byte_c] = get_u8x2!();
                NotEqual {
                    register: byte_a,
                    lhs: byte_b,
                    rhs: byte_c,
                }
            }
            Op::Jump => Jump {
                offset: u16::from_le_bytes([byte_a, get_u8!()]),
            },
            Op::JumpBack => JumpBack {
                offset: u16::from_le_bytes([byte_a, get_u8!()]),
            },
            Op::JumpIfTrue => JumpIfTrue {
                register: byte_a,
                offset: get_u16!(),
            },
            Op::JumpIfFalse => JumpIfFalse {
                register: byte_a,
                offset: get_u16!(),
            },
            Op::JumpIfNull => JumpIfNull {
                register: byte_a,
                offset: get_u16!(),
            },
            Op::Call => {
                let [function, frame_base, arg_count, unpacked_arg_count] = get_u8x4!();
                Call {
                    result: byte_a,
                    function,
                    frame_base,
                    arg_count,
                    packed_arg_count: unpacked_arg_count,
                }
            }
            Op::CallInstance => {
                let [
                    function,
                    instance,
                    frame_base,
                    arg_count,
                    unpacked_arg_count,
                ] = get_u8x5!();
                CallInstance {
                    result: byte_a,
                    function,
                    instance,
                    frame_base,
                    arg_count,
                    packed_arg_count: unpacked_arg_count,
                }
            }
            Op::Return => Return { register: byte_a },
            Op::Yield => Yield { register: byte_a },
            Op::Throw => Throw { register: byte_a },
            Op::Size => Size {
                register: byte_a,
                value: get_u8!(),
            },
            Op::IterNext => {
                let [byte_b, byte_c, byte_d] = get_u8x3!();
                IterNext {
                    result: Some(byte_a),
                    iterator: byte_b,
                    jump_offset: u16::from_le_bytes([byte_c, byte_d]),
                    temporary_output: false,
                }
            }
            Op::IterNextTemp => {
                let [byte_b, byte_c, byte_d] = get_u8x3!();
                IterNext {
                    result: Some(byte_a),
                    iterator: byte_b,
                    jump_offset: u16::from_le_bytes([byte_c, byte_d]),
                    temporary_output: true,
                }
            }
            Op::IterNextQuiet => IterNext {
                result: None,
                iterator: byte_a,
                jump_offset: get_u16!(),
                temporary_output: false,
            },
            Op::IterUnpack => IterNext {
                result: Some(byte_a),
                iterator: get_u8!(),
                jump_offset: 0,
                temporary_output: false,
            },
            Op::TempIndex => {
                let [byte_b, byte_c] = get_u8x2!();
                TempIndex {
                    register: byte_a,
                    value: byte_b,
                    index: byte_c as i8,
                }
            }
            Op::SliceFrom => {
                let [byte_b, byte_c] = get_u8x2!();
                SliceFrom {
                    register: byte_a,
                    value: byte_b,
                    index: byte_c as i8,
                }
            }
            Op::SliceTo => {
                let [byte_b, byte_c] = get_u8x2!();
                SliceTo {
                    register: byte_a,
                    value: byte_b,
                    index: byte_c as i8,
                }
            }
            Op::Index => {
                let [value, index] = get_u8x2!();
                Index {
                    register: byte_a,
                    value,
                    index,
                }
            }
            Op::IndexAssign => {
                let [index, value] = get_u8x2!();
                IndexMut {
                    register: byte_a,
                    index,
                    value,
                }
            }
            Op::AccessAssign => {
                let [key, value] = get_u8x2!();
                AccessAssign {
                    register: byte_a,
                    key,
                    value,
                }
            }
            Op::MetaInsert => {
                let register = byte_a;
                let [meta_id, value] = get_u8x2!();
                if let Ok(id) = meta_id.try_into() {
                    {
                        MetaInsert {
                            register,
                            value,
                            id,
                        }
                    }
                } else {
                    Error {
                        message: format!(
                            "Unexpected meta id {meta_id} found at instruction {op_ip}",
                        ),
                    }
                }
            }
            Op::MetaInsertNamed => {
                let register = byte_a;
                let [meta_id, name, value] = get_u8x3!();
                if let Ok(id) = meta_id.try_into() {
                    MetaInsertNamed {
                        register,
                        value,
                        id,
                        name,
                    }
                } else {
                    Error {
                        message: format!(
                            "Unexpected meta id {meta_id} found at instruction {op_ip}",
                        ),
                    }
                }
            }
            Op::MetaExport => {
                let meta_id = byte_a;
                let value = get_u8!();
                if let Ok(id) = meta_id.try_into() {
                    MetaExport { id, value }
                } else {
                    Error {
                        message: format!(
                            "Unexpected meta id {meta_id} found at instruction {op_ip}",
                        ),
                    }
                }
            }
            Op::MetaExportNamed => {
                let meta_id = byte_a;
                let [name, value] = get_u8x2!();
                if let Ok(id) = meta_id.try_into() {
                    MetaExportNamed { id, value, name }
                } else {
                    Error {
                        message: format!(
                            "Unexpected meta id {meta_id} found at instruction {op_ip}",
                        ),
                    }
                }
            }
            Op::Access => {
                let [value, key_a] = get_u8x2!();
                Access {
                    register: byte_a,
                    value,
                    key: get_var_u32_with_first_byte!(key_a).into(),
                }
            }
            Op::AccessString => {
                let [byte_b, byte_c] = get_u8x2!();
                AccessString {
                    register: byte_a,
                    value: byte_b,
                    key: byte_c,
                }
            }
            Op::TryStart => TryStart {
                arg_register: byte_a,
                catch_offset: get_u16!(),
            },
            Op::TryEnd => TryEnd,
            Op::Debug => Debug {
                register: byte_a,
                constant: get_var_u32!().into(),
            },
            Op::CheckSizeEqual => CheckSizeEqual {
                register: byte_a,
                size: get_u8!() as usize,
            },
            Op::CheckSizeMin => CheckSizeMin {
                register: byte_a,
                size: get_u8!() as usize,
            },
            Op::AssertType => AssertType {
                value: byte_a,
                allow_null: false,
                type_string: get_var_u32!().into(),
            },
            Op::CheckType => CheckType {
                value: byte_a,
                allow_null: false,
                type_string: get_var_u32!().into(),
                jump_offset: get_u16!(),
            },
            Op::AssertOptionalType => AssertType {
                value: byte_a,
                allow_null: true,
                type_string: get_var_u32!().into(),
            },
            Op::CheckOptionalType => CheckType {
                value: byte_a,
                allow_null: true,
                type_string: get_var_u32!().into(),
                jump_offset: get_u16!(),
            },
            Op::StringStart => StringStart {
                size_hint: get_var_u32_with_first_byte!(byte_a),
            },
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
                            if flags.has_representation() {
                                match StringFormatRepresentation::try_from(get_u8!()) {
                                    Ok(representation) => {
                                        options.representation = Some(representation);
                                    }
                                    Err(e) => return Some(Error { message: e }),
                                }
                            }
                            StringPush {
                                value,
                                format_options: Some(options),
                            }
                        }
                        Err(e) => Error { message: e },
                    }
                } else {
                    StringPush {
                        value,
                        format_options: None,
                    }
                }
            }
            Op::StringFinish => StringFinish { register: byte_a },
            _ => Error {
                message: format!("Unexpected opcode {op:?} found at instruction {op_ip}"),
            },
        };

        Some(instruction)
    }
}

#[inline(never)]
fn out_of_bounds_access_error(ip: usize) -> Option<Instruction> {
    Some(Instruction::Error {
        message: format!("Instruction access out of bounds at {ip}"),
    })
}
