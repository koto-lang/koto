use {
    crate::prelude::*,
    downcast_rs::impl_downcast,
    std::{
        cell::{Ref, RefMut},
        fmt,
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

impl KotoDisplay for ExternalValue {
    fn display(&self, s: &mut String, vm: &mut Vm, _options: KotoDisplayOptions) -> RuntimeResult {
        use UnaryOp::Display;
        if self.contains_meta_key(&Display.into()) {
            match vm.run_unary_op(Display, self.clone().into())? {
                Value::Str(display_result) => s.push_str(&display_result),
                unexpected => return type_error("String as @display result", &unexpected),
            }
        } else {
            s.push_str(&self.value_type());
        }
        Ok(().into())
    }
}

thread_local! {
    static TYPE_EXTERNAL_VALUE: ValueString = "ExternalValue".into();
}
