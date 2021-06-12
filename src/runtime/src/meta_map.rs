use {
    crate::Value,
    indexmap::IndexMap,
    koto_parser::MetaId,
    rustc_hash::FxHasher,
    std::{
        fmt,
        hash::{BuildHasherDefault, Hash},
    },
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    Equal,
    NotEqual,
    Index,
}

impl fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use BinaryOp::*;

        write!(
            f,
            "{}",
            match self {
                Add => "+",
                Subtract => "-",
                Multiply => "*",
                Divide => "/",
                Modulo => "%",
                Less => "<",
                LessOrEqual => "<=",
                Greater => ">",
                GreaterOrEqual => ">=",
                Equal => "==",
                NotEqual => "!=",
                Index => "[]",
            }
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum UnaryOp {
    Negate,
    Display,
}

impl fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use UnaryOp::*;

        write!(
            f,
            "{}",
            match self {
                Negate => "negate",
                Display => "display",
            }
        )
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum MetaKey {
    BinaryOp(BinaryOp),
    UnaryOp(UnaryOp),
    Type,
}

impl From<BinaryOp> for MetaKey {
    fn from(op: BinaryOp) -> Self {
        Self::BinaryOp(op)
    }
}

impl From<UnaryOp> for MetaKey {
    fn from(op: UnaryOp) -> Self {
        Self::UnaryOp(op)
    }
}

impl From<MetaId> for MetaKey {
    fn from(id: MetaId) -> Self {
        use {BinaryOp::*, UnaryOp::*};

        match id {
            MetaId::Add => MetaKey::BinaryOp(Add),
            MetaId::Subtract => MetaKey::BinaryOp(Subtract),
            MetaId::Multiply => MetaKey::BinaryOp(Multiply),
            MetaId::Divide => MetaKey::BinaryOp(Divide),
            MetaId::Modulo => MetaKey::BinaryOp(Modulo),
            MetaId::Less => MetaKey::BinaryOp(Less),
            MetaId::LessOrEqual => MetaKey::BinaryOp(LessOrEqual),
            MetaId::Greater => MetaKey::BinaryOp(Greater),
            MetaId::GreaterOrEqual => MetaKey::BinaryOp(GreaterOrEqual),
            MetaId::Equal => MetaKey::BinaryOp(Equal),
            MetaId::NotEqual => MetaKey::BinaryOp(NotEqual),
            MetaId::Index => MetaKey::BinaryOp(Index),
            MetaId::Negate => MetaKey::UnaryOp(Negate),
            MetaId::Display => MetaKey::UnaryOp(Display),
            MetaId::Type => MetaKey::Type,
            _ => unreachable!("Invalid MetaId"),
        }
    }
}

pub type MetaMap = IndexMap<MetaKey, Value, BuildHasherDefault<FxHasher>>;
