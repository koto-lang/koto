use {
    crate::{runtime_error, RuntimeResult, Value, ValueKey, ValueMap, Vm},
    downcast_rs::impl_downcast,
    std::{
        fmt,
        hash::{Hash, Hasher},
        sync::Arc,
    },
};

pub use downcast_rs::Downcast;

pub trait ExternalValue: fmt::Debug + fmt::Display + Send + Sync + Downcast {
    fn value_type(&self) -> String;
}

impl_downcast!(ExternalValue);

pub struct Args {
    pub register: u8,
    pub count: u8,
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

pub fn visit_external_value<T>(
    map: &ValueMap,
    mut f: impl FnMut(&mut T) -> RuntimeResult,
) -> RuntimeResult
where
    T: ExternalValue,
{
    match map.data().get(&ValueKey::from(Value::ExternalDataId)) {
        Some(Value::ExternalValue(maybe_external)) => {
            let mut value = maybe_external.as_ref().write();
            match value.downcast_mut::<T>() {
                Some(external) => f(external),
                None => runtime_error!(
                    "Invalid type for external value, found '{}'",
                    value.value_type(),
                ),
            }
        }
        _ => runtime_error!("External value not found"),
    }
}

pub fn is_external_instance<T>(map: &ValueMap) -> bool
where
    T: ExternalValue,
{
    match map.data().get(&ValueKey::from(Value::ExternalDataId)) {
        Some(Value::ExternalValue(maybe_external)) => maybe_external.as_ref().read().is::<T>(),
        _ => false,
    }
}

#[macro_export]
macro_rules! get_external_instance {
    ($args: ident,
     $external_name: expr,
     $fn_name: expr,
     $external_type: ident,
     $match_name: ident,
     $body: block) => {{
        match &$args {
            [Value::Map(instance), ..] => {
                $crate::visit_external_value(instance, |$match_name: &mut $external_type| $body)
            }
            _ => $crate::runtime_error!(
                "{0}.{1}: Expected {0} instance as first argument",
                $external_name,
                $fn_name,
            ),
        }
    }};
}
