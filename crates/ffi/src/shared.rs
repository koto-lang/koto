//! Transport-independent ABI concepts shared by native and wasm plugin transports.

use std::fmt;

/// The current ABI major version.
pub const ABI_MAJOR_VERSION: u16 = 1;

/// The current ABI minor version.
pub const ABI_MINOR_VERSION: u16 = 23;

/// The supported value kinds in the v1 plugin ABI.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KValueKind {
    /// The null value.
    Null,
    /// A boolean value.
    Bool,
    /// A signed 64-bit integer.
    I64,
    /// A 64-bit floating point number.
    F64,
    /// A range value.
    Range,
    /// A UTF-8 string.
    String,
    /// A list value.
    List,
    /// A tuple value.
    Tuple,
    /// A map value.
    Map,
    /// A Koto function.
    Function,
    /// A native function.
    NativeFunction,
    /// An iterator value.
    Iterator,
    /// A plugin-owned object value.
    Object,
    /// A runtime type that the current ABI doesn't expose.
    Unsupported,
}

/// A status code returned by ABI calls.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KotoStatusCode {
    /// The operation succeeded.
    Ok = 0,
    /// The operation failed.
    Error = 1,
}

/// The unary operations supported by the plugin VM facade.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum UnaryOp {
    /// `@debug`
    Debug,
    /// `@display`
    Display,
    /// `@negate`
    Negate,
    /// `@iterator`
    Iterator,
    /// `@next`
    Next,
    /// `@next_back`
    NextBack,
    /// `@size`
    Size,
}

/// The binary operations supported by the plugin VM facade.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BinaryOp {
    /// `@+`
    Add,
    /// `@-`
    Subtract,
    /// `@*`
    Multiply,
    /// `@/`
    Divide,
    /// `@%`
    Remainder,
    /// `@^`
    Power,
    /// `@r+`
    AddRhs,
    /// `@r-`
    SubtractRhs,
    /// `@r*`
    MultiplyRhs,
    /// `@r/`
    DivideRhs,
    /// `@r%`
    RemainderRhs,
    /// `@r^`
    PowerRhs,
    /// `@+=`
    AddAssign,
    /// `@-=`
    SubtractAssign,
    /// `@*=`
    MultiplyAssign,
    /// `@/=`
    DivideAssign,
    /// `@%=`
    RemainderAssign,
    /// `@^=`
    PowerAssign,
    /// `@<`
    Less,
    /// `@<=`
    LessOrEqual,
    /// `@>`
    Greater,
    /// `@>=`
    GreaterOrEqual,
    /// `@==`
    Equal,
    /// `@!=`
    NotEqual,
}

/// The read operations supported by the plugin VM facade.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ReadOp {
    /// `@index`
    Index,
    /// `@access`
    Access,
}

/// The write operations supported by the plugin VM facade.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum WriteOp {
    /// `@index_assign`
    IndexAssign,
    /// `@access_assign`
    AccessAssign,
}

impl fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use BinaryOp::*;

        write!(
            f,
            "{}",
            match self {
                Add | AddRhs => "+",
                Subtract | SubtractRhs => "-",
                Multiply | MultiplyRhs => "*",
                Divide | DivideRhs => "/",
                Remainder | RemainderRhs => "%",
                Power | PowerRhs => "^",
                AddAssign => "+=",
                SubtractAssign => "-=",
                MultiplyAssign => "*=",
                DivideAssign => "/=",
                RemainderAssign => "%=",
                PowerAssign => "^=",
                Less => "<",
                LessOrEqual => "<=",
                Greater => ">",
                GreaterOrEqual => ">=",
                Equal => "==",
                NotEqual => "!=",
            }
        )
    }
}

impl fmt::Display for ReadOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ReadOp::Index => "[]",
                ReadOp::Access => ".",
            }
        )
    }
}

impl fmt::Display for WriteOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                WriteOp::IndexAssign => "[]",
                WriteOp::AccessAssign => ".",
            }
        )
    }
}

/// The kinds of meta keys supported by the plugin ABI.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetaKeyKind {
    /// A unary operation.
    UnaryOp,
    /// A binary operation.
    BinaryOp,
    /// A read operation.
    ReadOp,
    /// A write operation.
    WriteOp,
    /// `@call`
    Call,
    /// `@meta name`
    Named,
    /// `@test name`
    Test,
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

/// Indicates whether a plugin-owned object is iterable.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IterableKind {
    /// The object isn't iterable.
    NotIterable,
    /// The object is iterable and can produce an iterator.
    Iterable,
    /// The object is a forward iterator.
    ForwardIterator,
    /// The object is a bidirectional iterator.
    BidirectionalIterator,
}
