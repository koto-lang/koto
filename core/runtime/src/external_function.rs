use crate::{prelude::*, Result};
use std::{
    fmt,
    hash::{Hash, Hasher},
    rc::Rc,
};

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
    pub function: Rc<dyn Fn(&mut Vm, &ArgRegisters) -> Result<Value> + 'static>,
    /// True if the function should behave as an instance function
    pub is_instance_function: bool,
}

impl ExternalFunction {
    /// Creates a new external function
    pub fn new(
        function: impl Fn(&mut Vm, &ArgRegisters) -> Result<Value> + 'static,
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
/// [Vm::get_args] should be called with this struct to retrieve the corresponding
/// slice of [Value]s.
#[allow(missing_docs)]
pub struct ArgRegisters {
    pub register: u8,
    pub count: u8,
}
