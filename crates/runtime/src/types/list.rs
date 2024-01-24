use crate::{prelude::*, Borrow, BorrowMut, PtrMut, Result};

/// The underlying Vec type used by [KList]
pub type ValueVec = smallvec::SmallVec<[KValue; 4]>;

/// The Koto runtime's List type
#[derive(Clone, Default)]
pub struct KList(PtrMut<ValueVec>);

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

    /// Returns the number of entries of the list
    pub fn len(&self) -> usize {
        self.data().len()
    }

    /// Returns true if there are no entries in the list
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a reference to the list's entries
    pub fn data(&self) -> Borrow<ValueVec> {
        self.0.borrow()
    }

    /// Returns a mutable reference to the list's entries
    pub fn data_mut(&self) -> BorrowMut<ValueVec> {
        self.0.borrow_mut()
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
