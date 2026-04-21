#[cfg(feature = "native_host")]
use crate::KCell;
#[cfg(feature = "native_host")]
use crate::native_host::transfer::AbiTransfer;
use crate::{Borrow, BorrowMut, PtrMut, Result, prelude::*};
use koto_api::{
    KotoCollection, KotoIdentity, KotoIndexSwap, KotoSequence, KotoSequenceMut, KotoSlice,
    KotoSliceMut,
};
use std::ops::Deref;

/// The underlying `Vec` type used by [KList]
pub type ValueVec = smallvec::SmallVec<[KValue; 4]>;

/// The List type used by the Koto runtime
#[derive(Clone, Default)]
pub struct KList(PtrMut<ValueVec>);

/// A slice-like view over a [`KList`]'s data.
pub struct KListData<'a>(Borrow<'a, ValueVec>);

/// A mutable view over a [`KList`]'s data.
pub struct KListDataMut<'a>(BorrowMut<'a, ValueVec>);

impl Deref for KListData<'_> {
    type Target = [KValue];

    fn deref(&self) -> &Self::Target {
        self.0.as_slice()
    }
}

impl KotoCollection<RuntimeBackend> for KListData<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl KotoSlice<RuntimeBackend> for KListData<'_> {
    fn get(&self, index: usize) -> Option<KValue> {
        self.0.get(index).cloned()
    }
}

impl KotoCollection<RuntimeBackend> for KListDataMut<'_> {
    fn len(&self) -> usize {
        KListDataMut::len(self)
    }
}

impl KotoSlice<RuntimeBackend> for KListDataMut<'_> {
    fn get(&self, index: usize) -> Option<KValue> {
        KListDataMut::get(self, index)
    }
}

impl KotoSliceMut<RuntimeBackend> for KListDataMut<'_> {
    fn set(&mut self, index: usize, value: KValue) -> std::result::Result<(), crate::Error> {
        if let Some(slot) = self.0.get_mut(index) {
            *slot = value;
            Ok(())
        } else {
            Err(format!("invalid list index ({index})").into())
        }
    }
}

impl KotoIndexSwap<RuntimeBackend> for KListDataMut<'_> {
    fn swap_indices(&mut self, a: usize, b: usize) -> std::result::Result<(), crate::Error> {
        let len = self.len();
        if a >= len {
            return Err(format!("invalid list index ({a})").into());
        }
        if b >= len {
            return Err(format!("invalid list index ({b})").into());
        }

        self.0.swap(a, b);
        Ok(())
    }
}

impl KList {
    /// Creates an empty list with the given capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self(ValueVec::with_capacity(capacity).into())
    }

    /// Creates a list containing the provided data
    pub fn with_data(data: ValueVec) -> Self {
        Self(data.into())
    }

    /// Creates a list containing the provided slice of [Values](crate::KValue)
    pub fn from_slice(data: &[KValue]) -> Self {
        Self(data.iter().cloned().collect::<ValueVec>().into())
    }

    /// Returns a reference to the list's entries
    pub fn data(&self) -> Borrow<'_, ValueVec> {
        self.0.borrow()
    }

    /// Returns a mutable reference to the list's entries
    pub fn data_mut(&self) -> BorrowMut<'_, ValueVec> {
        self.0.borrow_mut()
    }

    /// Returns true if the lists refer to the same underlying data
    pub fn is_same_instance(&self, other: &Self) -> bool {
        PtrMut::ptr_eq(&self.0, &other.0)
    }

    /// Replaces a list item at the given index.
    pub fn set(&self, index: usize, value: KValue) -> Result<()> {
        if let Some(slot) = self.data_mut().get_mut(index) {
            *slot = value;
            Ok(())
        } else {
            Err(format!("invalid list index ({index})").into())
        }
    }

    /// Swaps two list entries.
    pub fn swap_indices(&self, a: usize, b: usize) -> Result<()> {
        let len = self.len();
        if a >= len {
            return Err(format!("invalid list index ({a})").into());
        }
        if b >= len {
            return Err(format!("invalid list index ({b})").into());
        }

        self.data_mut().swap(a, b);
        Ok(())
    }

    /// Renders the list to the provided display context
    pub fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append('[');

        let id = PtrMut::address(&self.0);
        if ctx.is_in_parents(id) {
            ctx.append("...");
        } else {
            ctx.push_container(id);

            for (i, value) in self.data().iter().enumerate() {
                if i > 0 {
                    ctx.append(", ");
                }
                value.display(ctx)?;
            }

            ctx.pop_container();
        }

        ctx.append(']');
        Ok(())
    }
}

#[cfg(feature = "native_host")]
impl AbiTransfer for KList {
    type Abi = koto_ffi::native::KList;

    unsafe fn into_abi(self) -> Self::Abi {
        koto_ffi::native::KList(unsafe { PtrMut::into_raw(self.0) } as *mut std::ffi::c_void)
    }

    unsafe fn from_abi(list: Self::Abi) -> Self {
        Self(unsafe { PtrMut::from_raw(list.0 as *const KCell<ValueVec>) })
    }

    unsafe fn clone_from_abi(list: Self::Abi) -> Self {
        Self(unsafe { PtrMut::clone_from_raw(list.0 as *const KCell<ValueVec>) })
    }
}

impl KListDataMut<'_> {
    /// Returns the number of entries in the list.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns the value at `index`, if present.
    pub fn get(&self, index: usize) -> Option<KValue> {
        self.0.get(index).cloned()
    }
}

impl KotoCollection<RuntimeBackend> for KList {
    fn len(&self) -> usize {
        self.data().len()
    }
}

impl KotoSlice<RuntimeBackend> for KList {
    fn get(&self, index: usize) -> Option<KValue> {
        KList::data(self).get(index).cloned()
    }
}

impl KotoSequence<RuntimeBackend> for KList {
    type Data<'a>
        = KListData<'a>
    where
        Self: 'a;

    fn data(&self) -> Self::Data<'_> {
        KListData(KList::data(self))
    }

    fn from_slice(values: &[KValue]) -> Self {
        KList::with_data(values.iter().cloned().collect())
    }
}

impl KotoSequenceMut<RuntimeBackend> for KList {
    type DataMut<'a>
        = KListDataMut<'a>
    where
        Self: 'a;

    fn data_mut(&self) -> Self::DataMut<'_> {
        KListDataMut(KList::data_mut(self))
    }
}

impl KotoIdentity for KList {
    fn is_same_instance(&self, other: &Self) -> bool {
        KList::is_same_instance(self, other)
    }
}

impl KotoIndexSwap<RuntimeBackend> for KList {
    fn swap_indices(&mut self, a: usize, b: usize) -> std::result::Result<(), crate::Error> {
        KList::swap_indices(self, a, b)
    }
}

impl From<Vec<KValue>> for KList {
    fn from(values: Vec<KValue>) -> Self {
        KList::from_slice(&values)
    }
}

impl FromIterator<KValue> for KList {
    fn from_iter<T: IntoIterator<Item = KValue>>(iter: T) -> Self {
        KList::from(iter.into_iter().collect::<Vec<_>>())
    }
}
