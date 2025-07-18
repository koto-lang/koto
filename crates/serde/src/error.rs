use koto_runtime::KNumber;
use std::fmt;
use thiserror::Error;

/// The result type used when serializing to or from [KValue][koto_runtime::KValue]s
pub type Result<T> = std::result::Result<T, Error>;

/// The error type used when serializing to or from [KValue][koto_runtime::KValue]s
#[derive(Error, Debug, Clone)]
pub enum Error {
    #[error("{0}")]
    Message(String),

    #[error("failed to make KotoObject serializable '{0}'")]
    FailedToSerializeKObject(String),
    #[error("missing map key for value")]
    MissingMapKey,
    #[error("number out of i64 range {0}")]
    OutOfI64RangeNumber(KNumber),
    #[error("number out of u8 range {0}")]
    OutOfU8RangeNumber(KNumber),
    #[error("i128 out of i64 range {0}")]
    OutOfRangeI128(i128),
    #[error("u64 out of i64 range {0}")]
    OutOfRangeU64(u64),
    #[error("u128 out of i64 range {0}")]
    OutOfRangeU128(u128),
    #[error("expected char, found string containing {} chars", .0.chars().count())]
    StringDoesntContainSingleChar(String),

    #[error("{0} is unsupported")]
    Unsupported(String),
}

impl serde::de::Error for Error {
    fn custom<T>(message: T) -> Self
    where
        T: fmt::Display,
    {
        Self::Message(message.to_string())
    }
}

impl serde::ser::Error for Error {
    fn custom<T>(message: T) -> Self
    where
        T: fmt::Display,
    {
        Self::Message(message.to_string())
    }
}
