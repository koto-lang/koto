use {
    crate::{MetaKey, MetaMap, RuntimeResult, Value, ValueString, Vm},
    downcast_rs::impl_downcast,
    std::{
        cell::{Ref, RefCell, RefMut},
        fmt,
        hash::{Hash, Hasher},
        rc::Rc,
    },
};

pub use downcast_rs::Downcast;

thread_local! {
    static EXTERNAL_DATA_TYPE: ValueString = "External Data".into();
}

/// A trait for external data
pub trait ExternalData: Downcast {
    /// The type of the ExternalData as a [ValueString]
    fn data_type(&self) -> ValueString {
        EXTERNAL_DATA_TYPE.with(|x| x.clone())
    }
}

impl fmt::Display for dyn ExternalData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.data_type())
    }
}

impl fmt::Debug for dyn ExternalData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.data_type())
    }
}

impl_downcast!(ExternalData);

/// A value with data and behaviour defined externally to the Koto runtime
#[derive(Clone, Debug)]
pub struct ExternalValue {
    /// The [ExternalData] held by the value
    pub data: Rc<RefCell<dyn ExternalData>>,
    /// The [MetaMap] held by the value
    pub meta: Rc<RefCell<MetaMap>>,
}

impl ExternalValue {
    /// Creates a new ExternalValue from [ExternalData] and a [MetaMap]
    ///
    /// Typically you'll want to share the meta map between value instances,
    /// see [ExternalValue::with_shared_meta_map].
    pub fn new(data: impl ExternalData, meta: MetaMap) -> Self {
        Self {
            data: Rc::new(RefCell::new(data)),
            meta: Rc::new(RefCell::new(meta)),
        }
    }

    /// Creates a new ExternalValue from [ExternalData] and a shared [MetaMap]
    pub fn with_shared_meta_map(data: impl ExternalData, meta: Rc<RefCell<MetaMap>>) -> Self {
        Self {
            data: Rc::new(RefCell::new(data)),
            meta,
        }
    }

    /// Creates a new [ExternalValue] with the provided data, cloning the existing [MetaMap]
    #[must_use]
    pub fn with_new_data(&self, data: impl ExternalData) -> Self {
        Self {
            data: Rc::new(RefCell::new(data)),
            meta: self.meta.clone(),
        }
    }

    /// Returns true if the value's data matches the provided type
    pub fn has_data<T: ExternalData>(&self) -> bool {
        match self.data.try_borrow() {
            Ok(data) => data.downcast_ref::<T>().is_some(),
            Err(_) => false,
        }
    }

    /// Returns a reference to the value's data if it matches the provided type
    pub fn data<T: ExternalData>(&self) -> Option<Ref<T>> {
        match self.data.try_borrow() {
            Ok(data_ref) => Ref::filter_map(data_ref, |data| data.downcast_ref::<T>()).ok(),
            Err(_) => None,
        }
    }

    /// Returns a mutable reference to the value's data if it matches the provided type
    pub fn data_mut<T: ExternalData>(&self) -> Option<RefMut<T>> {
        match self.data.try_borrow_mut() {
            Ok(data_ref) => RefMut::filter_map(data_ref, |data| data.downcast_mut::<T>()).ok(),
            Err(_) => None,
        }
    }

    /// Returns the value's type as a [ValueString]
    ///
    /// [MetaKey::Type] will be checked for the type string,
    /// with "ExternalValue" being returned if it's not present.
    pub fn value_type(&self) -> ValueString {
        match self.get_meta_value(&MetaKey::Type) {
            Some(Value::Str(s)) => s,
            Some(_) => "ERROR: Expected String for @type".into(),
            None => TYPE_EXTERNAL_VALUE.with(|x| x.clone()),
        }
    }

    /// Returns the type of the internal [ExternalData]
    pub fn data_type(&self) -> ValueString {
        self.data.borrow().data_type()
    }

    /// Returns true if the value's meta map contains an entry with the given key
    pub fn contains_meta_key(&self, key: &MetaKey) -> bool {
        self.meta.borrow().contains_key(key)
    }

    /// Returns a clone of the meta value corresponding to the given key
    pub fn get_meta_value(&self, key: &MetaKey) -> Option<Value> {
        self.meta.borrow().get(key).cloned()
    }
}

thread_local! {
    static TYPE_EXTERNAL_VALUE: ValueString = "ExternalValue".into();
}

/// An function that's defined outside of the Koto runtime
///
/// See [Value::ExternalFunction]
pub struct ExternalFunction {
    /// The function implementation that should be called when calling the external function
    ///
    ///
    // Once Trait aliases are stabilized this can be simplified a bit,
    // see: https://github.com/rust-lang/rust/issues/55628
    #[allow(clippy::type_complexity)]
    pub function: Rc<dyn Fn(&mut Vm, &ArgRegisters) -> RuntimeResult + 'static>,
    /// True if the function should behave as an instance function
    pub is_instance_function: bool,
}

impl ExternalFunction {
    /// Creates a new external function
    pub fn new(
        function: impl Fn(&mut Vm, &ArgRegisters) -> RuntimeResult + 'static,
        is_instance_function: bool,
    ) -> Self {
        Self {
            function: Rc::new(function),
            is_instance_function,
        }
    }
}

impl Clone for ExternalFunction {
    fn clone(&self) -> Self {
        Self {
            function: self.function.clone(),
            is_instance_function: self.is_instance_function,
        }
    }
}

impl fmt::Debug for ExternalFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let raw = Rc::into_raw(self.function.clone());
        write!(
            f,
            "external {}function: {raw:?}",
            if self.is_instance_function {
                "instance "
            } else {
                ""
            },
        )
    }
}

impl Hash for ExternalFunction {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(Rc::as_ptr(&self.function) as *const () as usize);
    }
}

/// The start register and argument count for arguments when an ExternalFunction is called
///
/// [Vm::args] should be called with this struct to retrieve the corresponding slice of [Value]s.
#[allow(missing_docs)]
pub struct ArgRegisters {
    pub register: u8,
    pub count: u8,
}
