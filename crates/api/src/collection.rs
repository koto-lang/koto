use crate::KotoBackend;
use std::marker::PhantomData;

/// Shared collection operations available in both `koto_runtime` and `koto_plugin`.
pub trait KotoCollection<B: KotoBackend> {
    /// Returns the number of entries in the collection.
    fn len(&self) -> usize;

    /// Returns `true` if the collection has no entries.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Shared read-only slice operations available in both `koto_runtime` and `koto_plugin`.
pub trait KotoSlice<B: KotoBackend>: KotoCollection<B> {
    /// Returns the value at `index`, if present.
    fn get(&self, index: usize) -> Option<B::Value>;

    /// Returns an iterator over cloned slice entries.
    fn iter(&self) -> KotoSliceIter<'_, B, Self>
    where
        Self: Sized,
    {
        KotoSliceIter::new(self)
    }
}

/// Shared read-only owned sequence operations for types that hand out borrowed slice views.
pub trait KotoSequence<B: KotoBackend>: KotoCollection<B> {
    /// A borrowed view over the sequence's data.
    type Data<'a>: KotoSlice<B>
    where
        Self: 'a;

    /// Returns a borrowed view over the sequence's data.
    fn data(&self) -> Self::Data<'_>;

    /// Builds a sequence from the provided slice of values.
    fn from_slice(values: &[B::Value]) -> Self
    where
        Self: Sized;
}

impl<B: KotoBackend> KotoCollection<B> for [B::Value] {
    fn len(&self) -> usize {
        <[B::Value]>::len(self)
    }
}

impl<B: KotoBackend> KotoSlice<B> for [B::Value] {
    fn get(&self, index: usize) -> Option<B::Value> {
        <[B::Value]>::get(self, index).cloned()
    }
}

impl<B: KotoBackend> KotoCollection<B> for &[B::Value] {
    fn len(&self) -> usize {
        <[B::Value]>::len(self)
    }
}

impl<B: KotoBackend> KotoSlice<B> for &[B::Value] {
    fn get(&self, index: usize) -> Option<B::Value> {
        <[B::Value]>::get(self, index).cloned()
    }
}

/// An iterator returned by [`KotoSlice::iter`].
pub struct KotoSliceIter<'a, B: KotoBackend, T: ?Sized> {
    sequence: &'a T,
    index: usize,
    len: usize,
    _backend: PhantomData<B>,
}

impl<'a, B: KotoBackend, T: KotoSlice<B> + ?Sized> KotoSliceIter<'a, B, T> {
    fn new(sequence: &'a T) -> Self {
        Self {
            sequence,
            index: 0,
            len: sequence.len(),
            _backend: PhantomData,
        }
    }
}

impl<B: KotoBackend, T: KotoSlice<B> + ?Sized> Iterator for KotoSliceIter<'_, B, T> {
    type Item = B::Value;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.len {
            let index = self.index;
            self.index += 1;
            if let Some(value) = self.sequence.get(index) {
                return Some(value);
            }
        }

        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len.saturating_sub(self.index);
        (remaining, Some(remaining))
    }
}

impl<B: KotoBackend, T: KotoSlice<B> + ?Sized> ExactSizeIterator for KotoSliceIter<'_, B, T> {}

/// Shared associative lookup operations available on maps.
pub trait KotoMapLookup<B: KotoBackend> {
    /// Returns a cloned value for the given key, if present.
    fn get_key(&self, key: &B::Value) -> Option<B::Value>;

    /// Returns `true` if the map contains the given key.
    fn contains_key(&self, key: &B::Value) -> bool {
        self.get_key(key).is_some()
    }
}

/// Shared map operations available in both `koto_runtime` and `koto_plugin`.
pub trait KotoMap<B: KotoBackend>: KotoCollection<B> + KotoMapLookup<B> {
    /// Returns a cloned key/value pair at the given insertion index.
    fn get_index(&self, index: usize) -> Option<(B::Value, B::Value)>;

    /// Returns an iterator over cloned key/value entries.
    fn iter(&self) -> KotoMapIter<'_, B, Self>
    where
        Self: Sized,
    {
        KotoMapIter::new(self)
    }

    /// Returns an iterator over cloned map keys.
    fn keys(&self) -> KotoMapKeys<'_, B, Self>
    where
        Self: Sized,
    {
        KotoMapKeys { iter: self.iter() }
    }

    /// Returns an iterator over cloned map values.
    fn values(&self) -> KotoMapValues<'_, B, Self>
    where
        Self: Sized,
    {
        KotoMapValues { iter: self.iter() }
    }
}

/// Shared read-only owned map operations for types that hand out borrowed map views.
pub trait KotoMapSource<B: KotoBackend>: KotoCollection<B> {
    /// A borrowed view over the map's data.
    type Data<'a>: KotoMap<B>
    where
        Self: 'a;

    /// Returns a borrowed view over the map's data.
    fn data(&self) -> Self::Data<'_>;

    /// Builds a map from the provided key/value entries.
    fn from_entries(entries: &[(B::Value, B::Value)]) -> Self
    where
        Self: Sized;
}

impl<B: KotoBackend, T: KotoMapSource<B> + ?Sized> KotoMapLookup<B> for T {
    fn get_key(&self, key: &B::Value) -> Option<B::Value> {
        self.data().get_key(key)
    }
}

impl<B: KotoBackend, T: KotoMapSource<B> + ?Sized> KotoMap<B> for T {
    fn get_index(&self, index: usize) -> Option<(B::Value, B::Value)> {
        self.data().get_index(index)
    }
}

/// Shared mutable map operations for types that hand out borrowed mutable map views.
pub trait KotoMapSourceMut<B: KotoBackend>: KotoMapSource<B> {
    /// A mutable borrowed view over the map's data.
    type DataMut<'a>: KotoMap<B> + KotoIndexSwap<B>
    where
        Self: 'a;

    /// Returns a mutable borrowed view over the map's data.
    fn data_mut(&self) -> Self::DataMut<'_>;
}

/// An iterator returned by [`KotoMap::iter`].
pub struct KotoMapIter<'a, B: KotoBackend, T: ?Sized> {
    map: &'a T,
    index: usize,
    len: usize,
    _backend: PhantomData<B>,
}

impl<'a, B: KotoBackend, T: KotoMap<B> + ?Sized> KotoMapIter<'a, B, T> {
    fn new(map: &'a T) -> Self {
        Self {
            map,
            index: 0,
            len: map.len(),
            _backend: PhantomData,
        }
    }
}

impl<B: KotoBackend, T: KotoMap<B> + ?Sized> Iterator for KotoMapIter<'_, B, T> {
    type Item = (B::Value, B::Value);

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.len {
            let index = self.index;
            self.index += 1;
            if let Some(entry) = self.map.get_index(index) {
                return Some(entry);
            }
        }

        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len.saturating_sub(self.index);
        (remaining, Some(remaining))
    }
}

impl<B: KotoBackend, T: KotoMap<B> + ?Sized> ExactSizeIterator for KotoMapIter<'_, B, T> {}

/// An iterator returned by [`KotoMap::keys`].
pub struct KotoMapKeys<'a, B: KotoBackend, T: KotoMap<B> + ?Sized> {
    iter: KotoMapIter<'a, B, T>,
}

impl<B: KotoBackend, T: KotoMap<B> + ?Sized> Iterator for KotoMapKeys<'_, B, T> {
    type Item = B::Value;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(key, _)| key)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<B: KotoBackend, T: KotoMap<B> + ?Sized> ExactSizeIterator for KotoMapKeys<'_, B, T> {}

/// An iterator returned by [`KotoMap::values`].
pub struct KotoMapValues<'a, B: KotoBackend, T: KotoMap<B> + ?Sized> {
    iter: KotoMapIter<'a, B, T>,
}

impl<B: KotoBackend, T: KotoMap<B> + ?Sized> Iterator for KotoMapValues<'_, B, T> {
    type Item = B::Value;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(_, value)| value)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<B: KotoBackend, T: KotoMap<B> + ?Sized> ExactSizeIterator for KotoMapValues<'_, B, T> {}

/// Shared map-building operations available in both `koto_runtime` and `koto_plugin`.
pub trait KotoMapBuilder<B: KotoBackend> {
    /// The meta key type used by the map builder.
    type MetaKey;

    /// Creates a new map with the given `@type`.
    fn with_type(type_name: &str) -> Self
    where
        Self: Sized;

    /// Inserts a value into the map using a string key.
    fn insert(&self, key: &str, value: impl Into<B::Value>);

    /// Adds a function to the map using a string key.
    fn add_fn<F>(&self, key: &str, f: F)
    where
        F: for<'a> Fn(&mut B::CallContext<'a>) -> Result<B::Value, B::Error>
            + Send
            + Sync
            + 'static;

    /// Inserts a value into the map's meta map.
    fn insert_meta(&mut self, key: Self::MetaKey, value: impl Into<B::Value>);

    /// Adds a function to the map's meta map.
    fn add_meta_fn<F>(&mut self, key: Self::MetaKey, f: F)
    where
        F: for<'a> Fn(&mut B::CallContext<'a>) -> Result<B::Value, B::Error>
            + Send
            + Sync
            + 'static;
}

/// Shared mutable slice operations available in both backends.
pub trait KotoSliceMut<B: KotoBackend>: KotoSlice<B> {
    /// Replaces a slice item at the given index.
    fn set(&mut self, index: usize, value: B::Value) -> Result<(), B::Error>;
}

/// Shared mutable owned sequence operations available in both backends.
pub trait KotoSequenceMut<B: KotoBackend>: KotoSequence<B> {
    /// A borrowed mutable view over the sequence's data.
    type DataMut<'a>: KotoSliceMut<B> + KotoIndexSwap<B>
    where
        Self: 'a;

    /// Returns a mutable borrowed view over the sequence's data.
    fn data_mut(&self) -> Self::DataMut<'_>;
}

/// Shared ordered-container operations for swapping entries by index.
pub trait KotoIndexSwap<B: KotoBackend> {
    /// Swaps two entries by index.
    fn swap_indices(&mut self, a: usize, b: usize) -> Result<(), B::Error>;
}
