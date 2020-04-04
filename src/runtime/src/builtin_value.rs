use downcast_rs::impl_downcast;
pub use downcast_rs::Downcast;
use std::fmt;

pub trait BuiltinValue: fmt::Debug + fmt::Display + Downcast {
    fn value_type(&self) -> String;
}

impl_downcast!(BuiltinValue);
