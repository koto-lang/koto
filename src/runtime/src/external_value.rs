use {
    crate::prelude::*,
    downcast_rs::impl_downcast,
    std::{
        cell::{Ref, RefCell, RefMut},
        fmt,
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

    /// Called by koto.copy, should return a unique copy of the data
    fn make_copy(&self) -> RcCell<dyn ExternalData>;

    /// Called by koto.deep_copy, should return a deep copy of the data
    fn make_deep_copy(&self) -> RcCell<dyn ExternalData> {
        self.make_copy()
    }
}

impl_downcast!(ExternalData);

// Produce an RcCell<dyn External> from a value that implements ExternalData
impl<T: ExternalData> From<T> for RcCell<dyn ExternalData> {
    fn from(value: T) -> Self {
        RcCell::from(Rc::new(RefCell::new(value)) as Rc<RefCell<dyn ExternalData>>)
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

/// A value with data and behaviour defined externally to the Koto runtime
#[derive(Clone, Debug)]
pub struct External {
    /// The [ExternalData] held by the value
    data: RcCell<dyn ExternalData>,
    /// The [MetaMap] held by the value
    meta: RcCell<MetaMap>,
}

impl External {
    /// Creates a new External from [ExternalData] and a [MetaMap]
    ///
    /// Typically you'll want to share the meta map between value instances,
    /// see [External::with_shared_meta_map].
    pub fn new(data: impl ExternalData, meta: MetaMap) -> Self {
        Self {
            data: data.into(),
            meta: meta.into(),
        }
    }

    /// Creates a new External from [ExternalData] and a shared [MetaMap]
    pub fn with_shared_meta_map(data: impl ExternalData, meta: RcCell<MetaMap>) -> Self {
        Self {
            data: data.into(),
            meta,
        }
    }

    /// Creates a new [External] with the provided data, cloning the existing [MetaMap]
    #[must_use]
    pub fn with_new_data(&self, data: impl ExternalData) -> Self {
        Self {
            data: data.into(),
            meta: self.meta.clone(),
        }
    }

    /// Returns a unique copy of the value.
    ///
    /// This is the result of calling [ExternalData::make_copy] on the value's data,
    /// along with a shared clone of the metamap.
    pub fn make_copy(&self) -> Self {
        Self {
            data: self.data.borrow().make_copy(),
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
    /// with "External" being returned if it's not present.
    pub fn value_type(&self) -> ValueString {
        match self.get_meta_value(&MetaKey::Type) {
            Some(Value::Str(s)) => s,
            Some(_) => "ERROR: Expected String for @type".into(),
            None => TYPE_EXTERNAL.with(|x| x.clone()),
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

impl KotoDisplay for External {
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
    static TYPE_EXTERNAL: ValueString = "External".into();
}
