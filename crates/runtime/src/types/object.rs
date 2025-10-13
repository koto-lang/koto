use crate::{Borrow, BorrowMut, ErrorKind, PtrMut, Result, prelude::*};
use std::{any::Any, fmt, marker::PhantomData, ops::Deref};

/// A trait for specifying a Koto object's type
///
/// Using `#[derive(KotoType)]` is recommended.
pub trait KotoType {
    /// The Object's type as a static string
    fn type_static() -> &'static str
    where
        Self: Sized;

    /// The type of the Object as a [KString]
    ///
    /// This should defer to the type returned by [KotoType::type_static],
    /// and will be called whenever the object's type is needed by the runtime,
    /// e.g. when a script calls `koto.type`, so caching the result is a good idea.
    /// `#[derive(KotoType)]` takes care of the details here.
    fn type_string(&self) -> KString;
}

/// A trait for defining how objects should behave when copied in the Koto runtime
///
/// Use `#[derive(KotoCopy)]` for simple objects that don't need a custom implementation of
/// [KotoCopy::deep_copy].
pub trait KotoCopy {
    /// How the object should behave when called from `koto.copy`
    ///
    /// A default implementation can't be provided here, but a typical implementation will look
    /// similar to: `Object::from(self.clone())`
    fn copy(&self) -> KObject;

    /// How the object should behave when called from `koto.deep_copy`
    ///
    /// Deep copies should ensure that deep copies are performed for any Koto values that are owned
    /// by the object (see [KValue::deep_copy]).
    fn deep_copy(&self) -> KObject {
        self.copy()
    }
}

/// A trait that allows objects to support '.' accesses
///
/// This is the mechanism for attaching custom methods to objects in the Koto runtime.
///
/// The `#[koto_impl]` macro provides an easy way to declare methods that should be made available
/// via '.' access by using the `#[koto_method]` attribute, and then derives an appropriate
/// implementation of [KotoEntries].
pub trait KotoEntries {
    /// Returns an optional [KMap] containing entries that can be accessed via the '.' operator.
    ///
    /// Implementations should return a clone of a cached map. `None` is returned by default.
    fn entries(&self) -> Option<KMap> {
        None
    }
}

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
/// // The `#[koto_impl]` macro derives an implementation of [KotoEntries] containing wrapper
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
/// impl KotoObject for Foo {
///     fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
///         ctx.append(format!("Foo({})", self.data));
///         Ok(())
///     }
/// }
/// ```
///
/// See also: [KObject].
pub trait KotoObject: KotoType + KotoCopy + KotoEntries + KotoSend + KotoSync + Any {
    /// Called when the object should be displayed as a string, e.g. by `io.print`
    ///
    /// By default, the object's type is used as the display string.
    ///
    /// The [`DisplayContext`] is used to append strings to the result, and provides information
    /// about how the contents should be formatted,
    /// e.g. the value is in a container, or the result should be displayed with debug information.
    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(self.type_string());
        Ok(())
    }

    /// Called for indexing operations, e.g. `x[0]`
    ///
    /// See also: [KotoObject::size]
    fn index(&self, index: &KValue) -> Result<KValue> {
        let _ = index;
        unimplemented_error("@index", self.type_string())
    }

    /// Called when mutating an object via indexing, e.g. `x[0] = 99`
    ///
    /// See also: [KotoObject::size]
    fn index_mut(&mut self, index: &KValue, value: &KValue) -> Result<()> {
        let _ = (index, value);
        unimplemented_error("@index_mut", self.type_string())
    }

    /// Called when checking for the number of elements contained in the object
    ///
    /// The size should represent the maximum valid index that can be passed to
    /// [`KotoObject::index`].
    ///
    /// The runtime defers to this function when the 'size' of an object is needed,
    /// e.g. when `koto.size` is called, or when unpacking function arguments.
    ///
    /// The `Indexable` type hint will pass for objects with a defined size.
    ///
    /// See also: [`KotoObject::index`]
    fn size(&self) -> Option<usize> {
        None
    }

    /// Declares to the runtime whether or not the object is callable
    ///
    /// The `Callable` type hint defers to the function, expecting `true` to be returned for objects
    /// that implement [`KotoObject::call`].
    fn is_callable(&self) -> bool {
        false
    }

    /// Allows the object to behave as a function
    ///
    /// Objects that implement `call` should return `true` from [`KotoObject::is_callable`].
    fn call(&mut self, ctx: &mut CallContext) -> Result<KValue> {
        let _ = ctx;
        unimplemented_error("@||", self.type_string())
    }

    /// Defines the behavior of negation (e.g. `-x`)
    fn negate(&self) -> Result<KValue> {
        unimplemented_error("@negate", self.type_string())
    }

    /// The `+` addition operator
    ///
    /// This will be called by the runtime when the object is on the LHS.
    ///
    /// To specialize the behaviour of `+` when the object is on the RHS, see [Self::add_rhs].
    fn add(&self, other: &KValue) -> Result<KValue> {
        let _ = other;
        unimplemented_error("@+", self.type_string())
    }

    /// The `+` addition operator when the object is on the RHS
    ///
    /// This will be called when the value on the LHS doesn't implement the operation.
    fn add_rhs(&self, other: &KValue) -> Result<KValue> {
        let _ = other;
        unimplemented_error("@+", self.type_string())
    }

    /// The `-` subtraction operator
    ///
    /// This will be called by the runtime when the object is on the LHS of the operation,
    /// or as a fallback if the value on the LHS doesn't support the operation.
    ///
    /// To specialize the behaviour of `-` when the object is on the RHS, see [Self::subtract_rhs].
    fn subtract(&self, other: &KValue) -> Result<KValue> {
        let _ = other;
        unimplemented_error("@-", self.type_string())
    }

    /// The `-` subtraction operator when the object is on the RHS
    ///
    /// This will be called when the value on the LHS doesn't implement the operation.
    fn subtract_rhs(&self, other: &KValue) -> Result<KValue> {
        let _ = other;
        unimplemented_error("@-", self.type_string())
    }

    /// The `*` multiplication operator
    ///
    /// This will be called by the runtime when the object is on the LHS.
    ///
    /// To specialize the behaviour of `*` when the object is on the RHS, see [Self::multiply_rhs].
    fn multiply(&self, other: &KValue) -> Result<KValue> {
        let _ = other;
        unimplemented_error("@*", self.type_string())
    }

    /// The `*` multiplication operator when the object is on the RHS
    ///
    /// This will be called when the value on the LHS doesn't implement the operation.
    fn multiply_rhs(&self, other: &KValue) -> Result<KValue> {
        let _ = other;
        unimplemented_error("@*", self.type_string())
    }

    /// The `/` division operator
    fn divide(&self, other: &KValue) -> Result<KValue> {
        let _ = other;
        unimplemented_error("@/", self.type_string())
    }

    /// The `/` division operator when the object is on the RHS
    ///
    /// This will be called when the value on the LHS doesn't implement the operation.
    fn divide_rhs(&self, other: &KValue) -> Result<KValue> {
        let _ = other;
        unimplemented_error("@/", self.type_string())
    }

    /// The `%` remainder operator
    fn remainder(&self, other: &KValue) -> Result<KValue> {
        let _ = other;
        unimplemented_error("@%", self.type_string())
    }

    /// The `%` remainder operator when the object is on the RHS
    ///
    /// This will be called when the value on the LHS doesn't implement the operation.
    fn remainder_rhs(&self, other: &KValue) -> Result<KValue> {
        let _ = other;
        unimplemented_error("@%", self.type_string())
    }

    /// The `^` power operator
    fn power(&self, other: &KValue) -> Result<KValue> {
        let _ = other;
        unimplemented_error("@^", self.type_string())
    }

    /// The `^` power operator when the object is on the RHS
    ///
    /// This will be called when the value on the LHS doesn't implement the operation.
    fn power_rhs(&self, other: &KValue) -> Result<KValue> {
        let _ = other;
        unimplemented_error("@^", self.type_string())
    }

    /// The `+=` in-place addition operator
    fn add_assign(&mut self, other: &KValue) -> Result<()> {
        let _ = other;
        unimplemented_error("@+=", self.type_string())
    }

    /// The `-=` in-place subtraction operator
    fn subtract_assign(&mut self, other: &KValue) -> Result<()> {
        let _ = other;
        unimplemented_error("@-=", self.type_string())
    }

    /// The `*=` in-place multiplication operator
    fn multiply_assign(&mut self, other: &KValue) -> Result<()> {
        let _ = other;
        unimplemented_error("@*=", self.type_string())
    }

    /// The `/=` in-place division operator
    fn divide_assign(&mut self, other: &KValue) -> Result<()> {
        let _ = other;
        unimplemented_error("@/=", self.type_string())
    }

    /// The `%=` in-place remainder operator
    fn remainder_assign(&mut self, other: &KValue) -> Result<()> {
        let _ = other;
        unimplemented_error("@%=", self.type_string())
    }

    /// The `^=` in-place remainder operator
    fn power_assign(&mut self, other: &KValue) -> Result<()> {
        let _ = other;
        unimplemented_error("@^=", self.type_string())
    }

    /// The `<` less-than operator
    fn less(&self, other: &KValue) -> Result<bool> {
        let _ = other;
        unimplemented_error("@<", self.type_string())
    }

    /// The `<=` less-than-or-equal operator
    ///
    /// The default implementation derives its result from [Self::less] and [Self::equal].
    fn less_or_equal(&self, other: &KValue) -> Result<bool> {
        match self.less(other) {
            Ok(true) => Ok(true),
            Ok(false) => match self.equal(other) {
                Ok(result) => Ok(result),
                Err(error) if error.is_unimplemented_error() => {
                    unimplemented_error("@<=", self.type_string())
                }
                error => error,
            },
            Err(error) if error.is_unimplemented_error() => {
                unimplemented_error("@<=", self.type_string())
            }
            error => error,
        }
    }

    /// The `>` greater-than operator
    ///
    /// The default implementation derives its result from [Self::less] and [Self::equal].
    fn greater(&self, other: &KValue) -> Result<bool> {
        match self.less(other) {
            Ok(true) => Ok(false),
            Ok(false) => match self.equal(other) {
                Ok(result) => Ok(!result),
                Err(error) if error.is_unimplemented_error() => {
                    unimplemented_error("@>", self.type_string())
                }
                error => error,
            },
            Err(error) if error.is_unimplemented_error() => {
                unimplemented_error("@>", self.type_string())
            }
            error => error,
        }
    }

    /// The `>=` greater-than-or-equal operator
    ///
    /// The default implementation derives its result from [Self::less].
    fn greater_or_equal(&self, other: &KValue) -> Result<bool> {
        match self.less(other) {
            Ok(result) => Ok(!result),
            Err(error) if error.is_unimplemented_error() => {
                unimplemented_error("@>=", self.type_string())
            }
            error => error,
        }
    }

    /// The `==` equality operator
    fn equal(&self, other: &KValue) -> Result<bool> {
        let _ = other;
        unimplemented_error("@==", self.type_string())
    }

    /// The `!=` inequality operator
    ///
    /// The default implementation derives its result from [Self::equal].
    fn not_equal(&self, other: &KValue) -> Result<bool> {
        match self.equal(other) {
            Ok(result) => Ok(!result),
            Err(error) if error.is_unimplemented_error() => {
                unimplemented_error("@!=", self.type_string())
            }
            error => error,
        }
    }

    /// Declares to the runtime whether or not the object is iterable
    ///
    /// The `Iterable` type hint defers to this function,
    /// accepting anything other than `IsIterable::NotIterable`.
    fn is_iterable(&self) -> IsIterable {
        IsIterable::NotIterable
    }

    /// Returns an iterator that iterates over the objects contents
    ///
    /// If [`IsIterable::Iterable`] is returned from [`is_iterable`](Self::is_iterable),
    /// then the runtime will call this function when the object is used in iterable contexts,
    /// expecting a [`KIterator`] to be returned.
    fn make_iterator(&self, vm: &mut KotoVm) -> Result<KIterator> {
        let _ = vm;
        unimplemented_error("@iterator", self.type_string())
    }

    /// Gets the object's next value in an iteration
    ///
    /// If either [`ForwardIterator`][IsIterable::ForwardIterator] or
    /// [`BidirectionalIterator`][IsIterable::BidirectionalIterator] is returned from
    /// [is_iterable](Self::is_iterable), then the object will be wrapped in a [`KIterator`]
    /// whenever it's used in an iterable context. This function will then be called each time
    /// [`KIterator::next`] is invoked.
    fn iterator_next(&mut self, vm: &mut KotoVm) -> Option<KIteratorOutput> {
        let _ = vm;
        None
    }

    /// Gets the object's next value from the end of an iteration
    ///
    /// If [`BidirectionalIterator`][IsIterable::BidirectionalIterator] is returned from
    /// [`is_iterable`](Self::is_iterable), then the object will be wrapped in a [`KIterator`]
    /// whenever it's used in an iterable context. This function will then be called each time
    /// [`KIterator::next_back`] is invoked.
    fn iterator_next_back(&mut self, vm: &mut KotoVm) -> Option<KIteratorOutput> {
        let _ = vm;
        None
    }

    /// Converts the object into a serializable [KValue]
    ///
    /// This is called by `koto_serde`'s serialize implementation when the object is encountered
    /// during serialization.
    ///
    /// The object should prepare a [KValue] that best represents the object's properties.
    fn serialize(&self) -> Result<KValue> {
        unimplemented_error("serialize", self.type_string())
    }
}

/// A [`KotoObject`] wrapper used in the Koto runtime
#[derive(Clone)]
pub struct KObject {
    object: PtrMut<dyn KotoObject>,
}

impl KObject {
    /// Checks if the object is of the given type
    pub fn is_a<T: KotoObject>(&self) -> bool {
        match self.object.try_borrow() {
            Some(object) => (object.deref() as &dyn Any).is::<T>(),
            None => false,
        }
    }

    /// Attempts to borrow the underlying object immutably
    pub fn try_borrow(&self) -> Result<Borrow<'_, dyn KotoObject>> {
        self.object
            .try_borrow()
            .ok_or_else(|| ErrorKind::UnableToBorrowObject.into())
    }

    /// Attempts to borrow the underlying object mutably
    pub fn try_borrow_mut(&self) -> Result<BorrowMut<'_, dyn KotoObject>> {
        self.object
            .try_borrow_mut()
            .ok_or_else(|| ErrorKind::UnableToBorrowObject.into())
    }

    /// Attempts to immutably borrow and cast the underlying object to the specified type
    pub fn cast<T: KotoObject>(&self) -> Result<Borrow<'_, T>> {
        Borrow::filter_map(self.try_borrow()?, |object| {
            (object as &dyn Any).downcast_ref::<T>()
        })
        .map_err(|_| match self.try_borrow() {
            Ok(object) => ErrorKind::UnexpectedObjectType {
                expected: T::type_static(),
                unexpected: object.type_string(),
            }
            .into(),
            Err(e) => e,
        })
    }

    /// Attempts to mutably borrow and cast the underlying object to the specified type
    pub fn cast_mut<T: KotoObject>(&self) -> Result<BorrowMut<'_, T>> {
        BorrowMut::filter_map(self.try_borrow_mut()?, |object| {
            (object as &mut dyn Any).downcast_mut::<T>()
        })
        .map_err(|_| match self.try_borrow() {
            Ok(object) => ErrorKind::UnexpectedObjectType {
                expected: T::type_static(),
                unexpected: object.type_string(),
            }
            .into(),
            Err(e) => e,
        })
    }

    /// Returns true if the provided object occupies the same memory address
    pub fn is_same_instance(&self, other: &Self) -> bool {
        PtrMut::ptr_eq(&self.object, &other.object)
    }

    /// Returns the number of references currently held to the object
    pub fn ref_count(&self) -> usize {
        PtrMut::ref_count(&self.object)
    }
}

impl<T: KotoObject> From<T> for KObject {
    fn from(object: T) -> Self {
        Self {
            object: make_ptr_mut!(object),
        }
    }
}

impl fmt::Debug for KObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "KObject ({:?})", PtrMut::address(&self.object))
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

/// Creates an error that describes an unimplemented method
fn unimplemented_error<T>(fn_name: &'static str, object_type: KString) -> Result<T> {
    runtime_error!(ErrorKind::Unimplemented {
        fn_name,
        object_type
    })
}

/// An enum that indicates to the runtime if a [`KotoObject`] is iterable
pub enum IsIterable {
    /// The object is not iterable
    NotIterable,
    /// The object is iterable
    ///
    /// An iterable object is not itself an iterator, but provides an implementation of
    /// [KotoObject::make_iterator] that is used to make an iterator when one is needed by the
    /// runtime.
    Iterable,
    /// The object is a forward-only iterator
    ///
    /// A forward iterator provides an implementation of [KotoObject::iterator_next],
    /// but not [KotoObject::iterator_next_back].
    ForwardIterator,
    /// The object is a bidirectional iterator.
    ///
    /// A bidirectional iterator provides an implementation of [KotoObject::iterator_next] and
    /// [KotoObject::iterator_next_back].
    BidirectionalIterator,
}
