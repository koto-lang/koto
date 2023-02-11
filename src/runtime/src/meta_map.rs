use {
    crate::{
        external::{ArgRegisters, ExternalFunction},
        prelude::*,
    },
    indexmap::{Equivalent, IndexMap},
    koto_parser::MetaKeyId,
    std::{
        cell::RefCell,
        fmt,
        hash::{BuildHasherDefault, Hash},
        marker::PhantomData,
        ops::{Deref, DerefMut},
        rc::Rc,
    },
};

type MetaMapType = IndexMap<MetaKey, Value, BuildHasherDefault<KotoHasher>>;

/// The meta map used by [ValueMap](crate::ValueMap) and [ExternalValue](crate::ExternalValue)
///
/// Each ValueMap and ExternalValue contains a metamap,
/// which allows for customized value behaviour by implementing [MetaKeys](crate::MetaKey).
#[derive(Clone, Debug, Default)]
pub struct MetaMap(MetaMapType);

impl MetaMap {
    /// Extends the MetaMap with clones of another MetaMap's entries
    pub fn extend(&mut self, other: &MetaMap) {
        self.0.extend(other.0.clone().into_iter());
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
    /// Function call - `@||`
    ///
    /// Defines the behaviour when performing a function call on the value.
    Call,
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
    pub fn new<U>(type_name: U) -> Self
    where
        ValueString: From<U>,
    {
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
        self.map.add_instance_fn(key.into(), f);
        self
    }

    /// Adds a function that provides the ExternalValue instance
    ///
    /// A helper for a function that expects an instance of ExternalValue as the only argument.
    ///
    /// This is useful when the value itself is needed rather than its internal data.
    /// When the internal data is needed, see the various `data_` functions.
    pub fn value_fn<Key, F>(mut self, key: Key, f: F) -> Self
    where
        Key: Into<MetaKey>,
        F: Fn(&ExternalValue, &[Value]) -> RuntimeResult + 'static,
    {
        let type_name = self.type_name.clone();

        self.map
            .add_instance_fn(key.into(), move |vm, args| match vm.get_args(args) {
                [Value::ExternalValue(value), extra_args @ ..]
                    if value.value_type() == type_name =>
                {
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

        self.map
            .add_instance_fn(key.into(), move |vm, args| match vm.get_args(args) {
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

        self.map
            .add_instance_fn(key.into(), move |vm, args| match vm.get_args(args) {
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

        self.map
            .add_instance_fn(key.into(), move |vm, args| match vm.get_args(args) {
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

        self.map
            .add_instance_fn(key.into(), move |vm, args| match vm.get_args(args) {
                [Value::ExternalValue(value), extra_args @ ..] => match value.data_mut::<T>() {
                    Some(mut data) => f(&mut data, extra_args),
                    None => unexpected_data_type(value),
                },
                other => unexpected_instance_type(&type_name, other),
            });

        self
    }

    /// Adds a function that takes an ExternalValue instance, along with a shared VM and args
    ///
    /// A helper for a function that expects an instance of ExternalValue as the first argument,
    /// followed by other arguments.
    ///
    /// This is useful when you want mutable access to the internal data of an ExternalValue,
    /// along with following arguments.
    pub fn data_fn_with_vm_mut<Key, F>(mut self, key: Key, f: F) -> Self
    where
        Key: Into<MetaKey>,
        F: Fn(&mut T, &mut Vm, &[Value]) -> RuntimeResult + 'static,
    {
        let type_name = self.type_name.clone();

        self.map
            .add_instance_fn(key.into(), move |vm, args| match vm.get_args(args) {
                [Value::ExternalValue(value), extra_args @ ..] => match value.data_mut::<T>() {
                    Some(mut data) => {
                        let mut vm = vm.spawn_shared_vm();
                        f(&mut data, &mut vm, extra_args)
                    }
                    None => unexpected_data_type(value),
                },
                other => unexpected_instance_type(&type_name, other),
            });

        self
    }
}

fn unexpected_data_type(unexpected: &ExternalValue) -> Result<Value, RuntimeError> {
    runtime_error!(
        "Unexpected external data type: {}",
        unexpected.data_type().as_str()
    )
}

fn unexpected_instance_type(
    type_name: &ValueString,
    unexpected: &[Value],
) -> Result<Value, RuntimeError> {
    type_error_with_slice(&format!("'{}'", type_name.as_str()), unexpected)
}
