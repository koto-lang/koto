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
        options: KotoDisplayOptions,
    ) -> Result<()>;
}

/// Options for the [KotoDisplay] trait
#[derive(Clone, Copy, Default)]
pub struct KotoDisplayOptions {
    /// A contained value might need to be displayed differently,
    /// e.g., Strings should be displayed with quotes when they're inside a container.
    pub contained_value: bool,
}
