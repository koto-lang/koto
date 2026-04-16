use crate::KotoBackend;

/// The hash algorithm used by shared Koto macro-generated caches.
pub type KotoHasher = rustc_hash::FxHasher;

/// Shared static object type information available in both `koto_runtime` and `koto_plugin`.
pub trait KotoStaticType {
    /// Returns the type as a static string.
    fn type_static() -> &'static str
    where
        Self: Sized;
}

/// Shared object type information available in both `koto_runtime` and `koto_plugin`.
pub trait KotoType<B: KotoBackend>: KotoStaticType {
    /// Returns the type as a string-like value.
    fn type_string(&self) -> B::String;
}

/// Shared copy behavior for object values in both `koto_runtime` and `koto_plugin`.
pub trait KotoCopy<B: KotoBackend> {
    /// Returns a copy of the object.
    fn copy(&self) -> B::Object;

    /// Returns a deep copy of the object.
    fn deep_copy(&self) -> B::Object {
        self.copy()
    }
}

/// Shared read-only operations available on object borrows in both backends.
pub trait KotoObjectHandle<B: KotoBackend> {
    /// Returns the object's type as a string-like value.
    fn type_string(&self) -> B::String;
}

impl<B: KotoBackend, T: KotoType<B> + ?Sized> KotoObjectHandle<B> for T {
    fn type_string(&self) -> B::String {
        KotoType::type_string(self)
    }
}

/// Shared iterable-kind information for object values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KotoObjectIterable {
    /// The object isn't iterable.
    NotIterable,
    /// The object is iterable and can produce an iterator.
    Iterable,
    /// The object is a forward iterator.
    ForwardIterator,
    /// The object is a bidirectional iterator.
    BidirectionalIterator,
}
