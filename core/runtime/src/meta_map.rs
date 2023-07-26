use {
    crate::{
        external_function::{ArgRegisters, ExternalFunction},
        prelude::*,
    },
    indexmap::{Equivalent, IndexMap},
    koto_parser::MetaKeyId,
    std::{
        fmt,
        hash::{BuildHasherDefault, Hash},
        ops::{Deref, DerefMut},
    },
};

type MetaMapType = IndexMap<MetaKey, Value, BuildHasherDefault<KotoHasher>>;

/// The meta map used by [ValueMap](crate::ValueMap)
///
/// Each ValueMap contains a metamap, which allows for customized value behaviour by implementing
/// [MetaKeys](crate::MetaKey).
#[derive(Clone, Debug, Default)]
pub struct MetaMap(MetaMapType);

impl MetaMap {
    /// Extends the MetaMap with clones of another MetaMap's entries
    pub fn extend(&mut self, other: &MetaMap) {
        self.0.extend(other.0.clone());
    }

    /// Adds a function to the meta map
    pub fn add_fn(
        &mut self,
        key: MetaKey,
        f: impl Fn(&mut Vm, &ArgRegisters) -> RuntimeResult + 'static,
    ) {
        self.0.insert(
            key,
            Value::ExternalFunction(ExternalFunction::new(f, false)),
        );
    }

    /// Adds an instance function to the meta map
    pub fn add_instance_fn(
        &mut self,
        key: MetaKey,
        f: impl Fn(&mut Vm, &ArgRegisters) -> RuntimeResult + 'static,
    ) {
        self.0
            .insert(key, Value::ExternalFunction(ExternalFunction::new(f, true)));
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

/// The key type used by [MetaMaps](crate::MetaMap)
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum MetaKey {
    /// A binary operation
    ///
    /// e.g. `@+`, `@==`
    BinaryOp(BinaryOp),
    /// A unary operation
    ///
    /// e.g. `@not`
    UnaryOp(UnaryOp),
    /// Function call - `@||`
    ///
    /// Defines the behaviour when performing a function call on the value.
    Call,
    /// A named key
    ///
    /// e.g. `@meta my_named_key`
    ///
    /// Named entries are used in [ValueMaps][crate::ValueMap], so that shared named items can be
    /// made available without them being inserted into the map's contents.
    Named(ValueString),
    /// A test function
    ///
    /// e.g. `@test my_test`
    Test(ValueString),
    /// `@tests`
    ///
    /// Tests are defined together in a [ValueMap](crate::ValueMap).
    Tests,
    /// `@pre_test`
    ///
    /// Used to define a function that will be run before each `@test`.
    PreTest,
    /// `@post_test`
    ///
    /// Used to define a function that will be run after each `@test`.
    PostTest,
    /// `@main`
    ///
    /// Used to define a function that will be run when a module is first imported.
    Main,
    /// `@type`
    ///
    /// Used to define a [ValueString](crate::ValueString) that declare the value's type.
    Type,
    /// `@base`
    ///
    /// Defines a base map to be used as fallback for accesses when a key isn't found.
    Base,
}

impl From<&str> for MetaKey {
    fn from(name: &str) -> Self {
        Self::Named(name.into())
    }
}

impl From<ValueString> for MetaKey {
    fn from(name: ValueString) -> Self {
        Self::Named(name)
    }
}

impl From<UnaryOp> for MetaKey {
    fn from(op: UnaryOp) -> Self {
        Self::UnaryOp(op)
    }
}

impl From<BinaryOp> for MetaKey {
    fn from(op: BinaryOp) -> Self {
        Self::BinaryOp(op)
    }
}

/// The binary operations that can be implemented in a [MetaMap](crate::MetaMap)
///
/// See [MetaKey::BinaryOp]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BinaryOp {
    /// `@+`
    Add,
    /// `@-`
    Subtract,
    /// `@*`
    Multiply,
    /// `@/`
    Divide,
    /// `@%`
    Remainder,
    /// `@+=`
    AddAssign,
    /// `@-=`
    SubtractAssign,
    /// `@*=`
    MultiplyAssign,
    /// `@/=`
    DivideAssign,
    /// `@%=`
    RemainderAssign,
    /// `@<`
    Less,
    /// `@<=`
    LessOrEqual,
    /// `@>`
    Greater,
    /// `@>=`
    GreaterOrEqual,
    /// `@==`
    Equal,
    /// `@!=`
    NotEqual,
    /// `@[]`
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
                Remainder => "%",
                AddAssign => "+=",
                SubtractAssign => "-=",
                MultiplyAssign => "*=",
                DivideAssign => "/=",
                RemainderAssign => "%=",
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

/// The unary operations that can be implemented in a [MetaMap](crate::MetaMap)
///
/// See [MetaKey::UnaryOp]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum UnaryOp {
    /// `@display`
    Display,
    /// `@iterator`
    Iterator,
    /// `@next`
    Next,
    /// `@next_back`
    NextBack,
    /// `@negate`
    Negate,
    /// `@not`
    Not,
}

/// Converts a [MetaKeyId](koto_parser::MetaKeyId) into a [MetaKey]
pub fn meta_id_to_key(id: MetaKeyId, name: Option<ValueString>) -> Result<MetaKey, String> {
    use {BinaryOp::*, UnaryOp::*};

    let result = match id {
        MetaKeyId::Add => MetaKey::BinaryOp(Add),
        MetaKeyId::Subtract => MetaKey::BinaryOp(Subtract),
        MetaKeyId::Multiply => MetaKey::BinaryOp(Multiply),
        MetaKeyId::Divide => MetaKey::BinaryOp(Divide),
        MetaKeyId::Remainder => MetaKey::BinaryOp(Remainder),
        MetaKeyId::AddAssign => MetaKey::BinaryOp(AddAssign),
        MetaKeyId::SubtractAssign => MetaKey::BinaryOp(SubtractAssign),
        MetaKeyId::MultiplyAssign => MetaKey::BinaryOp(MultiplyAssign),
        MetaKeyId::DivideAssign => MetaKey::BinaryOp(DivideAssign),
        MetaKeyId::RemainderAssign => MetaKey::BinaryOp(RemainderAssign),
        MetaKeyId::Less => MetaKey::BinaryOp(Less),
        MetaKeyId::LessOrEqual => MetaKey::BinaryOp(LessOrEqual),
        MetaKeyId::Greater => MetaKey::BinaryOp(Greater),
        MetaKeyId::GreaterOrEqual => MetaKey::BinaryOp(GreaterOrEqual),
        MetaKeyId::Equal => MetaKey::BinaryOp(Equal),
        MetaKeyId::NotEqual => MetaKey::BinaryOp(NotEqual),
        MetaKeyId::Index => MetaKey::BinaryOp(Index),
        MetaKeyId::Iterator => MetaKey::UnaryOp(Iterator),
        MetaKeyId::Next => MetaKey::UnaryOp(Next),
        MetaKeyId::NextBack => MetaKey::UnaryOp(NextBack),
        MetaKeyId::Negate => MetaKey::UnaryOp(Negate),
        MetaKeyId::Not => MetaKey::UnaryOp(Not),
        MetaKeyId::Display => MetaKey::UnaryOp(Display),
        MetaKeyId::Call => MetaKey::Call,
        MetaKeyId::Named => {
            MetaKey::Named(name.ok_or_else(|| "Missing name for named meta entry".to_string())?)
        }
        MetaKeyId::Tests => MetaKey::Tests,
        MetaKeyId::Test => MetaKey::Test(name.ok_or_else(|| "Missing name for test".to_string())?),
        MetaKeyId::PreTest => MetaKey::PreTest,
        MetaKeyId::PostTest => MetaKey::PostTest,
        MetaKeyId::Main => MetaKey::Main,
        MetaKeyId::Type => MetaKey::Type,
        MetaKeyId::Base => MetaKey::Base,
        MetaKeyId::Invalid => return Err("Invalid MetaKeyId".to_string()),
    };

    Ok(result)
}

// Support efficient map lookups with &str
impl Equivalent<MetaKey> for str {
    fn equivalent(&self, other: &MetaKey) -> bool {
        match &other {
            MetaKey::Named(s) => self == s.as_str(),
            _ => false,
        }
    }
}

impl Equivalent<MetaKey> for ValueString {
    fn equivalent(&self, other: &MetaKey) -> bool {
        match &other {
            MetaKey::Named(s) => self == s,
            _ => false,
        }
    }
}
