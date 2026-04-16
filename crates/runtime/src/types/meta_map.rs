use crate::{Error, Result, prelude::*};
use indexmap::IndexMap;
use koto_api::{BinaryOp, MetaKey as SharedMetaKey, ReadOp, UnaryOp, WriteOp};
use koto_parser::MetaKeyId;
use std::{
    hash::BuildHasherDefault,
    ops::{Deref, DerefMut},
};

type MetaMapType = IndexMap<MetaKey, KValue, BuildHasherDefault<KotoHasher>>;

/// The meta key type used by the Koto runtime.
pub type MetaKey = SharedMetaKey<KString>;

/// The meta map used by [KMap](crate::KMap)
///
/// Each KMap contains a metamap, which allows for customized value behaviour by implementing
/// [`MetaKeys`](crate::MetaKey).
#[derive(Clone, Default)]
pub struct MetaMap(MetaMapType);

impl MetaMap {
    /// Extends the MetaMap with clones of another MetaMap's entries
    pub fn extend(&mut self, other: &MetaMap) {
        self.0.extend(other.0.clone());
    }

    /// Adds a function to the meta map
    pub fn add_fn(&mut self, key: MetaKey, f: impl KotoFunction) {
        self.0
            .insert(key, KValue::NativeFunction(KNativeFunction::new(f)));
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

/// Converts a [MetaKeyId](koto_parser::MetaKeyId) into a [MetaKey]
pub fn meta_id_to_key(id: MetaKeyId, name: Option<KString>) -> Result<MetaKey> {
    use {BinaryOp::*, ReadOp::*, UnaryOp::*, WriteOp::*};

    let result = match id {
        MetaKeyId::Index => MetaKey::ReadOp(Index),
        MetaKeyId::Access => MetaKey::ReadOp(Access),
        MetaKeyId::IndexAssign => MetaKey::WriteOp(IndexAssign),
        MetaKeyId::AccessAssign => MetaKey::WriteOp(AccessAssign),
        MetaKeyId::Add => MetaKey::BinaryOp(Add),
        MetaKeyId::Subtract => MetaKey::BinaryOp(Subtract),
        MetaKeyId::Multiply => MetaKey::BinaryOp(Multiply),
        MetaKeyId::Divide => MetaKey::BinaryOp(Divide),
        MetaKeyId::Remainder => MetaKey::BinaryOp(Remainder),
        MetaKeyId::Power => MetaKey::BinaryOp(Power),
        MetaKeyId::AddRhs => MetaKey::BinaryOp(AddRhs),
        MetaKeyId::SubtractRhs => MetaKey::BinaryOp(SubtractRhs),
        MetaKeyId::MultiplyRhs => MetaKey::BinaryOp(MultiplyRhs),
        MetaKeyId::DivideRhs => MetaKey::BinaryOp(DivideRhs),
        MetaKeyId::RemainderRhs => MetaKey::BinaryOp(RemainderRhs),
        MetaKeyId::PowerRhs => MetaKey::BinaryOp(PowerRhs),
        MetaKeyId::AddAssign => MetaKey::BinaryOp(AddAssign),
        MetaKeyId::SubtractAssign => MetaKey::BinaryOp(SubtractAssign),
        MetaKeyId::MultiplyAssign => MetaKey::BinaryOp(MultiplyAssign),
        MetaKeyId::DivideAssign => MetaKey::BinaryOp(DivideAssign),
        MetaKeyId::RemainderAssign => MetaKey::BinaryOp(RemainderAssign),
        MetaKeyId::PowerAssign => MetaKey::BinaryOp(PowerAssign),
        MetaKeyId::Less => MetaKey::BinaryOp(Less),
        MetaKeyId::LessOrEqual => MetaKey::BinaryOp(LessOrEqual),
        MetaKeyId::Greater => MetaKey::BinaryOp(Greater),
        MetaKeyId::GreaterOrEqual => MetaKey::BinaryOp(GreaterOrEqual),
        MetaKeyId::Equal => MetaKey::BinaryOp(Equal),
        MetaKeyId::NotEqual => MetaKey::BinaryOp(NotEqual),
        MetaKeyId::Iterator => MetaKey::UnaryOp(Iterator),
        MetaKeyId::Next => MetaKey::UnaryOp(Next),
        MetaKeyId::NextBack => MetaKey::UnaryOp(NextBack),
        MetaKeyId::Negate => MetaKey::UnaryOp(Negate),
        MetaKeyId::Debug => MetaKey::UnaryOp(Debug),
        MetaKeyId::Display => MetaKey::UnaryOp(Display),
        MetaKeyId::Size => MetaKey::UnaryOp(Size),
        MetaKeyId::Call => MetaKey::Call,
        MetaKeyId::Named => {
            MetaKey::Named(name.ok_or_else(|| Error::from("missing name for named meta entry"))?)
        }
        MetaKeyId::Test => MetaKey::Test(name.ok_or_else(|| Error::from("missing name for test"))?),
        MetaKeyId::PreTest => MetaKey::PreTest,
        MetaKeyId::PostTest => MetaKey::PostTest,
        MetaKeyId::Main => MetaKey::Main,
        MetaKeyId::Type => MetaKey::Type,
        MetaKeyId::Base => MetaKey::Base,
        MetaKeyId::Invalid => return runtime_error!("invalid MetaKeyId"),
    };

    Ok(result)
}
