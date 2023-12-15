use crate::{prelude::*, Result};
use std::{
    fmt,
    hash::{Hash, Hasher},
    rc::Rc,
};

/// An function that's defined outside of the Koto runtime
///
/// See [Value::NativeFunction]
pub struct KNativeFunction {
    /// The function implementation that should be called when calling the external function
    //
    // Disable a clippy false positive, see https://github.com/rust-lang/rust-clippy/issues/9299
    // The type signature can't be simplified without stabilized trait aliases,
    // see https://github.com/rust-lang/rust/issues/55628
    #[allow(clippy::type_complexity)]
    pub function: Rc<dyn Fn(&mut CallContext) -> Result<Value> + 'static>,
}

impl KNativeFunction {
    /// Creates a new external function
    pub fn new(function: impl Fn(&mut CallContext) -> Result<Value> + 'static) -> Self {
        Self {
            function: Rc::new(function),
        }
    }
}

impl Clone for KNativeFunction {
    fn clone(&self) -> Self {
        Self {
            function: self.function.clone(),
        }
    }
}

impl fmt::Debug for KNativeFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let raw = Rc::into_raw(self.function.clone());
        write!(f, "external function: {raw:?}",)
    }
}

impl Hash for KNativeFunction {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(Rc::as_ptr(&self.function) as *const () as usize);
    }
}

/// The context provided when a call to a [KNativeFunction] is made
#[allow(missing_docs)]
pub struct CallContext<'a> {
    /// The VM making the call
    ///
    /// The VM can be used for operations like [Vm::run_function], although
    /// the [CallContext::args] and [CallContext::instance] functions return references,
    /// so the values need to be cloned before mutable operations can be called.
    ///
    /// If a VM needs to be retained after the call, then see [Vm::spawn_shared_vm].
    pub vm: &'a mut Vm,
    instance_register: Option<u8>,
    arg_register: u8,
    arg_count: u8,
}

impl<'a> CallContext<'a> {
    /// Returns a new context for calling external functions
    pub fn new(
        vm: &'a mut Vm,
        instance_register: Option<u8>,
        arg_register: u8,
        arg_count: u8,
    ) -> Self {
        Self {
            vm,
            instance_register,
            arg_register,
            arg_count,
        }
    }

    /// Returns the `self` instance with which the function was called
    pub fn instance(&self) -> Option<&Value> {
        self.instance_register
            .map(|register| self.vm.get_register(register))
    }

    /// Returns the function call's arguments
    pub fn args(&self) -> &[Value] {
        self.vm.register_slice(self.arg_register, self.arg_count)
    }

    /// Returns the instance and args with which the function was called
    ///
    /// `instance_check` should check the provided value and return true if it is acceptable as an
    /// instance value for the function. If the function was called without an instance (e.g. it's
    /// being called as a standalone function), then the first argument will be checked and returned
    /// as the instance. If no instance is available that passes the check, then an 'expected
    /// arguments' error will be returned with the `expected_args_message`.
    ///
    /// This is used in the core library to allow operations like `list.size()` to be used in method
    /// contexts like `[1, 2, 3].to_tuple()`, or as standalone functions like `to_tuple [1, 2, 3]`.
    pub fn instance_and_args(
        &self,
        instance_check: impl Fn(&Value) -> bool,
        expected_args_message: &str,
    ) -> Result<(&Value, &[Value])> {
        match (self.instance(), self.args()) {
            (Some(instance), args) if instance_check(instance) => Ok((instance, args)),
            (_, [first, rest @ ..]) if instance_check(first) => Ok((first, rest)),
            (_, unexpected_args) => type_error_with_slice(expected_args_message, unexpected_args),
        }
    }
}
