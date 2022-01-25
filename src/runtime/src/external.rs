use {
    crate::{RuntimeResult, Vm},
    downcast_rs::impl_downcast,
    std::{
        cell::{Ref, RefCell, RefMut},
        fmt,
        hash::{Hash, Hasher},
        rc::Rc,
    },
};

pub use downcast_rs::Downcast;

use crate::MetaMap;

/// A trait for external data
pub trait ExternalData: Downcast {
    fn value_type(&self) -> String {
        "External Data".to_string()
    }
}

impl fmt::Display for dyn ExternalData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.value_type())
    }
}

impl fmt::Debug for dyn ExternalData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.value_type())
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

    pub fn data(&self) -> Ref<dyn ExternalData> {
        self.data.borrow()
    }

    pub fn data_mut(&self) -> RefMut<dyn ExternalData> {
        self.data.borrow_mut()
    }

    pub fn meta(&self) -> Ref<MetaMap> {
        self.meta.borrow()
    }
}

// Once Trait aliases are stabilized this can be simplified a bit,
// see: https://github.com/rust-lang/rust/issues/55628
#[allow(clippy::type_complexity)]
pub struct ExternalFunction {
    pub function: Rc<dyn Fn(&mut Vm, &Args) -> RuntimeResult + 'static>,
    pub is_instance_function: bool,
}

impl ExternalFunction {
    pub fn new(
        function: impl Fn(&mut Vm, &Args) -> RuntimeResult + 'static,
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
            "external {}function: {:?}",
            if self.is_instance_function {
                "instance "
            } else {
                ""
            },
            raw
        )
    }
}

impl Hash for ExternalFunction {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(Rc::as_ptr(&self.function) as *const () as usize);
    }
}

/// The start register and argument count for arguments when an ExternalFunction is called
pub struct Args {
    pub register: u8,
    pub count: u8,
}
