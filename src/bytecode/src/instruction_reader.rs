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
    MakeRange {
        register: u8,
        start: u8,
        end: u8,
    },
    MakeRangeInclusive {
        register: u8,
        start: u8,
        end: u8,
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
    JumpBack {
        offset: usize,
    },
    JumpIf {
        register: u8,
        offset: usize,
        jump_condition: bool,
    },
    Call {
        register: u8,
        arg_register: u8,
        arg_count: u8,
    },
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Instruction::*;
        match self {
            Error { .. } => unreachable!(),
            Copy { target, source } => write!(f, "Copy\t\ttarget: {}\tsource: {}", target, source),
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
            MakeRange {
                register,
                start,
                end,
            } => write!(
                f,
                "MakeRange\t\treg: {}\t\tstart: {}\t\tend: {}",
                register, start, end
            ),
            MakeRangeInclusive {
                register,
                start,
                end,
            } => write!(
                f,
                "MakeRangeInclusive\treg: {}\t\tstart: {}\t\tend: {}",
                register, start, end
            ),
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
            JumpBack { offset } => write!(f, "JumpBack\toffset: {}", offset),
            JumpIf {
                register,
                offset,
                jump_condition,
            } => write!(
                f,
                "JumpIf\t\treg: {}\t\toffset: {}\tcondition: {}",
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
            Op::MakeFunction => Some(MakeFunction {
                register: get_byte!(),
                arg_count: get_byte!(),
                size: get_u16!() as usize,
            }),
            Op::MakeRange => Some(MakeRange {
                register: get_byte!(),
                start: get_byte!(),
                end: get_byte!(),
            }),
            Op::MakeRangeInclusive => Some(MakeRangeInclusive {
                register: get_byte!(),
                start: get_byte!(),
                end: get_byte!(),
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
            Op::JumpBack => Some(JumpBack {
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
            Op::Call => Some(Call {
                register: get_byte!(),
                arg_register: get_byte!(),
                arg_count: get_byte!(),
            }),
        }
    }
}
