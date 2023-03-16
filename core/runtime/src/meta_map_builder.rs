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
///     fn make_copy(&self) -> PtrMut<dyn ExternalData> {
///         make_data_ptr(*self)
///     }
/// }
///
/// let meta_map = MetaMapBuilder::<MyData>::new("my_type")
///     .function("to_number", |context| Ok(context.data()?.x.into()))
///     .function(UnaryOp::Display, |context| {
///         Ok(format!("my_type: {}", context.data()?.x).into())
///     })
///     .function("invert", |context| {
///         context.data_mut()?.x *= -1.0;
///         context.ok_value()
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
    pub fn build(self) -> PtrMut<MetaMap> {
        self.map.into()
    }

    /// Adds a function to the MetaMap that expects a matching External as the first argument
    ///
    /// If the first argument to the function is an [External] containing [ExternalData] of type T,
    /// then the provided function will be called with the unwrapped External along any additional
    /// arguments.
    pub fn function<Key, F>(self, key: Key, f: F) -> Self
    where
        Key: Into<MetaKey> + Clone,
        F: Fn(MetaFnContext<T>) -> RuntimeResult + Clone + 'static,
    {
        self.function_aliased(&[key], f)
    }

    /// Adds a function to the MetaMap associated with more than one key
    ///
    /// See [Self::function] for more information.
    pub fn function_aliased<Key, F>(mut self, keys: &[Key], f: F) -> Self
    where
        Key: Into<MetaKey> + Clone,
        F: Fn(MetaFnContext<T>) -> RuntimeResult + Clone + 'static,
    {
        let type_name = self.type_name.clone();

        let wrapped_function = move |vm: &mut Vm, args: &ArgRegisters| match vm.get_args(args) {
            [Value::External(value), extra_args @ ..] => {
                f(MetaFnContext::new(value, extra_args, vm))
            }
            other => type_error_with_slice(&format!("'{}'", type_name.as_str()), other),
        };

        for key in keys {
            self.map
                .add_instance_fn(key.clone().into(), wrapped_function.clone());
        }

        self
    }
}

/// Context with helpers for functions passed into [MetaMapBuilder::function]
pub struct MetaFnContext<'a, T> {
    pub external: &'a External,
    pub args: &'a [Value],
    pub vm: &'a Vm,
    _phantom: PhantomData<T>,
}

impl<'a, T: ExternalData> MetaFnContext<'a, T> {
    fn new(external: &'a External, args: &'a [Value], vm: &'a Vm) -> Self {
        Self {
            external,
            args,
            vm,
            _phantom: PhantomData,
        }
    }

    /// Returns a reference to the External's data
    ///
    /// See [Self::data_mut] for more information.
    pub fn data(&self) -> Result<Borrow<T>, RuntimeError> {
        self.external.data::<T>().ok_or_else(|| {
            make_runtime_error!(
                "Failed to access data in meta function (has it already been accessed mutably?)"
            )
        })
    }

    /// Returns a mutable reference to the External's data
    ///
    /// An error will be returned in the following cases:
    ///   - The internal type doesn't match the type that was provided to the builder.
    ///   - The data has already been mutably borrowed.
    ///
    /// Note that the single-access rule for mutable references complicates the implementation of
    /// functions that need to support expressions that have the same value on the LHS and RHS of
    /// the expression, like `x += x`. The contents of the RHS necessary for the expression should
    /// be retrieved before attempting to assign them to the LHS.
    pub fn data_mut(&self) -> Result<BorrowMut<T>, RuntimeError> {
        self.external.data_mut::<T>().ok_or_else(|| {
            make_runtime_error!(
                "Failed to access data in meta function (has it already been accessed mutably?)"
            )
        })
    }

    /// Helper for functions that return the External as the result
    pub fn ok_value(&self) -> RuntimeResult {
        Ok(self.external.clone().into())
    }
}
