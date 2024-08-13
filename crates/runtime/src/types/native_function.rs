use crate::{error::unexpected_args_after_instance, prelude::*, Ptr, Result};
use std::{
    fmt,
    hash::{Hash, Hasher},
};

/// A trait for native functions used by the Koto runtime
pub trait KotoFunction:
    Fn(&mut CallContext) -> Result<KValue> + KotoSend + KotoSync + 'static
{
}

impl<T> KotoFunction for T where
    T: Fn(&mut CallContext) -> Result<KValue> + KotoSend + KotoSync + 'static
{
}

/// An function that's defined outside of the Koto runtime
///
/// See [KValue::NativeFunction]
pub struct KNativeFunction {
    /// The function implementation that should be called when calling the external function
    //
    // Disable a clippy false positive, see https://github.com/rust-lang/rust-clippy/issues/9299
    // The type signature can't be simplified without stabilized trait aliases,
    // see https://github.com/rust-lang/rust/issues/55628
    #[allow(clippy::type_complexity)]
    pub function: Ptr<dyn KotoFunction>,
}

impl KNativeFunction {
    /// Creates a new external function
    pub fn new(function: impl KotoFunction) -> Self {
        Self {
            function: make_ptr!(function),
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
        write!(f, "external function: {:?}", Ptr::address(&self.function))
    }
}

impl Hash for KNativeFunction {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Ptr::address(&self.function).hash(state)
    }
}

/// The context provided when a call to a [KNativeFunction] is made
///
/// See also: [crate::MethodContext].
#[allow(missing_docs)]
pub struct CallContext<'a> {
    /// The VM making the call
    ///
    /// The VM can be used for operations like [KotoVm::call_function], although
    /// the [CallContext::args] and [CallContext::instance] functions return references,
    /// so the values need to be cloned before mutable operations can be called.
    ///
    /// If a VM needs to be retained after the call, then see [KotoVm::spawn_shared_vm].
    pub vm: &'a mut KotoVm,
    frame_base: u8,
    arg_count: u8,
}

impl<'a> CallContext<'a> {
    /// Returns a new context for calling external functions
    pub fn new(vm: &'a mut KotoVm, frame_base: u8, arg_count: u8) -> Self {
        Self {
            vm,
            frame_base,
            arg_count,
        }
    }

    /// Returns the `self` instance with which the function was called
    pub fn instance(&self) -> &KValue {
        self.vm.get_register(self.frame_base)
    }

    /// Returns the function call's arguments
    pub fn args(&self) -> &[KValue] {
        self.vm.register_slice(self.frame_base + 1, self.arg_count)
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
        instance_check: impl Fn(&KValue) -> bool,
        expected_args_message: &str,
    ) -> Result<(&KValue, &[KValue])> {
        match (self.instance(), self.args()) {
            (instance, args) if instance_check(instance) => Ok((instance, args)),
            (_, [first, rest @ ..]) => {
                if instance_check(first) {
                    Ok((first, rest))
                } else {
                    unexpected_args_after_instance(expected_args_message, first, rest)
                }
            }
            (_, []) => unexpected_args(expected_args_message, &[]),
        }
    }
}
