use {
    crate::{
        external::{Args, ExternalFunction},
        runtime_error, ExternalData, ExternalValue, RuntimeResult, Value, ValueString, Vm,
    },
    indexmap::IndexMap,
    koto_parser::MetaKeyId,
    rustc_hash::FxHasher,
    std::{
        borrow::Borrow,
        fmt,
        hash::{BuildHasherDefault, Hash, Hasher},
        ops::{Deref, DerefMut},
    },
};

type MetaMapType = IndexMap<MetaKey, Value, BuildHasherDefault<FxHasher>>;

#[derive(Clone, Debug, Default)]
pub struct MetaMap(MetaMapType);

impl MetaMap {
    /// Initializes a meta map with the given type name
    pub fn with_type_name(name: &str) -> Self {
        let mut map = MetaMapType::default();
        map.insert(MetaKey::Type, name.into());
        Self(map)
    }

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

    /// Adds a function to the map
    pub fn add_fn(&mut self, key: MetaKey, f: impl Fn(&mut Vm, &Args) -> RuntimeResult + 'static) {
        self.0.insert(
            key,
            Value::ExternalFunction(ExternalFunction::new(f, false)),
        );
    }

    /// Adds an instance function to the map
    pub fn add_instance_fn(
        &mut self,
        key: MetaKey,
        f: impl Fn(&mut Vm, &Args) -> RuntimeResult + 'static,
    ) {
        self.0
            .insert(key, Value::ExternalFunction(ExternalFunction::new(f, true)));
    }

    /// Adds a named instance function for external values
    ///
    /// This is a helper for adding simple named instance functions for external values, taking
    /// care of accessing the value's external data, and downcasting to the expected type.
    ///
    /// The added function's first argument is expected to be a reference to the value's external
    /// data instance.
    ///
    /// The second argument is a reference to the containing value itself, which can be useful when
    /// a new instance of the value is to be created.
    ///
    /// The third argument is a slice of any additional arguments that the function is being called
    /// with in the Koto.
    ///
    /// # Example
    ///
    /// meta.add_named_instance_fn(
    ///     "to_number",
    ///     |data: &FooData, _value, _extra_args| Ok(Value::Number(data.x.into())),
    /// );
    pub fn add_named_instance_fn<T, F>(&mut self, fn_name: &str, f: F)
    where
        T: ExternalData,
        F: Fn(&T, &ExternalValue, &[Value]) -> RuntimeResult + 'static,
    {
        let type_name = self.external_type_name();
        let fn_name = fn_name.to_string();

        self.add_instance_fn(
            MetaKey::Named(fn_name.clone().into()),
            move |vm, args| match vm.get_args(args) {
                [Value::ExternalValue(instance_value), extra_args @ ..] => {
                    match instance_value.data().downcast_ref::<T>() {
                        Some(instance_data) => f(instance_data, instance_value, extra_args),
                        None => runtime_error!(
                            "{}.{} - Unexpected external data type: {}",
                            type_name,
                            fn_name,
                            instance_value.data().value_type(),
                        ),
                    }
                }
                _ => runtime_error!(format!(
                    "{type_name}.{fn_name} - Expected {type_name} as argument",
                    type_name = type_name,
                    fn_name = fn_name
                )),
            },
        );
    }

    /// Adds a named instance function for external values, with mutable data access.
    ///
    /// This is a helper for adding simple named instance functions for external values, taking
    /// care of accessing the value's external data, and downcasting to the expected type.
    ///
    /// The added function's first argument is expected to be a mutable reference to the value's
    /// external data instance.
    ///
    /// The second argument is a reference to the containing value itself, which can be useful when
    /// a new instance of the value is to be created.
    ///
    /// The third argument is a slice of any additional arguments that the function is being called
    /// with in the Koto.
    ///
    /// # Example
    ///
    /// meta.add_named_instance_fn(
    ///     "set_to_zero",
    ///     |data: &mut FooData, _value, _extra_args| {
    ///         data.x = 0;
    ///         Ok(Value::Empty),
    ///     }
    /// );
    pub fn add_named_instance_fn_mut<T, F>(&mut self, fn_name: &str, f: F)
    where
        T: ExternalData,
        F: Fn(&mut T, &ExternalValue, &[Value]) -> RuntimeResult + 'static,
    {
        let type_name = self.external_type_name();
        let fn_name = fn_name.to_string();

        self.add_instance_fn(
            MetaKey::Named(fn_name.clone().into()),
            move |vm, args| match vm.get_args(args) {
                [Value::ExternalValue(instance_value), extra_args @ ..] => {
                    match instance_value.data_mut().downcast_mut::<T>() {
                        Some(instance_data) => f(instance_data, instance_value, extra_args),
                        None => runtime_error!(
                            "{}.{} - Unexpected external data type: {}",
                            type_name,
                            fn_name,
                            instance_value.data().value_type(),
                        ),
                    }
                }
                _ => runtime_error!(format!(
                    "{type_name}.{fn_name} - Expected {type_name} as argument",
                    type_name = type_name,
                    fn_name = fn_name
                )),
            },
        );
    }

    /// Adds a unary op function for external values
    ///
    /// This is a helper for adding simple unary op handlers for external values, taking
    /// care of accessing the value's external data, and downcasting to the expected type.
    ///
    /// The added function's first argument is expected to be a reference to the value's
    /// external data instance.
    ///
    /// The second argument is a reference to the containing value itself, which can be useful when
    /// a new instance of the value is to be created.
    ///
    /// # Example
    ///
    /// meta.add_unary_op(UnaryOp::Negate, |data: &FooData, value| {
    ///     let result = value.with_new_data(FooData { x: -data.x });
    ///     Ok(result.into())
    /// });
    pub fn add_unary_op<T, F>(&mut self, op: UnaryOp, f: F)
    where
        T: ExternalData,
        F: Fn(&T, &ExternalValue) -> RuntimeResult + 'static,
    {
        let type_name = self.external_type_name();

        self.add_instance_fn(op.into(), move |vm, args| match vm.get_args(args) {
            [Value::ExternalValue(instance_value)] => {
                match instance_value.data().downcast_ref::<T>() {
                    Some(instance_data) => f(instance_data, instance_value),
                    None => runtime_error!(
                        "{}.@{} - Unexpected external data type: {}",
                        type_name,
                        op,
                        instance_value.data().value_type(),
                    ),
                }
            }
            _ => runtime_error!(format!(
                "{type_name}.@{op} - Expected {type_name} as argument",
                type_name = type_name,
                op = op
            )),
        });
    }

    /// Adds a binary op function for external values
    ///
    /// This is a helper for adding simple binary op handlers for external values, taking
    /// care of accessing the value's external data, and downcasting to the expected type.
    ///
    /// The right side of the binary operation is expected to be another ExternalValue with the same
    /// data type, see [MetaMap::add_binary_op_with_any_rhs] for binary ops with other value types.
    ///
    /// The added function's first and second arguments are expected to be references to the
    /// values' external data instances.
    ///
    /// The third and fourth arguments are references to the containing values themselves, which can
    /// be useful when the result of the operation should be a new instance of the value.
    ///
    /// # Example
    ///
    /// meta.add_binary_op(
    ///     BinaryOp::Add,
    ///     |data_a: &FooData, data_b, value_a, _| {
    ///         let result = value_a.with_new_data(FooData {
    ///             x: data_a.x + data_b.x,
    ///         });
    ///         Ok(result.into())
    ///     },
    /// );
    pub fn add_binary_op<T, F>(&mut self, op: BinaryOp, f: F)
    where
        T: ExternalData,
        F: Fn(&T, &T, &ExternalValue, &ExternalValue) -> RuntimeResult + 'static,
    {
        use Value::ExternalValue;

        let type_name = self.external_type_name();

        self.add_instance_fn(op.into(), move |vm, args| match vm.get_args(args) {
            [ExternalValue(value_a), ExternalValue(value_b)] => {
                match (
                    value_a.data().downcast_ref::<T>(),
                    value_b.data().downcast_ref::<T>(),
                ) {
                    (Some(data_a), Some(data_b)) => f(data_a, data_b, value_a, value_b),
                    _ => runtime_error!(
                        "{}.{} - Unexpected external data types: lhs: {}, rhs: {}",
                        type_name,
                        op,
                        value_a.data().value_type(),
                        value_b.data().value_type(),
                    ),
                }
            }
            _ => runtime_error!(format!(
                "{type_name}.@{op} - Expected two '{type_name}'s as arguments",
                type_name = type_name,
                op = op
            )),
        });
    }

    /// Adds a binary op function for external values
    ///
    /// This is a helper for adding simple binary op handlers for external values, taking
    /// care of accessing the value's external data, and downcasting to the expected type.
    ///
    /// The right side of the binary operation can be any value, see
    /// [MetaMap::add_binary_op] for binary ops with matching external value types.
    ///
    /// The added function's first argument is expected to be a reference to the value's external
    /// data instance.
    ///
    /// The second argument is a reference to the containing value itselt, which can
    /// be useful when the result of the operation should be a new instance of the value.
    ///
    /// The third argument is a reference to the value on the right side of the expression.
    ///
    /// # Example
    ///
    /// meta.add_binary_op_with_any_rhs(
    ///     BinaryOp::Index,
    ///     |data_a: &FooData, _, value_b| match value_b {
    ///         Number(index) => {
    ///             let index = usize::from(index);
    ///             let result = data_a.x + index as f64;
    ///             Ok(Number(result.into()))
    ///         }
    ///         unexpected => runtime_error!(
    ///             "Foo.@Index - Expected Number as argument, found {}",
    ///             unexpected.type_as_string()
    ///         ),
    ///     },
    /// );
    pub fn add_binary_op_with_any_rhs<T, F>(&mut self, op: BinaryOp, f: F)
    where
        T: ExternalData,
        F: Fn(&T, &ExternalValue, &Value) -> RuntimeResult + 'static,
    {
        use Value::ExternalValue;

        let type_name = self.external_type_name();

        self.add_instance_fn(op.into(), move |vm, args| match vm.get_args(args) {
            [ExternalValue(value_a), value_b] => match value_a.data().downcast_ref::<T>() {
                Some(data_a) => f(data_a, value_a, value_b),
                _ => runtime_error!(
                    "{}.{} - Unexpected external data type: {}",
                    type_name,
                    op,
                    value_a.data().value_type(),
                ),
            },
            _ => runtime_error!(format!(
                "{type_name}.@{op} - Expected '{type_name}' and a Value as arguments",
                type_name = type_name,
                op = op
            )),
        });
    }

    fn external_type_name(&self) -> Value {
        self.0
            .get(&MetaKey::Type)
            .cloned()
            .unwrap_or_else(|| "ExternalValue".into())
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
            MetaKey::Named(name) => MetaKeyRef::Named(name),
            MetaKey::Test(name) => MetaKeyRef::Test(name),
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
    Not,
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
                Not => "not",
                Display => "display",
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
        MetaKeyId::Modulo => MetaKey::BinaryOp(Modulo),
        MetaKeyId::Less => MetaKey::BinaryOp(Less),
        MetaKeyId::LessOrEqual => MetaKey::BinaryOp(LessOrEqual),
        MetaKeyId::Greater => MetaKey::BinaryOp(Greater),
        MetaKeyId::GreaterOrEqual => MetaKey::BinaryOp(GreaterOrEqual),
        MetaKeyId::Equal => MetaKey::BinaryOp(Equal),
        MetaKeyId::NotEqual => MetaKey::BinaryOp(NotEqual),
        MetaKeyId::Index => MetaKey::BinaryOp(Index),
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
