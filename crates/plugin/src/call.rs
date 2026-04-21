use crate::abi;
use crate::{
    KValue, KotoVm, PluginBackend, Result,
    error::{unexpected_args, unexpected_args_after_instance},
};
use koto_api::KotoCallContext;
cfg_select! {
    target_arch = "wasm32" => {
        use crate::types::decode_wasm_value;
        use koto_ffi::wasm;
    }
    _ => {
        use crate::types::decode_value;
    }
}
use std::{
    cell::{Cell, RefCell, UnsafeCell},
    mem, slice,
};

thread_local! {
    static CALL_ARGS_POOL: RefCell<Vec<Vec<KValue>>> = const { RefCell::new(Vec::new()) };
}

fn take_pooled_args() -> Vec<KValue> {
    CALL_ARGS_POOL.with(|pool| pool.borrow_mut().pop().unwrap_or_default())
}

fn return_pooled_args(mut args: Vec<KValue>) {
    args.clear();
    CALL_ARGS_POOL.with(|pool| pool.borrow_mut().push(args));
}

struct CallArgs {
    #[cfg(not(target_arch = "wasm32"))]
    api: *const abi::KotoHostApiV1,
    #[cfg(not(target_arch = "wasm32"))]
    args: *const abi::KValue,
    #[cfg(target_arch = "wasm32")]
    args: *const wasm::KValue,
    arg_count: usize,
    // This owns the pooled buffer so `args()` can fill it lazily through `&self`
    // and then return a plain slice without holding a `RefCell` borrow alive.
    decoded: UnsafeCell<Vec<KValue>>,
    is_decoded: Cell<bool>,
}

impl CallArgs {
    fn as_slice(&self) -> &[KValue] {
        let CallArgs {
            arg_count,
            decoded,
            is_decoded,
            ..
        } = self;

        if !is_decoded.get() {
            let decoded = unsafe { &mut *decoded.get() };
            decoded.clear();
            decoded.reserve(*arg_count);

            if *arg_count == 0 {
                is_decoded.set(true);
                return decoded.as_slice();
            }

            cfg_select! {
                target_arch = "wasm32" => {
                    let arg_handles = unsafe { slice::from_raw_parts(self.args, *arg_count) };
                    for arg in arg_handles {
                        decoded.push(
                            decode_wasm_value(*arg)
                                .expect("plugin wasm call context args were prevalidated before decoding"),
                        );
                    }
                }
                _ => {
                    let api = unsafe { &*self.api };
                    let arg_handles = unsafe { slice::from_raw_parts(self.args, *arg_count) };
                    for arg in arg_handles {
                        decoded.push(
                            decode_value(api, *arg)
                                .expect("plugin call context args were prevalidated before decoding"),
                        );
                    }
                }
            }
            is_decoded.set(true);
        }

        unsafe { &*decoded.get() }
    }
}

impl Drop for CallArgs {
    fn drop(&mut self) {
        let args = mem::take(unsafe { &mut *self.decoded.get() });
        return_pooled_args(args);
    }
}

/// A function call context.
pub struct CallContext {
    /// A VM facade backed by the active host callback.
    pub vm: KotoVm,
    pub(crate) instance: KValue,
    args: CallArgs,
}

impl CallContext {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn from_abi(
        api: &abi::KotoHostApiV1,
        vm: KotoVm,
        instance: KValue,
        args: *const abi::KValue,
        arg_count: usize,
    ) -> Self {
        Self {
            vm,
            instance,
            args: CallArgs {
                #[cfg(not(target_arch = "wasm32"))]
                api: api as *const _,
                args,
                arg_count,
                decoded: UnsafeCell::new(take_pooled_args()),
                is_decoded: Cell::new(false),
            },
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn from_abi(
        _api: &abi::KotoHostApiV1,
        _vm: KotoVm,
        instance: KValue,
        args: *const abi::KValue,
        arg_count: usize,
    ) -> Self {
        Self::from_wasm(instance, args.cast::<wasm::KValue>(), arg_count)
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn from_wasm(instance: KValue, args: *const wasm::KValue, arg_count: usize) -> Self {
        Self {
            vm: KotoVm::from_wasm(),
            instance,
            args: CallArgs {
                args,
                arg_count,
                decoded: UnsafeCell::new(take_pooled_args()),
                is_decoded: Cell::new(false),
            },
        }
    }

    /// Returns the `self` instance used for the call, or [`KValue::Null`] when absent.
    pub fn instance(&self) -> &KValue {
        &self.instance
    }

    /// Returns the call arguments.
    pub fn args(&self) -> &[KValue] {
        self.args.as_slice()
    }

    /// Returns the instance and args with which the function was called.
    ///
    /// If the call didn't provide a usable instance then the first argument is
    /// checked and treated as the instance instead.
    pub fn instance_and_args(
        &self,
        instance_check: impl Fn(&KValue) -> bool,
        expected_args_message: &str,
    ) -> Result<(&KValue, &[KValue])> {
        let args = self.args();
        match (self.instance(), args) {
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

impl KotoCallContext<PluginBackend> for CallContext {
    fn vm(&self) -> &KotoVm {
        &self.vm
    }

    fn vm_mut(&mut self) -> &mut KotoVm {
        &mut self.vm
    }

    fn instance(&self) -> &KValue {
        &self.instance
    }

    fn args(&self) -> &[KValue] {
        self.args()
    }

    fn instance_and_args(
        &self,
        instance_check: impl Fn(&KValue) -> bool,
        expected_args_message: &str,
    ) -> Result<(&KValue, &[KValue])> {
        CallContext::instance_and_args(self, instance_check, expected_args_message)
    }
}
