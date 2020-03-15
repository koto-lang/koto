use crate::{vec4, AstNode, Lookup};
use std::{fmt, rc::Rc};

pub type Id = Rc<String>;

#[derive(Clone, Debug)]
pub enum Node {
    Empty,
    Id(Id),
    Lookup(Lookup),
    Ref(LookupOrId),
    Bool(bool),
    Number(f64),
    Vec4(vec4::Vec4),
    Str(Rc<String>),
    List(Vec<AstNode>),
    Range {
        start: Box<AstNode>,
        end: Box<AstNode>,
        inclusive: bool,
    },
    IndexRange {
        start: Option<Box<AstNode>>,
        end: Option<Box<AstNode>>,
        inclusive: bool,
    },
    Map(Vec<(Id, AstNode)>),
    Block(Vec<AstNode>),
    Expressions(Vec<AstNode>),
    ReturnExpression(Box<AstNode>),
    RefExpression(Box<AstNode>),
    Negate(Box<AstNode>),
    Function(Rc<Function>),
    Call {
        function: LookupOrId,
        args: Vec<AstNode>,
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
    For(Rc<AstFor>),
    While(Rc<AstWhile>),
    Break,
    Continue,
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Node::*;
        match self {
            Empty => write!(f, "()"),
            Id(id) => write!(f, "Id: {}", id),
            Ref(lookup) => write!(f, "Ref: {}", lookup),
            Bool(b) => write!(f, "Bool: {}", b),
            Number(n) => write!(f, "Number: {}", n),
            Vec4(v) => write!(f, "Vec4: {:?}", v),
            Str(s) => write!(f, "Str: {}", s),
            List(l) => write!(
                f,
                "List with {} {}",
                l.len(),
                if l.len() == 1 { "entry" } else { "entries" }
            ),
            Range { inclusive, .. } => {
                write!(f, "Range: {}", if *inclusive { "..=" } else { ".." },)
            }
            IndexRange { inclusive, .. } => {
                write!(f, "Range: {}", if *inclusive { "..=" } else { ".." },)
            }
            Map(m) => write!(
                f,
                "Map with {} {}",
                m.len(),
                if m.len() == 1 { "entry" } else { "entries" }
            ),
            Block(b) => write!(
                f,
                "Block with {} expression{}",
                b.len(),
                if b.len() == 1 { "" } else { "s" }
            ),
            Expressions(e) => write!(
                f,
                "Expressions with {} expression{}",
                e.len(),
                if e.len() == 1 { "" } else { "s" }
            ),
            ReturnExpression(_) => write!(f, "Return Expression"),
            RefExpression(_) => write!(f, "Ref Expression"),
            Negate(_) => write!(f, "Negate"),
            Function(_) => write!(f, "Function"),
            Call { function, .. } => write!(f, "Call: {}", function),
            Lookup(lookup) => write!(f, "Lookup: {}", lookup),
            Assign { target, .. } => write!(f, "Assign: target: {}", target),
            MultiAssign { targets, .. } => write!(f, "MultiAssign: targets: {:?}", targets,),
            Op { op, .. } => write!(f, "Op: {:?}", op),
            If(_) => write!(f, "If"),
            For(_) => write!(f, "For"),
            While { .. } => write!(f, "While"),
            Break => write!(f, "Break"),
            Continue => write!(f, "Continue"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug)]
pub enum LookupOrId {
    Id(Id),
    Lookup(Lookup),
}

impl fmt::Display for LookupOrId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LookupOrId::Id(id) => write!(f, "Id: {}", id),
            LookupOrId::Lookup(lookup) => lookup.fmt(f),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Block {}

#[derive(Clone, Debug)]
pub struct Function {
    pub args: Vec<Id>,
    pub body: Vec<AstNode>,
}

#[derive(Clone, Debug)]
pub struct AstFor {
    pub args: Vec<Id>,
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
#[derive(Clone, Debug)]
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
    Id { id: Id, scope: Scope },
    Lookup(Lookup),
}

impl fmt::Display for AssignTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use AssignTarget::*;
        match self {
            Id { id, scope } => write!(
                f,
                "{}{}",
                id,
                if *scope == Scope::Global {
                    " - global"
                } else {
                    ""
                }
            ),
            Lookup(lookup) => write!(f, "{}", lookup),
        }
    }
}
