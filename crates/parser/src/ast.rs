use koto_lexer::Span;
use std::{fmt, num::TryFromIntError};

use crate::{error::*, ConstantPool, Node};

/// The index type used by nodes in the [Ast]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct AstIndex(u32);

impl From<AstIndex> for u32 {
    fn from(value: AstIndex) -> Self {
        value.0
    }
}

impl From<AstIndex> for usize {
    fn from(value: AstIndex) -> Self {
        value.0 as usize
    }
}

impl From<u32> for AstIndex {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<&u32> for AstIndex {
    fn from(value: &u32) -> Self {
        Self(*value)
    }
}

impl TryFrom<usize> for AstIndex {
    type Error = TryFromIntError;

    fn try_from(value: usize) -> std::result::Result<Self, Self::Error> {
        Ok(Self(u32::try_from(value)?))
    }
}

impl fmt::Display for AstIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A [Node] in the [Ast], along with its corresponding [Span]
#[derive(Clone, Debug, Default)]
pub struct AstNode {
    /// The node itself
    pub node: Node,
    /// The index of the node's corresponding [Span]
    ///
    /// The span is stored in the [Ast], and retrieved via [Ast::span].
    pub span: AstIndex,
}

/// A Koto program represented as an Abstract Syntax Tree
///
/// This is produced by the parser, and consumed by the compiler.
#[derive(Debug, Default)]
pub struct Ast {
    nodes: Vec<AstNode>,
    spans: Vec<Span>,
    constants: ConstantPool,
}

impl Ast {
    /// Initializes an Ast with the given capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(capacity),
            spans: Vec::with_capacity(capacity),
            constants: ConstantPool::default(),
        }
    }

    /// Pushes a node and corresponding span onto the tree
    pub fn push(&mut self, node: Node, span: Span) -> Result<AstIndex> {
        // We could potentially achieve some compression by
        // using a set for the spans, for now a Vec will do.
        self.spans.push(span);
        let span_index = AstIndex::try_from(self.spans.len() - 1)
            .map_err(|_| Error::new(InternalError::AstCapacityOverflow.into(), span))?;

        self.nodes.push(AstNode {
            node,
            span: span_index,
        });
        AstIndex::try_from(self.nodes.len() - 1)
            .map_err(|_| Error::new(InternalError::AstCapacityOverflow.into(), span))
    }

    /// Returns a node for a given node index
    pub fn node(&self, index: AstIndex) -> &AstNode {
        &self.nodes[usize::from(index)]
    }

    /// Returns a span for a given span index
    pub fn span(&self, index: AstIndex) -> &Span {
        &self.spans[usize::from(index)]
    }

    /// Returns the constant pool referred to by the AST
    pub fn constants(&self) -> &ConstantPool {
        &self.constants
    }

    /// Moves the constants out of the AST
    ///
    /// This is used when building a `Chunk` after compilation.
    /// The constants get transferred to the `Chunk` once the AST has been converted into bytecode.
    pub fn consume_constants(self) -> ConstantPool {
        self.constants
    }

    pub(crate) fn set_constants(&mut self, constants: ConstantPool) {
        self.constants = constants
    }

    /// Returns the root node in the tree
    pub fn entry_point(&self) -> Option<AstIndex> {
        if self.nodes.is_empty() {
            None
        } else {
            AstIndex::try_from(self.nodes.len() - 1).ok()
        }
    }

    /// Used in testing to validate the tree's contents
    pub fn nodes(&self) -> &[AstNode] {
        &self.nodes
    }
}
