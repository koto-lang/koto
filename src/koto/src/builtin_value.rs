use std::fmt;

pub trait BuiltinValue: fmt::Debug + fmt::Display {
    fn value_type(&self) -> String;
}
