use crate::{prelude::*, Result};

/// The underlying Vec type used by [ValueList]
pub type ValueVec = smallvec::SmallVec<[Value; 4]>;

/// The Koto runtime's List type
#[derive(Clone, Debug, Default)]
pub struct ValueList(PtrMut<ValueVec>);

impl ValueList {
    /// Creates an empty list with the given capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self(ValueVec::with_capacity(capacity).into())
    }

    /// Creates a list containing the provided data
    pub fn with_data(data: ValueVec) -> Self {
        Self(data.into())
    }

    /// Creates a list containing the provided slice of [Values](crate::Value)
    pub fn from_slice(data: &[Value]) -> Self {
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
}

impl KotoDisplay for ValueList {
    fn display(
        &self,
        s: &mut StringBuilder,
        vm: &mut Vm,
        ctx: &mut KotoDisplayOptions,
    ) -> Result<()> {
        s.append('[');

        let id = PtrMut::address(&self.0);
        if ctx.is_in_parents(id) {
            s.append("...");
        } else {
            ctx.push_container(id);

            for (i, value) in self.data().iter().enumerate() {
                if i > 0 {
                    s.append(", ");
                }
                value.display(s, vm, ctx)?;
            }

            ctx.pop_container();
        }

        s.append(']');
        Ok(())
    }
}
