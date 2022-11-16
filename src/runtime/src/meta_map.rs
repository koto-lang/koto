use {
    crate::{
        external::{Args, ExternalFunction},
        runtime_error, type_error_with_slice, ExternalData, ExternalValue, RuntimeError,
        RuntimeResult, Value, ValueString, Vm,
    },
    indexmap::IndexMap,
    koto_parser::MetaKeyId,
    rustc_hash::FxHasher,
    std::{
        borrow::Borrow,
        cell::RefCell,
        fmt,
        hash::{BuildHasherDefault, Hash, Hasher},
        marker::PhantomData,
        ops::{Deref, DerefMut},
        rc::Rc,
    },
};

type MetaMapType = IndexMap<MetaKey, Value, BuildHasherDefault<FxHasher>>;

#[derive(Clone, Debug, Default)]
pub struct MetaMap(MetaMapType);

impl MetaMap {
    /// Allows access to named entries without having to create a ValueString
    pub fn get_with_string(&self, key: &str) -> Option<&Value> {
        self.0.get(&key as &dyn AsMetaKeyRef)
    }

    /// Allows access to named entries without having to create a ValueString
    pub fn get_with_string_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.0.get_mut(&key as &dyn AsMetaKeyRef)
    }

    /// Extends the MetaMap with clones of another MetaMap's entries
    pub fn extend(&mut self, other: &MetaMap) {
        self.0.extend(other.0.clone().into_iter());
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

impl From<MetaMap> for Rc<RefCell<MetaMap>> {
    fn from(m: MetaMap) -> Self {
        Rc::new(RefCell::new(m))
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
    Main,
    Type,
}

impl MetaKey {
    fn as_ref(&self) -> MetaKeyRef {
        match self {
            MetaKey::BinaryOp(op) => MetaKeyRef::BinaryOp(*op),
            MetaKey::UnaryOp(op) => MetaKeyRef::UnaryOp(*op),
            MetaKey::Named(name) => MetaKeyRef::Named(name),
            MetaKey::Test(name) => MetaKeyRef::Test(name),
            MetaKey::Tests => MetaKeyRef::Tests,
            MetaKey::PreTest => MetaKeyRef::PreTest,
            MetaKey::PostTest => MetaKeyRef::PostTest,
            MetaKey::Main => MetaKeyRef::Main,
            MetaKey::Type => MetaKeyRef::Type,
        }
    }
}

impl fmt::Display for MetaKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MetaKey::BinaryOp(op) => write!(f, "@{op}"),
            MetaKey::UnaryOp(op) => write!(f, "@{op}"),
            MetaKey::Named(name) => write!(f, "{name}"),
            MetaKey::Test(test) => write!(f, "test({test})"),
            MetaKey::Tests => f.write_str("@tests"),
            MetaKey::PreTest => f.write_str("@pre_test"),
            MetaKey::PostTest => f.write_str("@post_test"),
            MetaKey::Main => f.write_str("@main"),
            MetaKey::Type => f.write_str("@type"),
        }
    }
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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
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
                Remainder => "%",
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
    Display,
    Iterator,
    Negate,
    Not,
}

impl fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use UnaryOp::*;

        write!(
            f,
            "{}",
            match self {
                Display => "display",
                Iterator => "iterator",
                Negate => "negate",
                Not => "not",
            }
        )
    }
}

pub fn meta_id_to_key(id: MetaKeyId, name: Option<ValueString>) -> Result<MetaKey, String> {
    use {BinaryOp::*, UnaryOp::*};

    let result = match id {
        MetaKeyId::Add => MetaKey::BinaryOp(Add),
        MetaKeyId::Subtract => MetaKey::BinaryOp(Subtract),
        MetaKeyId::Multiply => MetaKey::BinaryOp(Multiply),
        MetaKeyId::Divide => MetaKey::BinaryOp(Divide),
        MetaKeyId::Remainder => MetaKey::BinaryOp(Remainder),
        MetaKeyId::Less => MetaKey::BinaryOp(Less),
        MetaKeyId::LessOrEqual => MetaKey::BinaryOp(LessOrEqual),
        MetaKeyId::Greater => MetaKey::BinaryOp(Greater),
        MetaKeyId::GreaterOrEqual => MetaKey::BinaryOp(GreaterOrEqual),
        MetaKeyId::Equal => MetaKey::BinaryOp(Equal),
        MetaKeyId::NotEqual => MetaKey::BinaryOp(NotEqual),
        MetaKeyId::Index => MetaKey::BinaryOp(Index),
        MetaKeyId::Iterator => MetaKey::UnaryOp(Iterator),
        MetaKeyId::Negate => MetaKey::UnaryOp(Negate),
        MetaKeyId::Not => MetaKey::UnaryOp(Not),
        MetaKeyId::Display => MetaKey::UnaryOp(Display),
        MetaKeyId::Named => {
            MetaKey::Named(name.ok_or_else(|| "Missing name for named meta entry".to_string())?)
        }
        MetaKeyId::Tests => MetaKey::Tests,
        MetaKeyId::Test => MetaKey::Test(name.ok_or_else(|| "Missing name for test".to_string())?),
        MetaKeyId::PreTest => MetaKey::PreTest,
        MetaKeyId::PostTest => MetaKey::PostTest,
        MetaKeyId::Main => MetaKey::Main,
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
    Main,
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

/// A builder for MetaMaps
///
/// This simplifies adding functions to a [MetaMap], where a common requirement is to work with the
/// [ExternalData] contained in an [ExternalValue].
///
/// # Example
///
/// #[derive(Debug)]
/// struct MyData {
///     x: f64,
/// }
///
/// impl ExternalData for MyData {}
///
/// let meta_map = MetaMapBuilder::<MyData>::new("my_type")
///     # A 'data function' expects the input value to be an instance of the ExternalData type
///     # provided to the builder.
///     .data_fn("to_number", |data| Ok(Value::Number(data.x.into())))
///     .data_fn(UnaryOp::Display, |data| {
///         Ok(format!("TestExternalData: {}", data.x).into())
///     })
///     # A mutable data function provides a mutable reference to the underlying ExternalData.
///     .data_fn_mut("invert", |data| {
///         data.x *= -1.0;
///         Ok(Value::Null)
///     })
///     # Finally, the build function consumes the builder and provides a MetaMap, ready for
///     # attaching to external values.
///     .build();
pub struct MetaMapBuilder<T: ExternalData> {
    // The map that's being built
    map: MetaMap,
    // Keep hold of the type name for error messages
    type_name: ValueString,
    // We want to have T available through the implementation
    _phantom: PhantomData<T>,
}

impl<T: ExternalData> MetaMapBuilder<T> {
    /// Initialize a builder with the given type name
    pub fn new(type_name: &str) -> Self {
        let type_name = ValueString::from(type_name);
        let mut map = MetaMap::default();
        map.insert(MetaKey::Type, Value::Str(type_name.clone()));

        Self {
            map,
            type_name,
            _phantom: PhantomData,
        }
    }

    /// Build the MetaMap, consuming the builder
    pub fn build(self) -> Rc<RefCell<MetaMap>> {
        self.map.into()
    }

    /// Adds an ExternalValue instance function
    ///
    /// A helper for a function that expects an instance of ExternalValue as the only argument,
    /// with internal data that matches the builder's data type.
    ///
    /// The provided function is called with the external value itself, rather than the internal
    /// data. See the `data_` functions for helpers that provide access to the internal data.
    pub fn instance_fn<Key, F>(mut self, key: Key, f: F) -> Self
    where
        Key: Into<MetaKey>,
        F: Fn(&ExternalValue) -> RuntimeResult + 'static,
    {
        let type_name = self.type_name.clone();

        self.add_fn(key.into(), move |vm, args| match vm.get_args(args) {
            [Value::ExternalValue(value)] => match value.data::<T>() {
                Some(_) => f(value),
                None => unexpected_data_type(value),
            },
            other => unexpected_instance_type(&type_name, other),
        });

        self
    }

    /// Adds a function that provides the data contained in an ExternalValue instance
    ///
    /// A helper for a function that expects an instance of ExternalValue as the only argument,
    /// with internal data that matches the builder's data type.
    ///
    /// The provided function is called with a reference to the value's internal data.
    pub fn data_fn<Key, F>(mut self, key: Key, f: F) -> Self
    where
        Key: Into<MetaKey>,
        F: Fn(&T) -> RuntimeResult + 'static,
    {
        let type_name = self.type_name.clone();

        self.add_fn(key.into(), move |vm, args| match vm.get_args(args) {
            [Value::ExternalValue(value)] => match value.data::<T>() {
                Some(data) => f(&data),
                None => unexpected_data_type(value),
            },
            other => unexpected_instance_type(&type_name, other),
        });

        self
    }

    /// Adds a function that provides the data contained in an ExternalValue instance
    ///
    /// A helper for a function that expects an instance of ExternalValue as the only argument,
    /// with internal data that matches the builder's data type.
    ///
    /// The provided function is called with a mutable reference to the value's internal data.
    pub fn data_fn_mut<Key, F>(mut self, key: Key, f: F) -> Self
    where
        Key: Into<MetaKey>,
        F: Fn(&mut T) -> RuntimeResult + 'static,
    {
        let type_name = self.type_name.clone();

        self.add_fn(key.into(), move |vm, args| match vm.get_args(args) {
            [Value::ExternalValue(value)] => match value.data_mut::<T>() {
                Some(mut data) => f(&mut data),
                None => unexpected_data_type(value),
            },
            other => unexpected_instance_type(&type_name, other),
        });

        self
    }

    /// Adds a function that provides the data contained in an ExternalValue instance
    ///
    /// A helper for a function that expects an instance of ExternalValue as the first argument,
    /// with internal data that matches the builder's data type.
    ///
    /// The provided function is called with a reference to the value's internal data,
    /// along with a slice containing any additional arguments.
    pub fn data_fn_with_args<Key, F>(mut self, key: Key, f: F) -> Self
    where
        Key: Into<MetaKey>,
        F: Fn(&T, &[Value]) -> RuntimeResult + 'static,
    {
        let type_name = self.type_name.clone();

        self.add_fn(key.into(), move |vm, args| match vm.get_args(args) {
            [Value::ExternalValue(value), extra_args @ ..] => match value.data::<T>() {
                Some(data) => f(&data, extra_args),
                None => unexpected_data_type(value),
            },
            other => unexpected_instance_type(&type_name, other),
        });

        self
    }

    /// Adds a function that provides the data contained in an ExternalValue instance
    ///
    /// A helper for a function that expects an instance of ExternalValue as the first argument,
    /// with internal data that matches the builder's data type.
    ///
    /// The provided function is called with a mutable reference to the value's internal data,
    /// along with a slice containing any additional arguments.
    pub fn data_fn_with_args_mut<Key, F>(mut self, key: Key, f: F) -> Self
    where
        Key: Into<MetaKey>,
        F: Fn(&mut T, &[Value]) -> RuntimeResult + 'static,
    {
        let type_name = self.type_name.clone();

        self.add_fn(key.into(), move |vm, args| match vm.get_args(args) {
            [Value::ExternalValue(value), extra_args @ ..] => match value.data_mut::<T>() {
                Some(mut data) => f(&mut data, extra_args),
                None => unexpected_data_type(value),
            },
            other => unexpected_instance_type(&type_name, other),
        });

        self
    }

    /// Adds a function that provides the data contained in two ExternalValue instances
    ///
    /// A helper for a function that expects to be called with two ExternalValues with internal data
    /// that matches the builder's data type, e.g. a BinaryOp.
    ///
    /// The provided function is called with references to the internal data of both ExternalValues.
    pub fn data_fn_2<Key, F>(mut self, key: Key, f: F) -> Self
    where
        Key: Into<MetaKey>,
        F: Fn(&T, &T) -> RuntimeResult + 'static,
    {
        let type_name = self.type_name.clone();

        self.add_fn(key.into(), move |vm, args| match vm.get_args(args) {
            [Value::ExternalValue(value_a), Value::ExternalValue(value_b)] => {
                match (value_a.data::<T>(), value_b.data::<T>()) {
                    (Some(data_a), Some(data_b)) => f(&data_a, &data_b),
                    _ => unexpected_data_type_2(value_a, value_b),
                }
            }
            other => unexpected_instance_type_2(&type_name, other),
        });

        self
    }

    /// Adds a function that provides the data contained in two ExternalValue instances
    ///
    /// A helper for a function that expects to be called with two ExternalValues with internal data
    /// that matches the builder's data type, e.g. a BinaryOp.
    ///
    /// The provided function is called with mutable references to the internal data of both
    /// ExternalValues.
    pub fn data_fn_2_mut<Key, F>(mut self, key: Key, f: F) -> Self
    where
        Key: Into<MetaKey>,
        F: Fn(&mut T, &mut T) -> RuntimeResult + 'static,
    {
        let type_name = self.type_name.clone();

        self.add_fn(key.into(), move |vm, args| match vm.get_args(args) {
            [Value::ExternalValue(value_a), Value::ExternalValue(value_b)] => {
                match (value_a.data_mut::<T>(), value_b.data_mut::<T>()) {
                    (Some(mut data_a), Some(mut data_b)) => f(&mut data_a, &mut data_b),
                    _ => unexpected_data_type_2(value_a, value_b),
                }
            }
            other => unexpected_instance_type_2(&type_name, other),
        });

        self
    }

    fn add_fn(&mut self, key: MetaKey, f: impl Fn(&mut Vm, &Args) -> RuntimeResult + 'static) {
        self.map
            .insert(key, Value::ExternalFunction(ExternalFunction::new(f, true)));
    }
}

fn unexpected_data_type(unexpected: &ExternalValue) -> Result<Value, RuntimeError> {
    runtime_error!("Unexpected external data type: {}", unexpected.data_type(),)
}

fn unexpected_data_type_2(
    unexpected_a: &ExternalValue,
    unexpected_b: &ExternalValue,
) -> Result<Value, RuntimeError> {
    runtime_error!(
        "Unexpected external data types: lhs: {}, rhs: {}",
        unexpected_a.data_type(),
        unexpected_b.data_type(),
    )
}

fn unexpected_instance_type(
    type_name: &ValueString,
    unexpected: &[Value],
) -> Result<Value, RuntimeError> {
    type_error_with_slice(&format!("'{type_name}'"), unexpected)
}

fn unexpected_instance_type_2(
    type_name: &ValueString,
    unexpected: &[Value],
) -> Result<Value, RuntimeError> {
    type_error_with_slice(&format!("two '{type_name}'s"), unexpected)
}
