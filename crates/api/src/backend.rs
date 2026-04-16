use crate::{KotoMapSource, KotoNumber, KotoRange, KotoSequence, KotoString, KotoValue};
use std::fmt;

/// Backend-specific types used by the shared Koto API traits.
pub trait KotoBackend: Sized {
    /// The backend's error type.
    type Error;

    /// The backend's value type.
    type Value: KotoValue<Self>;

    /// The backend's number type.
    type Number: KotoNumber;

    /// The backend's range type.
    type Range: KotoRange;

    /// The backend's string type.
    type String: KotoString;

    /// The backend's list type.
    type List: KotoSequence<Self>;

    /// The backend's tuple type.
    type Tuple: KotoSequence<Self>;

    /// The backend's map type.
    type Map: KotoMapSource<Self>;

    /// The backend's object type.
    type Object;

    /// The backend's iterator type.
    type Iterator;

    /// The backend's iterator-output type.
    type IteratorOutput;

    /// The backend's function type.
    type Function;

    /// The backend's native-function type.
    type NativeFunction;

    /// The backend's VM type.
    type Vm;

    /// The backend's display-context type.
    type DisplayContext<'a>: fmt::Write
    where
        Self: 'a;

    /// The backend's call-context type.
    type CallContext<'a>
    where
        Self: 'a;

    /// Returns an unimplemented-object-operation error.
    fn unimplemented_object_op<T>(
        op: &'static str,
        object_type: Self::String,
    ) -> Result<T, Self::Error>;

    /// Returns true if the given error represents an unimplemented operation.
    fn is_unimplemented_error(error: &Self::Error) -> bool;
}
