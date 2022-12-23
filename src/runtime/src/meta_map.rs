use {
    crate::{
        external::{ArgRegisters, ExternalFunction},
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

/// The meta map used by [ValueMap](crate::ValueMap) and [ExternalValue](crate::ExternalValue)
///
/// Each ValueMap and ExternalValue contains a metamap,
/// which allows for customized value behaviour by implementing [MetaKeys](crate::MetaKey).
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
    /// A named key
    ///
    /// e.g. `@meta my_named_key`
    ///
    /// This allows for named entries to be included in the meta map,
    /// which is particularly useful in [ExternalValue](crate::ExternalValue) metamaps.
    ///
    /// Named entries also have use in [ValueMaps][crate::ValueMap] where shared named items can be
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
    /// `@negate`
    Negate,
    /// `@not`
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
/// ```
/// use koto_runtime::prelude::*;
///
/// #[derive(Debug)]
/// struct MyData {
///     x: f64,
/// }
///
/// impl ExternalData for MyData {}
///
/// let meta_map = MetaMapBuilder::<MyData>::new("my_type")
///     // A 'data function' expects the input value to be an instance of the ExternalData type
///     // provided to the builder.
///     .data_fn("to_number", |data| Ok(Value::Number(data.x.into())))
///     .data_fn(UnaryOp::Display, |data| {
///         Ok(format!("TestExternalData: {}", data.x).into())
///     })
///     // A mutable data function provides a mutable reference to the underlying ExternalData.
///     .data_fn_mut("invert", |data| {
///         data.x *= -1.0;
///         Ok(Value::Null)
///     })
///     // Finally, the build function consumes the builder and provides a MetaMap, ready for
///     // attaching to external values.
///     .build();
/// ```
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

    /// Adds a function to the `MetaMap`
    ///
    /// The function will be called with the VM and ArgRegisters, and the args themselves need to be
    /// retrieved via vm.get_args(_),
    ///
    /// See the `data_` functions for helpers that provide access to the internal data of an
    /// ExternalValue, which is often what you want when adding functions to a MetaMap.
    pub fn function<Key, F>(mut self, key: Key, f: F) -> Self
    where
        Key: Into<MetaKey>,
        F: Fn(&mut Vm, &ArgRegisters) -> RuntimeResult + 'static,
    {
        self.insert_fn(key.into(), f);
        self
    }

    /// Adds a function that provides the ExternalValue instance
    ///
    /// A helper for a function that expects an instance of ExternalValue as the only argument.
    ///
    /// This is useful when the value itself is needed rather than its internal data.
    /// When the internal data is needed, see the various `data_` functions.
    pub fn external_value_fn<Key, F>(mut self, key: Key, f: F) -> Self
    where
        Key: Into<MetaKey>,
        F: Fn(&ExternalValue, &[Value]) -> RuntimeResult + 'static,
    {
        let type_name = self.type_name.clone();

        self.insert_fn(key.into(), move |vm, args| match vm.get_args(args) {
            [Value::ExternalValue(value), extra_args @ ..] if value.value_type() == type_name => {
                f(value, extra_args)
            }
            other => unexpected_instance_type(&type_name, other),
        });

        self
    }

    /// Adds a function that provides access to the data contained in an ExternalValue
    ///
    /// A helper for a function that expects an instance of ExternalValue as the only argument.
    ///
    /// This is useful when you want access to the ExternalValue's internal data,
    /// e.g. when implementing a UnaryOp.
    pub fn data_fn<Key, F>(mut self, key: Key, f: F) -> Self
    where
        Key: Into<MetaKey>,
        F: Fn(&T) -> RuntimeResult + 'static,
    {
        let type_name = self.type_name.clone();

        self.insert_fn(key.into(), move |vm, args| match vm.get_args(args) {
            [Value::ExternalValue(value)] if value.value_type() == type_name => {
                match value.data::<T>() {
                    Some(data) => f(&data),
                    None => unexpected_data_type(value),
                }
            }
            other => unexpected_instance_type(&type_name, other),
        });

        self
    }

    /// Adds a function that provides mutable access to the data contained in an ExternalValue
    ///
    /// A helper for a function that expects an instance of ExternalValue as the only argument.
    ///
    /// This is useful when you want mutable access to the ExternalValue's internal data,
    /// e.g. when implementing a UnaryOp, or something like `.reset()` function.
    pub fn data_fn_mut<Key, F>(mut self, key: Key, f: F) -> Self
    where
        Key: Into<MetaKey>,
        F: Fn(&mut T) -> RuntimeResult + 'static,
    {
        let type_name = self.type_name.clone();

        self.insert_fn(key.into(), move |vm, args| match vm.get_args(args) {
            [Value::ExternalValue(value)] if value.value_type() == type_name => {
                match value.data_mut::<T>() {
                    Some(mut data) => f(&mut data),
                    None => unexpected_data_type(value),
                }
            }
            other => unexpected_instance_type(&type_name, other),
        });

        self
    }

    /// Adds a function that takes an ExternalValue instance, followed by other arguments
    ///
    /// A helper for a function that expects an instance of ExternalValue as the first argument,
    /// followed by other arguments.
    ///
    /// This is useful when you want access to the internal data of an ExternalValue,
    /// along with following arguments.
    pub fn data_fn_with_args<Key, F>(mut self, key: Key, f: F) -> Self
    where
        Key: Into<MetaKey>,
        F: Fn(&T, &[Value]) -> RuntimeResult + 'static,
    {
        let type_name = self.type_name.clone();

        self.insert_fn(key.into(), move |vm, args| match vm.get_args(args) {
            [Value::ExternalValue(value), extra_args @ ..] => match value.data::<T>() {
                Some(data) => f(&data, extra_args),
                None => unexpected_data_type(value),
            },
            other => unexpected_instance_type(&type_name, other),
        });

        self
    }

    /// Adds a function that takes an ExternalValue instance, followed by other arguments
    ///
    /// A helper for a function that expects an instance of ExternalValue as the first argument,
    /// followed by other arguments.
    ///
    /// This is useful when you want mutable access to the internal data of an ExternalValue,
    /// along with following arguments.
    pub fn data_fn_with_args_mut<Key, F>(mut self, key: Key, f: F) -> Self
    where
        Key: Into<MetaKey>,
        F: Fn(&mut T, &[Value]) -> RuntimeResult + 'static,
    {
        let type_name = self.type_name.clone();

        self.insert_fn(key.into(), move |vm, args| match vm.get_args(args) {
            [Value::ExternalValue(value), extra_args @ ..] => match value.data_mut::<T>() {
                Some(mut data) => f(&mut data, extra_args),
                None => unexpected_data_type(value),
            },
            other => unexpected_instance_type(&type_name, other),
        });

        self
    }

    fn insert_fn(
        &mut self,
        key: MetaKey,
        f: impl Fn(&mut Vm, &ArgRegisters) -> RuntimeResult + 'static,
    ) {
        self.map
            .insert(key, Value::ExternalFunction(ExternalFunction::new(f, true)));
    }
}

fn unexpected_data_type(unexpected: &ExternalValue) -> Result<Value, RuntimeError> {
    runtime_error!("Unexpected external data type: {}", unexpected.data_type(),)
}

fn unexpected_instance_type(
    type_name: &ValueString,
    unexpected: &[Value],
) -> Result<Value, RuntimeError> {
    type_error_with_slice(&format!("'{type_name}'"), unexpected)
}
