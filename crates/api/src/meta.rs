pub use koto_ffi::{BinaryOp, ReadOp, UnaryOp, WriteOp};

/// Meta keys shared by Koto backends.
///
/// The key is generic over the backend's string type so that runtime and plugin
/// can keep using their own wrapper types without duplicating the enum shape.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum MetaKey<S> {
    /// A binary operation.
    BinaryOp(BinaryOp),
    /// A unary operation.
    UnaryOp(UnaryOp),
    /// A read operation.
    ReadOp(ReadOp),
    /// A write operation.
    WriteOp(WriteOp),
    /// Function call, `@call`.
    Call,
    /// A named key, `@meta name`.
    Named(S),
    /// A named test, `@test name`.
    Test(S),
    /// `@pre_test`
    PreTest,
    /// `@post_test`
    PostTest,
    /// `@main`
    Main,
    /// `@type`
    Type,
    /// `@base`
    Base,
}

impl<S> From<UnaryOp> for MetaKey<S> {
    fn from(op: UnaryOp) -> Self {
        Self::UnaryOp(op)
    }
}

impl<S> From<BinaryOp> for MetaKey<S> {
    fn from(op: BinaryOp) -> Self {
        Self::BinaryOp(op)
    }
}

impl<S> From<ReadOp> for MetaKey<S> {
    fn from(op: ReadOp) -> Self {
        Self::ReadOp(op)
    }
}

impl<S> From<WriteOp> for MetaKey<S> {
    fn from(op: WriteOp) -> Self {
        Self::WriteOp(op)
    }
}
