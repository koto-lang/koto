//! Serde serialization and deserialization support for Koto value types

mod deserialize;
mod serialize;

pub use crate::{deserialize::DeserializableKValue, serialize::SerializableKValue};
