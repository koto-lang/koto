//! Serde serialization and deserialization support for Koto value types

mod deserialize;
mod deserializer;
mod error;
mod serialize;
mod serializer;

#[cfg(feature = "plugin")]
mod plugin_deserialize;
#[cfg(feature = "plugin")]
mod plugin_serialize;

pub use crate::{
    deserialize::DeserializableKValue,
    deserializer::{Deserializer, from_koto_value},
    error::{Error, Result},
    serialize::SerializableKValue,
    serializer::{Serializer, to_koto_value},
};

#[cfg(feature = "plugin")]
pub mod plugin {
    pub use crate::{
        plugin_deserialize::DeserializableKValue, plugin_serialize::SerializableKValue,
    };
}
