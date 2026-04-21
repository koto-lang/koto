#[cfg(feature = "native_host")]
use crate::native_host::transfer::AbiTransfer;
use crate::{Ptr, Result, error::unexpected_args_after_instance, prelude::*};
use koto_api::KotoCallContext;
#[cfg(any(feature = "native_host", test))]
use koto_ffi::native as abi;
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
/// See [`KValue::NativeFunction`]
pub struct KNativeFunction {
    /// The function implementation that should be called when calling the external function
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

#[cfg(feature = "native_host")]
impl AbiTransfer for KNativeFunction {
    type Abi = abi::OpaqueHandle;

    unsafe fn into_abi(self) -> Self::Abi {
        // Safety: `OpaqueHandle` is a repr(C) two-word transport type used only to carry raw fat
        // pointers across the FFI boundary. The layout is verified by the ABI unit test below.
        unsafe {
            std::mem::transmute::<*const dyn KotoFunction, abi::OpaqueHandle>(Ptr::into_raw(
                self.function,
            ))
        }
    }

    unsafe fn from_abi(handle: Self::Abi) -> Self {
        Self {
            // Safety: `handle` originated from `into_abi`, so this reconstructs the exact raw fat
            // pointer that was previously transported as an `OpaqueHandle`.
            function: unsafe {
                Ptr::from_raw(std::mem::transmute::<
                    abi::OpaqueHandle,
                    *const dyn KotoFunction,
                >(handle))
            },
        }
    }

    unsafe fn clone_from_abi(handle: Self::Abi) -> Self {
        Self {
            // Safety: `handle` originated from `into_abi`, so this reconstructs the exact raw fat
            // pointer that was previously transported as an `OpaqueHandle`.
            function: unsafe {
                Ptr::clone_from_raw(std::mem::transmute::<
                    abi::OpaqueHandle,
                    *const dyn KotoFunction,
                >(handle))
            },
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

/// The context provided to [native functions](KNativeFunction) when called
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

impl KotoCallContext<crate::RuntimeBackend> for CallContext<'_> {
    fn vm(&self) -> &KotoVm {
        &*self.vm
    }

    fn vm_mut(&mut self) -> &mut KotoVm {
        self.vm
    }

    fn instance(&self) -> &KValue {
        self.vm.get_register(self.frame_base)
    }

    fn args(&self) -> &[KValue] {
        self.vm.register_slice(self.frame_base + 1, self.arg_count)
    }

    fn instance_and_args(
        &self,
        instance_check: impl Fn(&KValue) -> bool,
        expected_args_message: &str,
    ) -> Result<(&KValue, &[KValue])> {
        CallContext::instance_and_args(self, instance_check, expected_args_message)
    }
}

#[cfg(test)]
mod abi_tests {
    use super::*;
    use std::{
        ffi::c_void,
        mem::{align_of, size_of},
    };

    #[test]
    fn opaque_fat_ptr_matches_native_function_pointer_layout() {
        assert_eq!(
            size_of::<*const dyn KotoFunction>(),
            size_of::<abi::OpaqueHandle>()
        );
        assert_eq!(
            align_of::<*const dyn KotoFunction>(),
            align_of::<abi::OpaqueHandle>()
        );
        assert_eq!(
            size_of::<abi::OpaqueHandle>(),
            size_of::<[*mut c_void; 2]>()
        );
    }
}
