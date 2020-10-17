use {
    crate::Value,
    std::{fmt, sync::Arc},
};

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ValueTuple(Arc<[Value]>);

impl ValueTuple {
    pub fn data(&self) -> &[Value] {
        &self.0
    }
}

impl fmt::Display for ValueTuple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(")?;
        for (i, value) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{}", value)?;
        }
        write!(f, ")")
    }
}

impl From<&[Value]> for ValueTuple {
    fn from(v: &[Value]) -> Self {
        Self(v.into())
    }
}

impl From<Vec<Value>> for ValueTuple {
    fn from(v: Vec<Value>) -> Self {
        Self(v.into())
    }
}
