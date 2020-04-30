use crate::{RuntimeResult, Value, Vm};
use downcast_rs::impl_downcast;
pub use downcast_rs::Downcast;
use std::{fmt, sync::Arc};

pub trait ExternalValue: fmt::Debug + fmt::Display + Send + Sync + Downcast {
    fn value_type(&self) -> String;
}

impl_downcast!(ExternalValue);

// Once Trait aliases are stabilized this can be simplified a bit,
// see: https://github.com/rust-lang/rust/issues/55628
// TODO: rename to ExternalFunction
#[allow(clippy::type_complexity)]
pub struct ExternalFunction {
    pub function: Arc<dyn Fn(&mut Vm, &[Value]) -> RuntimeResult + Send + Sync + 'static>,
    pub is_instance_function: bool,
}

impl ExternalFunction {
    pub fn new(
        function: impl Fn(&mut Vm, &[Value]) -> RuntimeResult + Send + Sync + 'static,
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
