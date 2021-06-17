use crate::ValueString;

use {
    crate::Value,
    indexmap::IndexMap,
    koto_parser::MetaKeyId,
    rustc_hash::FxHasher,
    std::{
        borrow::Borrow,
        fmt,
        hash::Hasher,
        hash::{BuildHasherDefault, Hash},
        ops::{Deref, DerefMut},
    },
};

type MetaMapType = IndexMap<MetaKey, Value, BuildHasherDefault<FxHasher>>;

#[derive(Clone, Debug, Default)]
pub struct MetaMap(MetaMapType);

impl MetaMap {
    /// Extends the MetaMap with clones of another MetaMap's entries
    #[inline]
    pub fn extend(&mut self, other: &MetaMap) {
        self.0.extend(other.0.clone().into_iter());
    }

    /// Allows access to named entries without having to create a ValueString
    #[inline]
    pub fn get_with_string(&self, key: &str) -> Option<&Value> {
        self.0.get(&key as &dyn AsMetaKeyRef)
    }

    /// Allows access to named entries without having to create a ValueString
    #[inline]
    pub fn get_with_string_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.0.get_mut(&key as &dyn AsMetaKeyRef)
    }
}

impl Deref for MetaMap {
    type Target = MetaMapType;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for MetaMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum MetaKey {
    BinaryOp(BinaryOp),
    UnaryOp(UnaryOp),
    Named(ValueString),
    Test(ValueString),
    Tests,
    PreTest,
    PostTest,
    Type,
}

impl MetaKey {
    fn as_ref(&self) -> MetaKeyRef {
        match &self {
            MetaKey::BinaryOp(op) => MetaKeyRef::BinaryOp(*op),
            MetaKey::UnaryOp(op) => MetaKeyRef::UnaryOp(*op),
            MetaKey::Named(name) => MetaKeyRef::Named(&name),
            MetaKey::Test(name) => MetaKeyRef::Test(&name),
            MetaKey::Tests => MetaKeyRef::Tests,
            MetaKey::PreTest => MetaKeyRef::PreTest,
            MetaKey::PostTest => MetaKeyRef::PostTest,
            MetaKey::Type => MetaKeyRef::Type,
        }
    }
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

pub fn meta_id_to_key(id: MetaKeyId, name: Option<&str>) -> Result<MetaKey, String> {
    use {BinaryOp::*, UnaryOp::*};

    let result = match id {
        MetaKeyId::Add => MetaKey::BinaryOp(Add),
        MetaKeyId::Subtract => MetaKey::BinaryOp(Subtract),
        MetaKeyId::Multiply => MetaKey::BinaryOp(Multiply),
        MetaKeyId::Divide => MetaKey::BinaryOp(Divide),
        MetaKeyId::Modulo => MetaKey::BinaryOp(Modulo),
        MetaKeyId::Less => MetaKey::BinaryOp(Less),
        MetaKeyId::LessOrEqual => MetaKey::BinaryOp(LessOrEqual),
        MetaKeyId::Greater => MetaKey::BinaryOp(Greater),
        MetaKeyId::GreaterOrEqual => MetaKey::BinaryOp(GreaterOrEqual),
        MetaKeyId::Equal => MetaKey::BinaryOp(Equal),
        MetaKeyId::NotEqual => MetaKey::BinaryOp(NotEqual),
        MetaKeyId::Index => MetaKey::BinaryOp(Index),
        MetaKeyId::Negate => MetaKey::UnaryOp(Negate),
        MetaKeyId::Display => MetaKey::UnaryOp(Display),
        MetaKeyId::Named => MetaKey::Named(
            name.ok_or_else(|| "Missing name for named meta entry".to_string())?
                .into(),
        ),
        MetaKeyId::Tests => MetaKey::Tests,
        MetaKeyId::Test => MetaKey::Test(
            name.ok_or_else(|| "Missing name for test".to_string())?
                .into(),
        ),
        MetaKeyId::PreTest => MetaKey::PreTest,
        MetaKeyId::PostTest => MetaKey::PostTest,
        MetaKeyId::Type => MetaKey::Type,
        MetaKeyId::Invalid => return Err("Invalid MetaKeyId".to_string()),
    };

    Ok(result)
}

// Currently only used to support MetaMap::get_with_string()
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum MetaKeyRef<'a> {
    BinaryOp(BinaryOp),
    UnaryOp(UnaryOp),
    Named(&'a str),
    Test(&'a str),
    Tests,
    PreTest,
    PostTest,
    Type,
}

// A trait that allows for allocation-free map accesses with &str
trait AsMetaKeyRef {
    fn as_meta_key_ref(&self) -> MetaKeyRef;
}

impl<'a> Hash for dyn AsMetaKeyRef + 'a {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_meta_key_ref().hash(state);
    }
}

impl<'a> PartialEq for dyn AsMetaKeyRef + 'a {
    fn eq(&self, other: &Self) -> bool {
        self.as_meta_key_ref() == other.as_meta_key_ref()
    }
}

impl<'a> Eq for dyn AsMetaKeyRef + 'a {}

impl AsMetaKeyRef for MetaKey {
    fn as_meta_key_ref(&self) -> MetaKeyRef {
        self.as_ref()
    }
}

// The key part of this whole mechanism; wrap a &str as MetaKeyRef::Named,
// allowing a map search to be performed directly against &str
impl<'a> AsMetaKeyRef for &'a str {
    fn as_meta_key_ref(&self) -> MetaKeyRef {
        MetaKeyRef::Named(self)
    }
}

impl<'a> Borrow<dyn AsMetaKeyRef + 'a> for MetaKey {
    fn borrow(&self) -> &(dyn AsMetaKeyRef + 'a) {
        self
    }
}

impl<'a> Borrow<dyn AsMetaKeyRef + 'a> for &'a str {
    fn borrow(&self) -> &(dyn AsMetaKeyRef + 'a) {
        self
    }
}
