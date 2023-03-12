use {
    crate::{external_function::ArgRegisters, prelude::*},
    std::marker::PhantomData,
};

/// A builder for MetaMaps
///
/// This simplifies adding functions to a [MetaMap], where a common requirement is to work with the
/// [ExternalData] contained in an [External].
///
/// # Example
///
/// ```
/// use koto_runtime::prelude::*;
///
/// #[derive(Clone, Copy, Debug)]
/// struct MyData {
///     x: f64,
/// }
///
/// impl ExternalData for MyData {
///     fn make_copy(&self) -> RcCell<dyn ExternalData> {
///         (*self).into()
///     }
/// }
///
/// let meta_map = MetaMapBuilder::<MyData>::new("my_type")
///     // A 'data function' expects the input value to be an instance of the ExternalData type
///     // provided to the builder.
///     .data_fn("to_number", |data| Ok(data.x.into()))
///     .data_fn(UnaryOp::Display, |data| {
///         Ok(format!("my_type: {}", data.x).into())
///     })
///     // A mutable data function provides a mutable reference to the underlying ExternalData.
///     .data_fn_mut("invert", |data| {
///         data.x *= -1.0;
///         Ok(Value::Null)
///     })
///     // Finally, the build function consumes the builder and provides a MetaMap, ready for
///     // embedding in external values.
///     .build();
/// ```
pub struct MetaMapBuilder<T: ExternalData> {
    // The map that's being built
    map: MetaMap,
    // Keep hold of the type name for error messages
    type_name: ValueString,
    // We want to have T available through the implementation, so it needs to be used by a field
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
    pub fn build(self) -> RcCell<MetaMap> {
        self.map.into()
    }

    /// Adds a function to the `MetaMap`
    ///
    /// The function will be called with the VM and ArgRegisters, and the args themselves need to be
    /// retrieved via vm.get_args(_),
    ///
    /// See the `data_` functions for helpers that provide access to the internal data of an
    /// External, which is often what you want when adding functions to a MetaMap.
    pub fn function<Key, F>(mut self, key: Key, f: F) -> Self
    where
        Key: Into<MetaKey>,
        F: Fn(&mut Vm, &ArgRegisters) -> RuntimeResult + 'static,
    {
        self.map.add_instance_fn(key.into(), f);
        self
    }

    /// Creates an alias for an existing entry
    ///
    /// Currently no error is returned if the requested entry doesn't exist,
    /// this could be revisited if turns out returning an error would be useful.
    pub fn alias<KeySource, KeyTarget>(
        mut self,
        key_source: KeySource,
        key_target: KeyTarget,
    ) -> Self
    where
        KeySource: Into<MetaKey>,
        KeyTarget: Into<MetaKey>,
    {
        if let Some(existing) = self.map.get(&key_source.into()).cloned() {
            self.map.insert(key_target.into(), existing);
        }
        self
    }

    /// Adds a function that takes the External instance as the first argument
    ///
    /// This is useful when the value itself is needed rather than its internal data,
    /// which is useful for self-modifying functions that return self as the result,
    /// like when implementing a builder pattern.
    ///
    /// When only the internal data is needed, see the various `data_` functions.
    pub fn value_fn<Key, F>(mut self, key: Key, f: F) -> Self
    where
        Key: Into<MetaKey>,
        F: Fn(External, &[Value]) -> RuntimeResult + 'static,
    {
        let type_name = self.type_name.clone();

        self.map
            .add_instance_fn(key.into(), move |vm, args| match vm.get_args(args) {
                [Value::External(value), extra_args @ ..]
                    if value.value_type() == type_name && value.has_data::<T>() =>
                {
                    f(value.clone(), extra_args)
                }
                other => unexpected_instance_type(&type_name, other),
            });

        self
    }

    /// Adds a function that provides access to the data contained in an External
    ///
    /// A helper for a function that expects an instance of External as the only argument.
    ///
    /// This is useful when you want access to the External's internal data,
    /// e.g. when implementing a UnaryOp.
    pub fn data_fn<Key, F>(mut self, key: Key, f: F) -> Self
    where
        Key: Into<MetaKey>,
        F: Fn(&T) -> RuntimeResult + 'static,
    {
        let type_name = self.type_name.clone();

        self.map
            .add_instance_fn(key.into(), move |vm, args| match vm.get_args(args) {
                [Value::External(value)] if value.value_type() == type_name => {
                    match value.data::<T>() {
                        Some(data) => f(&data),
                        None => unexpected_data_type(value),
                    }
                }
                other => unexpected_instance_type(&type_name, other),
            });

        self
    }

    /// Adds a function that provides mutable access to the data contained in an External
    ///
    /// A helper for a function that expects an instance of External as the only argument.
    ///
    /// This is useful when you want mutable access to the External's internal data,
    /// e.g. when implementing a UnaryOp, or something like `.reset()` function.
    pub fn data_fn_mut<Key, F>(mut self, key: Key, f: F) -> Self
    where
        Key: Into<MetaKey>,
        F: Fn(&mut T) -> RuntimeResult + 'static,
    {
        let type_name = self.type_name.clone();

        self.map
            .add_instance_fn(key.into(), move |vm, args| match vm.get_args(args) {
                [Value::External(value)] if value.value_type() == type_name => {
                    match value.data_mut::<T>() {
                        Some(mut data) => f(&mut data),
                        None => unexpected_data_type(value),
                    }
                }
                other => unexpected_instance_type(&type_name, other),
            });

        self
    }

    /// Adds a function that takes an External instance, followed by other arguments
    ///
    /// A helper for a function that expects an instance of External as the first argument,
    /// followed by other arguments.
    ///
    /// This is useful when you want access to the internal data of an External,
    /// along with following arguments.
    pub fn data_fn_with_args<Key, F>(mut self, key: Key, f: F) -> Self
    where
        Key: Into<MetaKey>,
        F: Fn(&T, &[Value]) -> RuntimeResult + 'static,
    {
        let type_name = self.type_name.clone();

        self.map
            .add_instance_fn(key.into(), move |vm, args| match vm.get_args(args) {
                [Value::External(value), extra_args @ ..] => match value.data::<T>() {
                    Some(data) => f(&data, extra_args),
                    None => unexpected_data_type(value),
                },
                other => unexpected_instance_type(&type_name, other),
            });

        self
    }

    /// Adds a function that takes an External instance, followed by other arguments
    ///
    /// A helper for a function that expects an instance of External as the first argument,
    /// followed by any other arguments.
    ///
    /// This is useful when you want mutable access to the internal data of an External,
    /// along with following arguments.
    ///
    /// The mutable reference take of the first argument will prevent additional references from
    /// being taken, so if one of the other arguments could be another instance of the first
    /// argument, then value_fn should be used instead.
    pub fn data_fn_with_args_mut<Key, F>(mut self, key: Key, f: F) -> Self
    where
        Key: Into<MetaKey>,
        F: Fn(&mut T, &[Value]) -> RuntimeResult + 'static,
    {
        let type_name = self.type_name.clone();

        self.map
            .add_instance_fn(key.into(), move |vm, args| match vm.get_args(args) {
                [Value::External(value), extra_args @ ..] => match value.data_mut::<T>() {
                    Some(mut data) => f(&mut data, extra_args),
                    None => unexpected_data_type(value),
                },
                other => unexpected_instance_type(&type_name, other),
            });

        self
    }

    /// Adds a function that takes an External instance, along with a shared VM and args
    ///
    /// A helper for a function that expects an instance of External as the first argument,
    /// followed by any other arguments, along with a VM that shares the calling context.
    ///
    /// This is useful when you want mutable access to the internal data of an External,
    /// along with following arguments.
    ///
    /// The mutable reference take of the first argument will prevent additional references from
    /// being taken, so if one of the other arguments could be another instance of the first
    /// argument, then value_fn should be used instead.
    pub fn data_fn_with_vm_mut<Key, F>(mut self, key: Key, f: F) -> Self
    where
        Key: Into<MetaKey>,
        F: Fn(&mut T, &mut Vm, &[Value]) -> RuntimeResult + 'static,
    {
        let type_name = self.type_name.clone();

        self.map
            .add_instance_fn(key.into(), move |vm, args| match vm.get_args(args) {
                [Value::External(value), extra_args @ ..] => match value.data_mut::<T>() {
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

fn unexpected_data_type(unexpected: &External) -> Result<Value, RuntimeError> {
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
