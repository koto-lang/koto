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
    static EXTERNAL_DATA_TYPE: ValueString  = "External Data".into();
}

/// A trait for external data
pub trait ExternalData: Downcast {
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
    pub data: Rc<RefCell<dyn ExternalData>>,
    pub meta: Rc<RefCell<MetaMap>>,
}

impl ExternalValue {
    pub fn new(data: impl ExternalData, meta: MetaMap) -> Self {
        Self {
            data: Rc::new(RefCell::new(data)),
            meta: Rc::new(RefCell::new(meta)),
        }
    }

    pub fn with_shared_meta_map(data: impl ExternalData, meta: Rc<RefCell<MetaMap>>) -> Self {
        Self {
            data: Rc::new(RefCell::new(data)),
            meta,
        }
    }

    #[must_use]
    pub fn with_new_data(&self, data: impl ExternalData) -> Self {
        Self {
            data: Rc::new(RefCell::new(data)),
            meta: self.meta.clone(),
        }
    }

    pub fn has_data<T: ExternalData>(&self) -> bool {
        match self.data.try_borrow() {
            Ok(data) => data.downcast_ref::<T>().is_some(),
            Err(_) => false,
        }
    }

    pub fn data<T: ExternalData>(&self) -> Option<Ref<T>> {
        match self.data.try_borrow() {
            Ok(data_ref) => Ref::filter_map(data_ref, |data| data.downcast_ref::<T>()).ok(),
            Err(_) => None,
        }
    }

    pub fn data_mut<T: ExternalData>(&self) -> Option<RefMut<T>> {
        match self.data.try_borrow_mut() {
            Ok(data_ref) => RefMut::filter_map(data_ref, |data| data.downcast_mut::<T>()).ok(),
            Err(_) => None,
        }
    }

    pub fn value_type(&self) -> ValueString {
        match self.get_meta_value(&MetaKey::Type) {
            Some(Value::Str(s)) => s,
            Some(_) => "ERROR: Expected String for @type".into(),
            None => TYPE_EXTERNAL_VALUE.with(|x| x.clone()),
        }
    }

    pub fn data_type(&self) -> ValueString {
        self.data.borrow().data_type()
    }

    ///
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

// Once Trait aliases are stabilized this can be simplified a bit,
// see: https://github.com/rust-lang/rust/issues/55628
#[allow(clippy::type_complexity)]
pub struct ExternalFunction {
    pub function: Rc<dyn Fn(&mut Vm, &ArgRegisters) -> RuntimeResult + 'static>,
    pub is_instance_function: bool,
}

impl ExternalFunction {
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
pub struct ArgRegisters {
    pub register: u8,
    pub count: u8,
}
