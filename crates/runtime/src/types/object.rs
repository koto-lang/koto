#[cfg(any(feature = "native_host", test))]
use crate::KCell;
#[cfg(feature = "native_host")]
use crate::native_host::transfer::AbiTransfer;
use crate::{Borrow, BorrowMut, ErrorKind, KFunction, PtrMut, Result, prelude::*};
use koto_api::{
    KotoAccess, KotoBackend, KotoCopy, KotoMethodContext, KotoObjectCast, KotoObjectOps, KotoType,
};
#[cfg(any(feature = "native_host", test))]
use koto_ffi::native as abi;
use std::{any::Any, fmt, marker::PhantomData};

/// A trait that allows objects to support '.' accesses
///
/// This is the mechanism for attaching custom methods to objects in the Koto runtime.
///
/// A trait for implementing objects that can be added to the Koto runtime
///
/// [`KotoObject`]s are added to the Koto runtime by the [KObject] type, and stored as
/// [`KValue::Object`]s.
///
/// ## Example
///
/// ```
/// use koto_runtime::{derive::*, prelude::*, Result};
///
/// #[derive(Clone, Default, KotoType, KotoCopy)]
/// #[koto(runtime = koto_runtime)]
/// pub struct Foo {
///     data: i32,
/// }
///
/// // The `#[koto_impl]` macro derives an implementation of [KotoAccess] containing wrapper
/// // functions for each impl function tagged with `#[koto_method]`.
/// #[koto_impl(runtime = koto_runtime)]
/// impl Foo {
///     // Simple methods tagged with `#[koto_method]` can use a `&self` argument.
///     #[koto_method(alias = "data")]
///     fn get_data(&self) -> KValue {
///         self.data.into()
///     }
///
///     // An example of a more complex method that makes use of [MethodContext] to return the
///     // instance as the result, which allows for chaining of setter operations.  e.g.:
///     // ```koto
///     // make_foo(42)
///     //  .set_data(99)
///     //  .set_data(-1)
///     //  .get_data()
///     // # -1
///     // ```
///     #[koto_method]
///     fn set_data(ctx: MethodContext<Self>) -> Result<KValue> {
///         match ctx.args {
///             [KValue::Number(n)] => ctx.instance_mut()?.data = n.into(),
///             unexpected => return unexpected_args("|Number|", unexpected),
///         }
///
///         // Return the object instance as the result of the setter operation
///         ctx.instance_result()
///     }
/// }
///
/// impl KotoObjectOps<RuntimeBackend> for Foo {
///     fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
///         ctx.append(format!("Foo({})", self.data));
///         Ok(())
///     }
/// }
/// ```
///
/// See also: [KObject].
pub trait KotoObject:
    KotoObjectOps<RuntimeBackend>
    + KotoType<RuntimeBackend>
    + KotoCopy<RuntimeBackend>
    + KotoAccess<RuntimeBackend>
    + KotoSend
    + KotoSync
    + Any
{
}

impl<T> KotoObject for T where
    T: KotoObjectOps<RuntimeBackend>
        + KotoType<RuntimeBackend>
        + KotoCopy<RuntimeBackend>
        + KotoAccess<RuntimeBackend>
        + KotoSend
        + KotoSync
        + Any
{
}

/// The runtime backend marker used by [`KotoObjectOps`].
pub struct RuntimeBackend;

impl KotoBackend for RuntimeBackend {
    type Error = crate::Error;
    type Value = KValue;
    type Number = KNumber;
    type Range = KRange;
    type String = KString;
    type List = KList;
    type Tuple = KTuple;
    type Map = KMap;
    type Object = KObject;
    type Iterator = KIterator;
    type IteratorOutput = KIteratorOutput;
    type Function = KFunction;
    type NativeFunction = KNativeFunction;
    type Vm = KotoVm;
    type DisplayContext<'a>
        = DisplayContext<'a>
    where
        Self: 'a;
    type CallContext<'a>
        = CallContext<'a>
    where
        Self: 'a;

    fn unimplemented_object_op<T>(
        op: &'static str,
        object_type: Self::String,
    ) -> std::result::Result<T, Self::Error> {
        unimplemented_error(op, object_type)
    }

    fn is_unimplemented_error(error: &Self::Error) -> bool {
        error.is_unimplemented_error()
    }
}

/// A [`KotoObject`] wrapper used in the Koto runtime
#[derive(Clone)]
pub struct KObject {
    handle: PtrMut<dyn KotoObject>,
}

#[allow(missing_docs)]
impl KObject {
    /// Checks if the object is of the given type
    pub fn is_a<T: KotoType<RuntimeBackend> + 'static>(&self) -> bool {
        self.try_borrow()
            .ok()
            .map(|object| (&*object as &dyn Any).is::<T>())
            .unwrap_or(false)
    }

    /// Attempts to borrow the underlying object immutably
    pub fn try_borrow(&self) -> Result<Borrow<'_, dyn KotoObject>> {
        self.handle
            .try_borrow()
            .ok_or_else(|| ErrorKind::UnableToBorrowObject.into())
    }

    /// Attempts to borrow the underlying object mutably
    pub fn try_borrow_mut(&self) -> Result<BorrowMut<'_, dyn KotoObject>> {
        self.handle
            .try_borrow_mut()
            .ok_or_else(|| ErrorKind::UnableToBorrowObject.into())
    }

    /// Attempts to immutably borrow and cast the underlying object to the specified type
    pub fn cast<T: KotoType<RuntimeBackend> + 'static>(&self) -> Result<Borrow<'_, T>> {
        Borrow::filter_map(self.try_borrow()?, |object| {
            (object as &dyn Any).downcast_ref::<T>()
        })
        .map_err(|_| match self.try_borrow() {
            Ok(object) => ErrorKind::UnexpectedObjectType {
                expected: T::type_static(),
                unexpected: KotoType::type_string(&*object),
            }
            .into(),
            Err(e) => e,
        })
    }

    /// Attempts to mutably borrow and cast the underlying object to the specified type
    pub fn cast_mut<T: KotoType<RuntimeBackend> + 'static>(&self) -> Result<BorrowMut<'_, T>> {
        BorrowMut::filter_map(self.try_borrow_mut()?, |object| {
            (object as &mut dyn Any).downcast_mut::<T>()
        })
        .map_err(|_| match self.try_borrow() {
            Ok(object) => ErrorKind::UnexpectedObjectType {
                expected: T::type_static(),
                unexpected: KotoType::type_string(&*object),
            }
            .into(),
            Err(e) => e,
        })
    }

    /// Returns true if the provided object occupies the same memory address
    pub fn is_same_instance(&self, other: &Self) -> bool {
        PtrMut::ptr_eq(&self.handle, &other.handle)
    }

    /// Returns a copy of the object.
    pub fn copy(&self) -> Result<Self> {
        self.handle
            .try_borrow()
            .map(|object| KotoCopy::copy(&*object))
            .ok_or_else(|| ErrorKind::UnableToBorrowObject.into())
    }
}

impl<T> From<T> for KObject
where
    T: KotoObject + 'static,
{
    fn from(object: T) -> Self {
        Self {
            handle: make_ptr_mut!(object),
        }
    }
}

impl fmt::Debug for KObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "KObject ({:?})", PtrMut::address(&self.handle))
    }
}

impl KotoCopy<RuntimeBackend> for KObject {
    fn copy(&self) -> KObject {
        KObject::copy(self).unwrap_or_else(|_| self.clone())
    }
}

#[cfg(feature = "native_host")]
impl AbiTransfer for KObject {
    type Abi = abi::KObject;

    unsafe fn into_abi(self) -> Self::Abi {
        // Safety: `abi::KObject` is a repr(C) opaque fat-handle transport type used only to carry
        // raw object trait pointers across the FFI boundary. The layout is verified by the ABI
        // unit test below.
        unsafe {
            std::mem::transmute::<*const KCell<dyn KotoObject>, abi::KObject>(PtrMut::into_raw(
                self.handle,
            ))
        }
    }

    unsafe fn from_abi(handle: Self::Abi) -> Self {
        Self {
            // Safety: `handle` originated from `into_abi`, so this reconstructs the exact raw fat
            // pointer that was previously transported as an `abi::KObject`.
            handle: unsafe {
                PtrMut::from_raw(std::mem::transmute::<
                    abi::KObject,
                    *const KCell<dyn KotoObject>,
                >(handle))
            },
        }
    }

    unsafe fn clone_from_abi(handle: Self::Abi) -> Self {
        Self {
            // Safety: `handle` originated from `into_abi`, so this reconstructs the exact raw fat
            // pointer that was previously transported as an `abi::KObject`.
            handle: unsafe {
                PtrMut::clone_from_raw(std::mem::transmute::<
                    abi::KObject,
                    *const KCell<dyn KotoObject>,
                >(handle))
            },
        }
    }
}

impl KotoIdentity for KObject {
    fn is_same_instance(&self, other: &Self) -> bool {
        KObject::is_same_instance(self, other)
    }
}

impl KotoObjectCast<RuntimeBackend> for KObject {
    type ObjectRef<'a, T: 'static>
        = Borrow<'a, T>
    where
        Self: 'a;
    type ObjectRefMut<'a, T: 'static>
        = BorrowMut<'a, T>
    where
        Self: 'a;

    fn is_a<T: KotoType<RuntimeBackend> + 'static>(&self) -> bool {
        KObject::is_a::<T>(self)
    }

    fn cast<T: KotoType<RuntimeBackend> + 'static>(&self) -> Result<Self::ObjectRef<'_, T>> {
        KObject::cast::<T>(self)
    }

    fn cast_mut<T: KotoType<RuntimeBackend> + 'static>(
        &mut self,
    ) -> Result<Self::ObjectRefMut<'_, T>> {
        KObject::cast_mut::<T>(self)
    }
}

/// A trait that represents the basic requirements of fields in a type that implements [`KotoObject`]
///
/// This is useful for reducing repetitive duplication in bounds when implementing a generic
/// [KotoObject] type.
pub trait KotoField: Clone + KotoSend + KotoSync + 'static {}
impl<T> KotoField for T where T: Clone + KotoSend + KotoSync + 'static {}

/// Context provided to a function that implements an object method
///
/// This is used by the `#[koto_impl]` macro when generating wrappers for functions tagged with
/// `#[koto_method]`. A native function is called with a [CallContext], and for functions that
/// implement object methods a [MethodContext] is produced when the first call argument is a
/// [KObject].
pub struct MethodContext<'a, T> {
    /// The method call arguments
    pub args: &'a [KValue],
    /// A VM that can be used by the method for operations that require a runtime
    //
    // Q. Why isn't this a mutable reference like in CallContext?
    // A. Because the arguments (including the object instance) have already been retrieved by
    //    reference from the VM, disallowing a mutable reference.
    pub vm: &'a KotoVm,
    // The instance of the object for the method call,
    // accessible via the context's `instance`/`instance_mut` functions
    object: &'a KObject,
    // We want to be able to cast to `T`.
    _phantom: PhantomData<T>,
}

impl<'a, T: KotoObject> MethodContext<'a, T> {
    /// Makes a new method context
    pub fn new(object: &'a KObject, args: &'a [KValue], vm: &'a KotoVm) -> Self {
        Self {
            object,
            args,
            vm,
            _phantom: PhantomData,
        }
    }

    /// Attempts to immutably borrow the object instance
    pub fn instance(&self) -> Result<Borrow<'_, T>> {
        self.object.cast::<T>()
    }

    /// Attempts to mutably borrow the object instance
    pub fn instance_mut(&self) -> Result<BorrowMut<'_, T>> {
        self.object.cast_mut::<T>()
    }

    /// Returns a clone of the instance as a [KValue]
    ///
    /// This is useful for builder methods.
    /// e.g.
    ///
    /// ```koto
    /// make_foo()
    ///   .set_x 99
    ///   .set_y 123
    /// ```
    ///
    /// Here `set_x` and `set_y` would use `instance_result` to allow the builder chain to continue.
    pub fn instance_result(&self) -> Result<KValue> {
        Ok(self.object.clone().into())
    }
}

impl<T: KotoObject> KotoMethodContext<RuntimeBackend> for MethodContext<'_, T> {
    type Instance<'a>
        = Borrow<'a, T>
    where
        Self: 'a;
    type InstanceMut<'a>
        = BorrowMut<'a, T>
    where
        Self: 'a;

    fn vm(&self) -> &KotoVm {
        self.vm
    }

    fn args(&self) -> &[KValue] {
        self.args
    }

    fn instance(&self) -> Result<Self::Instance<'_>> {
        MethodContext::instance(self)
    }

    fn instance_mut(&mut self) -> Result<Self::InstanceMut<'_>> {
        MethodContext::instance_mut(self)
    }

    fn instance_result(&self) -> Result<KValue> {
        MethodContext::instance_result(self)
    }
}

/// Creates an error that describes an unimplemented method
fn unimplemented_error<T>(fn_name: &'static str, object_type: KString) -> Result<T> {
    runtime_error!(ErrorKind::Unimplemented {
        fn_name,
        object_type
    })
}

/// Indicates whether a [`KotoObject`] is iterable.
pub use koto_api::KotoObjectIterable as IsIterable;

#[cfg(test)]
mod abi_tests {
    use super::*;
    use std::{
        ffi::c_void,
        mem::{align_of, size_of},
    };

    #[test]
    fn opaque_object_handle_matches_object_pointer_layout() {
        assert_eq!(
            size_of::<*const KCell<dyn KotoObject>>(),
            size_of::<abi::KObject>()
        );
        assert_eq!(
            align_of::<*const KCell<dyn KotoObject>>(),
            align_of::<abi::KObject>()
        );
        assert_eq!(size_of::<abi::KObject>(), size_of::<[*mut c_void; 2]>());
    }
}
