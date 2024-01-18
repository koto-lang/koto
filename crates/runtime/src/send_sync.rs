//! Definitions of Send and Sync used in the Koto runtime
//!
//! When Koto is being used in a single-threaded context [KotoSend] and [KotoSync] are empty
//! traits implemented for all types.

#[cfg(feature = "rc")]
mod traits {
    /// An empty trait for single-threaded contexts, implemented for all types
    pub trait KotoSend {}
    impl<T> KotoSend for T {}

    /// An empty trait for single-threaded contexts, implemented for all types
    pub trait KotoSync {}
    impl<T> KotoSync for T {}
}

#[cfg(not(feature = "rc"))]
mod traits {
    pub use Send as KotoSend;
    pub use Sync as KotoSync;
}

pub use traits::*;
