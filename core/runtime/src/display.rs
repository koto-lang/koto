use koto_memory::Address;

use crate::{Result, StringBuilder, Vm};

/// A trait for Koto runtime values that need custom display behaviour
pub trait KotoDisplay {
    /// Prepares a display string for the value
    ///
    /// The VM needs to be provided so that values with custom @display implementations will be
    /// displayed correcty.
    fn display(
        &self,
        s: &mut StringBuilder,
        vm: &mut Vm,
        options: &mut KotoDisplayOptions,
    ) -> Result<()>;
}

/// Options for the [KotoDisplay] trait
#[derive(Clone, Default)]
pub struct KotoDisplayOptions {
    // A contained value might need to be displayed differently,
    // - Strings should be displayed with quotes when they're inside a container.
    // - Containers should check the parent list to avoid recursive display operations.
    parent_containers: Vec<Address>,
}

impl KotoDisplayOptions {
    /// Returns true if the value that's being displayed is in a container
    pub fn is_contained(&self) -> bool {
        !self.parent_containers.is_empty()
    }

    /// Returns true if the given ID is present in the parent container list
    pub fn is_in_parents(&self, id: Address) -> bool {
        self.parent_containers
            .iter()
            .any(|parent_id| *parent_id == id)
    }

    /// Adds the given ID to the parents list
    ///
    /// Containers should call this before displaying their contained values.
    pub fn push_container(&mut self, id: Address) {
        self.parent_containers.push(id);
    }

    /// Pops the previously added parent ID
    ///
    /// Containers should call this after displaying their contained values.
    pub fn pop_container(&mut self) {
        self.parent_containers.pop();
    }
}
