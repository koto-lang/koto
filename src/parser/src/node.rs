use crate::{AstNode, Lookup, LookupSlice};
use std::sync::Arc;

pub type ConstantIndex = u32;

#[derive(Clone, Debug)]
pub enum Node {
    Empty,
    Id(ConstantIndex),
    Lookup(Lookup),
    Copy(LookupOrId),
    BoolTrue,
    BoolFalse,
    Number(ConstantIndex),
    Str(ConstantIndex),
    Vec4(Vec<AstNode>),
    List(Vec<AstNode>),
    Range {
        start: Box<AstNode>,
        end: Box<AstNode>,
        inclusive: bool,
    },
    RangeFrom {
        start: Box<AstNode>,
    },
    RangeTo {
        end: Box<AstNode>,
        inclusive: bool,
    },
    RangeFull,
    Map(Vec<(ConstantIndex, AstNode)>),
    Block(Vec<AstNode>),
    Expressions(Vec<AstNode>),
    CopyExpression(Box<AstNode>),
    Negate(Box<AstNode>),
    Function(Arc<Function>),
    Call {
        function: LookupOrId,
        args: Vec<AstNode>,
    },
    Debug {
        expressions: Vec<(ConstantIndex, AstNode)>,
    },
    Assign {
        target: AssignTarget,
        expression: Box<AstNode>,
    },
    MultiAssign {
        targets: Vec<AssignTarget>,
        expressions: Vec<AstNode>,
    },
    Op {
        op: AstOp,
        lhs: Box<AstNode>,
        rhs: Box<AstNode>,
    },
    If(AstIf),
    For(Arc<AstFor>),
    While(Arc<AstWhile>),
    Break,
    Continue,
    Return,
    ReturnExpression(Box<AstNode>),
}

impl Default for Node {
    fn default() -> Self {
        Node::Empty
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug)]
pub enum LookupOrId {
    Id(ConstantIndex),
    Lookup(Lookup),
}

impl LookupOrId {
    pub fn as_slice<'a>(&'a self) -> LookupSliceOrId<'a> {
        match self {
            LookupOrId::Id(id) => LookupSliceOrId::Id(*id),
            LookupOrId::Lookup(lookup) => LookupSliceOrId::LookupSlice(lookup.as_slice()),
        }
    }
}

#[derive(Clone, Debug)]
pub enum LookupSliceOrId<'a> {
    Id(ConstantIndex),
    LookupSlice(LookupSlice<'a>),
}

#[derive(Clone, Debug)]
pub struct Block {}

#[derive(Clone, Debug)]
pub struct Function {
    pub args: Vec<ConstantIndex>,
    pub captures: Vec<ConstantIndex>,
    pub body: Vec<AstNode>,
}

#[derive(Clone, Debug)]
pub struct AstFor {
    pub args: Vec<ConstantIndex>,
    pub ranges: Vec<AstNode>,
    pub condition: Option<Box<AstNode>>,
    pub body: Box<AstNode>,
}

#[derive(Clone, Debug)]
pub struct AstWhile {
    pub condition: Box<AstNode>,
    pub body: Box<AstNode>,
    pub negate_condition: bool,
}

#[derive(Clone, Debug)]
pub struct AstIf {
    pub condition: Box<AstNode>,
    pub then_node: Box<AstNode>,
    pub else_if_condition: Option<Box<AstNode>>,
    pub else_if_node: Option<Box<AstNode>>,
    pub else_node: Option<Box<AstNode>>,
}

#[derive(Clone, Copy, Debug)]
pub enum AstOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    And,
    Or,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scope {
    Global,
    Local,
}

#[derive(Clone, Debug)]
pub enum AssignTarget {
    Id { id_index: u32, scope: Scope },
    Lookup(Lookup),
}

impl AssignTarget {
    pub fn to_node(&self) -> Node {
        match self {
            AssignTarget::Id { id_index, .. } => Node::Id(*id_index),
            AssignTarget::Lookup(lookup) => Node::Lookup(lookup.clone()),
        }
    }
}
