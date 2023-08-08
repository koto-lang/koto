use crate::{prelude::*, ExternalFunction, Result};
use downcast_rs::{impl_downcast, Downcast};
use std::{cell::RefCell, fmt, marker::PhantomData, rc::Rc};

/// A trait for implementing objects that can be added to the Koto runtime
///
/// See also: [Object].
pub trait KotoObject: KotoCopyable + Downcast {
    /// The type of the Object as a [ValueString]
    ///
    /// A typical pattern will be to implement [KotoType] for use with [ObjectEntryBuilder],
    /// and then defer to [KotoType::TYPE].
    ///
    /// This will be called whenever the object's type is needed by the runtime,
    /// e.g. when a script calls `koto.type`, so it can make sense to cache a [ValueString],
    /// and then return clones of it to avoid creating lots of strings.
    ///
    /// ```
    /// use koto_runtime::prelude::*;
    ///
    /// #[derive(Clone)]
    /// pub struct Foo;
    ///
    /// impl KotoType for Foo {
    ///     const TYPE: &'static str = "Foo";
    /// }
    ///
    /// impl KotoObject for Foo {
    ///     fn object_type(&self) -> ValueString {
    ///         FOO_TYPE_STRING.with(|t| t.clone())
    ///     }
    /// }
    ///
    /// thread_local! {
    ///     static FOO_TYPE_STRING: ValueString = Foo::TYPE.into();
    /// }
    /// ```
    fn object_type(&self) -> ValueString;

    /// Called when the object should be displayed as a string, e.g. by `io.print`
    ///
    /// By default, the object's type is used as the display string.
    ///
    /// A VM is provided as an argument in case the object contains other Koto values that should be
    /// displayed.
    ///
    /// The [KotoDisplayOptions] provide context that might alter the ideal string representation.
    fn display(
        &self,
        out: &mut StringBuilder,
        _vm: &mut Vm,
        _options: KotoDisplayOptions,
    ) -> Result<()> {
        out.append(self.object_type());
        Ok(())
    }

    /// Called when the object should be displayed in a debugging context
    fn debug(&self) -> ValueString {
        self.object_type()
    }

    /// Returns a [Value] corresponding to the specified key within the object
    ///
    /// This method is used to retrieve a named entry attached to an object, providing a way to
    /// access the object's methods or associated values.
    ///
    /// The returned value should represent the data associated with the given key. If the key
    /// does not match any entry within the object, `None` should be returned.
    ///
    /// The recommended pattern for complex objects is to use an [ObjectEntryBuilder] to create a
    /// cached [DataMap], which helps to avoid the cost of recreating values for each lookup.
    ///
    /// See the [ObjectEntryBuilder] docs for an example.
    fn lookup(&self, _key: &ValueKey) -> Option<Value> {
        None
    }

    /// Called for indexing operations, e.g. `x[0]`
    fn index(&self, _index: &Value) -> Result<Value> {
        unimplemented_error("@index", self.object_type())
    }

    /// Allows the object to behave as a function
    ///
    /// To access the call arguments, use [Vm::get_args] with the provided [ArgRegisters].
    fn call(&mut self, _vm: &mut Vm, _args: &ArgRegisters) -> Result<Value> {
        unimplemented_error("@||", self.object_type())
    }

    /// Defines the behavior of negation (e.g. `-x`)
    fn negate(&self, _vm: &mut Vm) -> Result<Value> {
        unimplemented_error("@negate", self.object_type())
    }

    /// The `+` addition operator ()
    fn add(&self, _rhs: &Value) -> Result<Value> {
        unimplemented_error("@+", self.object_type())
    }

    /// The `-` subtraction operator
    fn subtract(&self, _rhs: &Value) -> Result<Value> {
        unimplemented_error("@-", self.object_type())
    }

    /// The `*` multiplication operator
    fn multiply(&self, _rhs: &Value) -> Result<Value> {
        unimplemented_error("@*", self.object_type())
    }

    /// The `/` division operator
    fn divide(&self, _rhs: &Value) -> Result<Value> {
        unimplemented_error("@/", self.object_type())
    }

    /// The `%` remainder operator
    fn remainder(&self, _rhs: &Value) -> Result<Value> {
        unimplemented_error("@%", self.object_type())
    }

    /// The `+=` in-place addition operator
    fn add_assign(&mut self, _rhs: &Value) -> Result<()> {
        unimplemented_error("@+=", self.object_type())
    }

    /// The `-=` in-place subtraction operator
    fn subtract_assign(&mut self, _rhs: &Value) -> Result<()> {
        unimplemented_error("@-=", self.object_type())
    }

    /// The `*=` in-place multiplication operator
    fn multiply_assign(&mut self, _rhs: &Value) -> Result<()> {
        unimplemented_error("@*=", self.object_type())
    }

    /// The `/=` in-place division operator
    fn divide_assign(&mut self, _rhs: &Value) -> Result<()> {
        unimplemented_error("@/=", self.object_type())
    }

    /// The `%=` in-place remainder operator
    fn remainder_assign(&mut self, _rhs: &Value) -> Result<()> {
        unimplemented_error("@%=", self.object_type())
    }

    /// The `<` less-than operator
    fn less(&self, _rhs: &Value) -> Result<bool> {
        unimplemented_error("@<", self.object_type())
    }

    /// The `<=` less-than-or-equal operator
    fn less_or_equal(&self, _rhs: &Value) -> Result<bool> {
        unimplemented_error("@<=", self.object_type())
    }

    /// The `>` greater-than operator
    fn greater(&self, _rhs: &Value) -> Result<bool> {
        unimplemented_error("@>", self.object_type())
    }

    /// The `>=` greater-than-or-equal operator
    fn greater_or_equal(&self, _rhs: &Value) -> Result<bool> {
        unimplemented_error("@>=", self.object_type())
    }

    /// The `==` equality operator
    fn equal(&self, _rhs: &Value) -> Result<bool> {
        unimplemented_error("@==", self.object_type())
    }

    /// The `!=` inequality operator
    fn not_equal(&self, _rhs: &Value) -> Result<bool> {
        unimplemented_error("@!=", self.object_type())
    }

    /// Declares to the runtime whether or not the object is iterable
    fn is_iterable(&self) -> IsIterable {
        IsIterable::NotIterable
    }

    /// Returns an iterator that iterates over the objects contents
    ///
    /// If [IsIterable::Iterable] is returned from [is_iterable](Self::is_iterable),
    /// then the runtime will call this function when the object is used in iterable contexts,
    /// expecting a [ValueIterator] to be returned.
    fn make_iterator(&self, _vm: &mut Vm) -> Result<ValueIterator> {
        unimplemented_error("@iterator", self.object_type())
    }

    /// Gets the object's next value in an iteration
    ///
    /// If either [ForwardIterator][IsIterable::ForwardIterator] or
    /// [BidirectionalIterator][IsIterable::BidirectionalIterator] is returned from
    /// [is_iterable](Self::is_iterable), then the object will be wrapped in a [ValueIterator]
    /// whenever it's used in an iterable context. This function will then be called each time
    /// [ValueIterator::next] is invoked.
    fn iterator_next(&mut self, _vm: &mut Vm) -> Option<ValueIteratorOutput> {
        None
    }

    /// Gets the object's next value from the end of an iteration
    ///
    /// If [BidirectionalIterator][IsIterable::BidirectionalIterator] is returned from
    /// [is_iterable](Self::is_iterable), then the object will be wrapped in a [ValueIterator]
    /// whenever it's used in an iterable context. This function will then be called each time
    /// [ValueIterator::next_back] is invoked.
    fn iterator_next_back(&mut self, _vm: &mut Vm) -> Option<ValueIteratorOutput> {
        None
    }
}

impl_downcast!(KotoObject);

impl fmt::Debug for dyn KotoObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.debug())
    }
}

/// Defines how copies of [KotoObject]s should be made
///
/// A blanket default implementation is provided for any [KotoObject] that implements [Clone].
pub trait KotoCopyable {
    /// How the object should behave when called from `koto.copy`
    fn copy(&self) -> Object;

    /// How the object should behave when called from `koto.deep_copy`
    ///
    /// Deep copies should ensure that deep copies are performed for any Koto values that are owned
    /// by the object (see [Value::deep_copy]).
    fn deep_copy(&self) -> Object {
        self.copy()
    }
}

impl<T: Clone + KotoObject> KotoCopyable for T {
    fn copy(&self) -> Object {
        self.clone().into()
    }
}

/// A wrapper for [KotoObject]s used in the Koto runtime
#[derive(Clone, Debug)]
pub struct Object {
    object: PtrMut<dyn KotoObject>,
}

impl Object {
    /// Checks if the object is of the given type
    pub fn is_a<T: KotoObject>(&self) -> bool {
        match self.object.try_borrow() {
            Ok(object) => object.downcast_ref::<T>().is_some(),
            Err(_) => false,
        }
    }

    /// Attempts to borrow the underlying object immutably
    pub fn try_borrow(&self) -> Result<Borrow<dyn KotoObject>> {
        self.object.try_borrow().map_err(|_| {
            make_runtime_error!("Attempting to borrow an object that is already mutably borrowed")
        })
    }

    /// Attempts to borrow the underlying object mutably
    pub fn try_borrow_mut(&self) -> Result<BorrowMut<dyn KotoObject>> {
        self.object.try_borrow_mut().map_err(|_| {
            make_runtime_error!("Attempting to borrow an object that is already mutably borrowed")
        })
    }

    /// Attempts to immutably borrow and cast the underlying object to the specified type
    pub fn cast<T: KotoObject>(&self) -> Result<Borrow<T>> {
        Borrow::filter_map(self.try_borrow()?, |object| object.downcast_ref::<T>())
            .map_err(|_| make_runtime_error!("Incorrect object type"))
    }

    /// Attempts to mutably borrow and cast the underlying object to the specified type
    pub fn cast_mut<T: KotoObject>(&self) -> Result<BorrowMut<T>> {
        BorrowMut::filter_map(self.try_borrow_mut()?, |object| object.downcast_mut::<T>())
            .map_err(|_| make_runtime_error!("Incorrect object type"))
    }

    /// Checks if the provided object is the same instance as this one
    pub fn is_same_instance(&self, other: &Self) -> bool {
        PtrMut::ptr_eq(&self.object, &other.object)
    }
}

impl<T: KotoObject> From<T> for Object {
    fn from(object: T) -> Self {
        Self {
            object: PtrMut::from(Rc::new(RefCell::new(object)) as Rc<RefCell<dyn KotoObject>>),
        }
    }
}

/// A trait for specifying an object's type
///
/// See also: [KotoObject::object_type]
pub trait KotoType {
    /// The object's type
    const TYPE: &'static str;
}

/// A helper for building a lookup map for objects that implement [KotoObject::lookup]
///
/// ```
/// use koto_runtime::prelude::*;
///
/// #[derive(Clone, Default)]
/// pub struct Foo {
///     data: i32,
/// }
///
/// impl KotoType for Foo {
///     const TYPE: &'static str = "Foo";
/// }
///
/// impl KotoObject for Foo {
///     fn object_type(&self) -> ValueString {
///         FOO_TYPE_STRING.with(|t| t.clone())
///     }
///
///     fn lookup(&self, key: &ValueKey) -> Option<Value> {
///         FOO_ENTRIES.with(|entries| entries.get(key).cloned())
///     }
/// }
///
/// impl From<Foo> for Value {
///     fn from(foo: Foo) -> Self {
///         Object::from(foo).into()
///     }
/// }
///
/// fn make_foo_entries() -> DataMap {
///     use Value::*;
///
///     ObjectEntryBuilder::<Foo>::new()
///         .method_aliased(&["data", "get_data"], |ctx| Ok(ctx.instance()?.data.into()))
///         .method("set_data", |ctx| {
///             let new_data = match ctx.args {
///                 [Object(o)] if o.is_a::<Foo>() => {
///                     // .unwrap() is safe here, the is_a guard guarantees a successful cast
///                     o.cast::<Foo>().unwrap().data
///                 }
///                 [Number(n)] => n.into(),
///                 unexpected => return type_error_with_slice("a Number", unexpected),
///             };
///
///             // Set the instance's new data value
///             ctx.instance_mut()?.data = new_data;
///             // Return the object as the result of the setter operation
///             ctx.instance_result()
///         })
///         .build()
/// }
///
/// thread_local! {
///     static FOO_TYPE_STRING: ValueString = Foo::TYPE.into();
///     static FOO_ENTRIES: DataMap = make_foo_entries();
/// }
/// ```
pub struct ObjectEntryBuilder<T: KotoObject + KotoType> {
    // The map that's being built
    map: DataMap,
    // We want to have T available through the implementation
    _phantom: PhantomData<T>,
}

impl<T: KotoObject + KotoType> Default for ObjectEntryBuilder<T> {
    fn default() -> Self {
        Self {
            map: DataMap::default(),
            _phantom: PhantomData,
        }
    }
}

impl<T: KotoObject + KotoType> ObjectEntryBuilder<T> {
    /// Makes a new object entry builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the resulting DataMap, consuming the builder
    pub fn build(self) -> DataMap {
        self.map
    }

    /// Adds a method to the object's entries
    ///
    /// The provided function will be called with a [MethodContext] that provides access to the
    /// object instance and arguments.
    pub fn method<Key, F>(self, key: Key, f: F) -> Self
    where
        Key: Into<ValueKey> + Clone,
        F: Fn(MethodContext<T>) -> RuntimeResult + Clone + 'static,
    {
        self.method_aliased(&[key], f)
    }

    /// Adds a method with equivalent names to the object's entries
    ///
    /// This is useful when you want to provide aliases for functions,
    /// e.g. `color.red()` and `color.r()` should both provide the color's red component.
    pub fn method_aliased<Key, F>(mut self, keys: &[Key], f: F) -> Self
    where
        Key: Into<ValueKey> + Clone,
        F: Fn(MethodContext<T>) -> RuntimeResult + Clone + 'static,
    {
        let wrapped_function = move |vm: &mut Vm, args: &ArgRegisters| match vm.get_args(args) {
            [Value::Object(o), extra_args @ ..] => f(MethodContext::new(o, extra_args, vm)),
            other => type_error_with_slice(&format!("'{}'", T::TYPE), other),
        };

        for key in keys {
            self.map.insert(
                key.clone().into(),
                Value::ExternalFunction(ExternalFunction::new(wrapped_function.clone(), true)),
            );
        }

        self
    }
}

/// Context provided to a function that implements an object method
///
/// See [ObjectEntryBuilder]
pub struct MethodContext<'a, T> {
    /// The method call arguments
    pub args: &'a [Value],
    /// A VM that can be used by the method for operations that require a runtime
    pub vm: &'a Vm,
    // The instance of the object for the method call,
    // accessable via the context's `instance`/`instance_mut` functions
    object: &'a Object,
    // We want access to `T` in the implementation
    _phantom: PhantomData<T>,
}

impl<'a, T: KotoObject> MethodContext<'a, T> {
    /// Makes a new method context
    fn new(object: &'a Object, args: &'a [Value], vm: &'a Vm) -> Self {
        Self {
            object,
            args,
            vm,
            _phantom: PhantomData,
        }
    }

    /// Attempts to immutably borrow the object instance
    pub fn instance(&self) -> Result<Borrow<T>> {
        self.object.cast::<T>()
    }

    /// Attempts to mutably borrow the object instance
    pub fn instance_mut(&self) -> Result<BorrowMut<T>> {
        self.object.cast_mut::<T>()
    }

    /// Helper for methods that need to return a clone of the instance as the method result
    pub fn instance_result(&self) -> RuntimeResult {
        Ok(self.object.clone().into())
    }
}

/// Creates an error that describes an unimplemented method
fn unimplemented_error<T>(method: &str, object_type: ValueString) -> Result<T> {
    runtime_error!("{method} is unimplemented for {object_type}")
}

/// An enum that indicates to the runtime if a [KotoObject] is iterable
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
