use {
    crate::{RuntimeResult, Vm},
    downcast_rs::impl_downcast,
    parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard},
    std::{
        fmt,
        hash::{Hash, Hasher},
        sync::Arc,
    },
};

pub use downcast_rs::Downcast;

use crate::MetaMap;

/// A trait for external data
pub trait ExternalData: fmt::Debug + fmt::Display + Send + Sync + Downcast {
    fn value_type(&self) -> String {
        "External Data".to_string()
    }
}

impl_downcast!(ExternalData);

/// A value with data and behaviour defined externally to the Koto runtime
#[derive(Clone, Debug)]
pub struct ExternalValue {
    pub data: Arc<RwLock<dyn ExternalData>>,
    pub meta: Arc<RwLock<MetaMap>>,
}

impl ExternalValue {
    pub fn new(data: impl ExternalData, meta: MetaMap) -> Self {
        Self {
            data: Arc::new(RwLock::new(data)),
            meta: Arc::new(RwLock::new(meta)),
        }
    }

    pub fn with_shared_meta_map(data: impl ExternalData, meta: Arc<RwLock<MetaMap>>) -> Self {
        Self {
            data: Arc::new(RwLock::new(data)),
            meta,
        }
    }

    pub fn with_new_data(&self, data: impl ExternalData) -> Self {
        Self {
            data: Arc::new(RwLock::new(data)),
            meta: self.meta.clone(),
        }
    }

    pub fn data(&self) -> RwLockReadGuard<dyn ExternalData> {
        self.data.read()
    }

    pub fn data_mut(&self) -> RwLockWriteGuard<dyn ExternalData> {
        self.data.write()
    }

    pub fn meta(&self) -> RwLockReadGuard<MetaMap> {
        self.meta.read()
    }
}

// Once Trait aliases are stabilized this can be simplified a bit,
// see: https://github.com/rust-lang/rust/issues/55628
#[allow(clippy::type_complexity)]
pub struct ExternalFunction {
    pub function: Arc<dyn Fn(&mut Vm, &Args) -> RuntimeResult + Send + Sync + 'static>,
    pub is_instance_function: bool,
}

impl ExternalFunction {
    pub fn new(
        function: impl Fn(&mut Vm, &Args) -> RuntimeResult + Send + Sync + 'static,
        is_instance_function: bool,
    ) -> Self {
        Self {
            function: Arc::new(function),
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
        let raw = Arc::into_raw(self.function.clone());
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
        state.write_usize(Arc::as_ptr(&self.function) as *const () as usize);
    }
}

/// The start register and argument count for arguments when an ExternalFunction is called
pub struct Args {
    pub register: u8,
    pub count: u8,
}
