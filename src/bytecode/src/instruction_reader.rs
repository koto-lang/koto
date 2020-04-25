use crate::Op;
use std::{
    convert::{TryFrom, TryInto},
    fmt,
};

#[derive(Debug)]
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
    SetTrue {
        register: u8,
    },
    SetFalse {
        register: u8,
    },
    Return {
        register: u8,
    },
    LoadNumber {
        register: u8,
        constant: usize,
    },
    LoadString {
        register: u8,
        constant: usize,
    },
    LoadGlobal {
        register: u8,
        constant: usize,
    },
    MakeList {
        register: u8,
        size_hint: usize,
    },
    MakeMap {
        register: u8,
        size_hint: usize,
    },
    RangeExclusive {
        register: u8,
        start: u8,
        end: u8,
    },
    RangeInclusive {
        register: u8,
        start: u8,
        end: u8,
    },
    MakeIterator {
        register: u8,
        range: u8,
    },
    MakeFunction {
        register: u8,
        arg_count: u8,
        size: usize,
    },
    Add {
        register: u8,
        lhs: u8,
        rhs: u8,
    },
    Multiply {
        register: u8,
        lhs: u8,
        rhs: u8,
    },
    Less {
        register: u8,
        lhs: u8,
        rhs: u8,
    },
    Greater {
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
        register: u8,
        arg_register: u8,
        arg_count: u8,
    },
    IteratorNext {
        register: u8,
        iterator: u8,
        jump_offset: usize,
    },
    ListPush {
        register: u8,
        value: u8,
    },
    ListUpdate {
        list: u8,
        index: u8,
        value: u8,
    },
    ListIndex {
        register: u8,
        list: u8,
        index: u8,
    },
    MapInsert {
        register: u8,
        key: u8,
        value: u8,
    },
    MapAccess {
        register: u8,
        map: u8,
        key: u8,
    },
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Instruction::*;
        match self {
            Error { .. } => unreachable!(),
            Copy { target, source } => write!(f, "Copy\t\treg: {}\t\tsource: {}", target, source),
            SetEmpty { register } => write!(f, "SetEmpty\treg: {}", register),
            SetTrue { register } => write!(f, "SetTrue\treg: {}", register),
            SetFalse { register } => write!(f, "SetFalse\treg: {}", register),
            Return { register } => write!(f, "Return\t\treg: {}", register),
            LoadNumber { register, constant } => {
                write!(f, "LoadNumber\treg: {}\t\tconstant: {}", register, constant)
            }
            LoadString { register, constant } => {
                write!(f, "LoadString\treg: {}\t\tconstant: {}", register, constant)
            }
            LoadGlobal { register, constant } => {
                write!(f, "LoadGlobal\treg: {}\t\tconstant: {}", register, constant)
            }
            MakeList {
                register,
                size_hint,
            } => write!(f, "MakeList\treg: {}\t\tsize_hint: {}", register, size_hint),
            MakeMap {
                register,
                size_hint,
            } => write!(
                f,
                "MakeMap\t\treg: {}\t\tsize_hint: {}",
                register, size_hint
            ),
            RangeExclusive {
                register,
                start,
                end,
            } => write!(
                f,
                "RangeExclusive\treg: {}\t\tstart: {}\tend: {}",
                register, start, end
            ),
            RangeInclusive {
                register,
                start,
                end,
            } => write!(
                f,
                "RangeInclusive\treg: {}\t\tstart: {}\t\tend: {}",
                register, start, end
            ),
            MakeIterator { register, range } => {
                write!(f, "MakeIterator\treg: {}\t\trange: {}", register, range)
            }
            MakeFunction {
                register,
                arg_count,
                size,
            } => write!(
                f,
                "MakeFunction\treg: {}\t\targ_count: {}\tsize: {}",
                register, arg_count, size
            ),
            Add { register, lhs, rhs } => write!(
                f,
                "Add\t\treg: {}\t\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            Multiply { register, lhs, rhs } => write!(
                f,
                "Multiply\treg: {}\t\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            Less { register, lhs, rhs } => write!(
                f,
                "Less\t\treg: {}\t\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            Greater { register, lhs, rhs } => write!(
                f,
                "Greater\treg: {}\t\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            Equal { register, lhs, rhs } => write!(
                f,
                "Equal\t\treg: {}\t\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            NotEqual { register, lhs, rhs } => write!(
                f,
                "NotEqual\treg: {}\t\tlhs: {}\t\trhs: {}",
                register, lhs, rhs
            ),
            Jump { offset } => write!(f, "Jump\t\toffset: {}", offset),
            JumpIf {
                register,
                offset,
                jump_condition,
            } => write!(
                f,
                "JumpIf\t\treg: {}\t\toffset: {}\tcondition: {}",
                register, offset, jump_condition
            ),
            JumpBack { offset } => write!(f, "JumpBack\toffset: {}", offset),
            JumpBackIf {
                register,
                offset,
                jump_condition,
            } => write!(
                f,
                "JumpBackIf\t\treg: {}\t\toffset: {}\tcondition: {}",
                register, offset, jump_condition
            ),
            Call {
                register,
                arg_register,
                arg_count,
            } => write!(
                f,
                "Call\t\treg: {}\t\targ_reg: {}\targs: {}",
                register, arg_register, arg_count
            ),
            IteratorNext {
                register,
                iterator,
                jump_offset,
            } => write!(
                f,
                "IteratorNext\treg: {}\t\titerator: {}\tjump offset: {}",
                register, iterator, jump_offset
            ),
            ListPush { register, value } => {
                write!(f, "ListPush\treg: {}\t\tvalue: {}", register, value)
            }
            ListUpdate { list, index, value } => write!(
                f,
                "ListUpdate\tlist: {}\t\tindex: {}\t\tvalue: {}",
                list, index, value
            ),
            ListIndex {
                register,
                list,
                index,
            } => write!(
                f,
                "ListInsert\treg: {}\t\tlist: {}\t\tindex: {}",
                register, list, index
            ),
            MapInsert {
                register,
                key,
                value,
            } => write!(
                f,
                "MapInsert\treg: {}\t\tkey: {}\t\tvalue: {}",
                register, key, value
            ),
            MapAccess { register, map, key } => write!(
                f,
                "MapAccess\treg: {}\t\tmap: {}\t\tkey: {}",
                register, map, key
            ),
        }
    }
}

pub struct InstructionReader<'a> {
    bytes: &'a [u8],
    ip: usize,
}

impl<'a> InstructionReader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, ip: 0 }
    }

    pub fn position(&self) -> usize {
        self.ip
    }

    pub fn jump(&mut self, offset: usize) {
        self.ip += offset;
    }

    pub fn jump_back(&mut self, offset: usize) {
        self.ip -= offset;
    }

    pub fn jump_to(&mut self, ip: usize) {
        self.ip = ip
    }
}

impl<'a> Iterator for InstructionReader<'a> {
    type Item = Instruction;

    fn next(&mut self) -> Option<Self::Item> {
        use Instruction::*;

        macro_rules! get_byte {
            () => {{
                match self.bytes.get(self.ip) {
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
                match self.bytes.get(self.ip..self.ip + 2) {
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
                match self.bytes.get(self.ip..self.ip + 4) {
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

        let byte = match self.bytes.get(self.ip) {
            Some(byte) => *byte,
            None => return None,
        };

        let op = match Op::try_from(byte) {
            Ok(op) => op,
            Err(_) => {
                return Some(Error {
                    message: format!(
                        "Unexpected opcode {} found at instruction {}",
                        byte, self.ip
                    ),
                });
            }
        };

        self.ip += 1;

        match op {
            Op::Copy => Some(Copy {
                target: get_byte!(),
                source: get_byte!(),
            }),
            Op::SetEmpty => Some(SetEmpty {
                register: get_byte!(),
            }),
            Op::SetTrue => Some(SetTrue {
                register: get_byte!(),
            }),
            Op::SetFalse => Some(SetFalse {
                register: get_byte!(),
            }),
            Op::Return => Some(Return {
                register: get_byte!(),
            }),
            Op::LoadNumber => Some(LoadNumber {
                register: get_byte!(),
                constant: get_byte!() as usize,
            }),
            Op::LoadNumberLong => Some(LoadNumber {
                register: get_byte!(),
                constant: get_u32!() as usize,
            }),
            Op::LoadString => Some(LoadString {
                register: get_byte!(),
                constant: get_byte!() as usize,
            }),
            Op::LoadStringLong => Some(LoadString {
                register: get_byte!(),
                constant: get_u32!() as usize,
            }),
            Op::LoadGlobal => Some(LoadGlobal {
                register: get_byte!(),
                constant: get_byte!() as usize,
            }),
            Op::LoadGlobalLong => Some(LoadGlobal {
                register: get_byte!(),
                constant: get_u32!() as usize,
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
            Op::RangeExclusive => Some(RangeExclusive {
                register: get_byte!(),
                start: get_byte!(),
                end: get_byte!(),
            }),
            Op::RangeInclusive => Some(RangeInclusive {
                register: get_byte!(),
                start: get_byte!(),
                end: get_byte!(),
            }),
            Op::MakeIterator => Some(MakeIterator {
                register: get_byte!(),
                range: get_byte!(),
            }),
            Op::MakeFunction => Some(MakeFunction {
                register: get_byte!(),
                arg_count: get_byte!(),
                size: get_u16!() as usize,
            }),
            Op::Add => Some(Add {
                register: get_byte!(),
                lhs: get_byte!(),
                rhs: get_byte!(),
            }),
            Op::Multiply => Some(Multiply {
                register: get_byte!(),
                lhs: get_byte!(),
                rhs: get_byte!(),
            }),
            Op::Less => Some(Less {
                register: get_byte!(),
                lhs: get_byte!(),
                rhs: get_byte!(),
            }),
            Op::Greater => Some(Greater {
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
                register: get_byte!(),
                arg_register: get_byte!(),
                arg_count: get_byte!(),
            }),
            Op::IteratorNext => Some(IteratorNext {
                register: get_byte!(),
                iterator: get_byte!(),
                jump_offset: get_u16!() as usize,
            }),
            Op::ListPush => Some(ListPush {
                register: get_byte!(),
                value: get_byte!(),
            }),
            Op::ListUpdate => Some(ListUpdate {
                list: get_byte!(),
                index: get_byte!(),
                value: get_byte!(),
            }),
            Op::ListIndex => Some(ListIndex {
                register: get_byte!(),
                list: get_byte!(),
                index: get_byte!(),
            }),
            Op::MapInsert => Some(MapInsert {
                register: get_byte!(),
                key: get_byte!(),
                value: get_byte!(),
            }),
            Op::MapAccess => Some(MapAccess {
                register: get_byte!(),
                map: get_byte!(),
                key: get_byte!(),
            }),
        }
    }
}
