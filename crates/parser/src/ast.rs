use indexmap::IndexSet;
use koto_lexer::Span;
use std::{fmt, num::TryFromIntError};

use crate::{ConstantIndex, ConstantPool, Node, error::*};

#[expect(unused, reason = "Imported for docs")]
use crate::Parser;

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

/// Koto code represented as an Abstract Syntax Tree
///
/// ASTs get produced by the [Parser], using an [AstBuilder].
#[derive(Debug, Default, Clone)]
pub struct Ast {
    nodes: Vec<AstNode>,
    spans: Vec<Span>,
    constants: ConstantPool,
    top_level_assigned_ids: IndexSet<ConstantIndex>,
    accessed_non_locals: IndexSet<ConstantIndex>,
}

impl Ast {
    /// Initializes an Ast with the given capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(capacity),
            spans: Vec::with_capacity(capacity),
            constants: ConstantPool::default(),
            top_level_assigned_ids: IndexSet::new(),
            accessed_non_locals: IndexSet::new(),
        }
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

    /// Returns the IDs assigned in the module's root frame
    ///
    /// The set contains the IDs in the order that they were first assigned.
    pub fn top_level_assigned_ids(&self) -> &IndexSet<ConstantIndex> {
        &self.top_level_assigned_ids
    }

    /// Returns the non-local IDs accessed by the module
    ///
    /// The set contains the IDs in the order that they were first accessed.
    pub fn accessed_non_locals(&self) -> &IndexSet<ConstantIndex> {
        &self.accessed_non_locals
    }

    /// Moves the constants out of the AST
    ///
    /// This is used when building a `Chunk` after compilation.
    /// The constants get transferred to the `Chunk` once the AST has been converted into bytecode.
    pub fn consume_constants(self) -> ConstantPool {
        self.constants
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

impl fmt::Display for Ast {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "constants:")?;
        if self.constants.size() == 0 {
            writeln!(f, "  []")?;
        } else {
            for (i, constant) in self.constants.iter().enumerate() {
                writeln!(f, "  {i}: {constant}")?;
            }
        }

        writeln!(f, "nodes:")?;
        if self.nodes.is_empty() {
            writeln!(f, "  []")?;
        } else {
            for (i, ast_node) in self.nodes.iter().enumerate() {
                writeln!(f, "  {i}: {:?}", ast_node.node)?;
            }
        }

        Ok(())
    }
}

/// A helper for building [Ast]s
pub struct AstBuilder {
    nodes: Vec<AstNode>,
    spans: Vec<Span>,
}

impl AstBuilder {
    /// Initializes an Ast builder with the given capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(capacity),
            spans: Vec::with_capacity(capacity),
        }
    }

    /// Pushes a node and corresponding span into the tree
    pub fn push(&mut self, node: Node, span: Span) -> Result<AstIndex> {
        // We could potentially achieve some compression by using a set for the spans,
        // but a Vec will do for now.
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

    /// Returns the root node in the tree
    pub fn entry_point(&self) -> Option<AstIndex> {
        if self.nodes.is_empty() {
            None
        } else {
            AstIndex::try_from(self.nodes.len() - 1).ok()
        }
    }

    /// Consumes the builder along with some metadata and returns an [Ast]
    pub fn build(
        self,
        constants: ConstantPool,
        top_level_assigned_ids: IndexSet<ConstantIndex>,
        accessed_non_locals: IndexSet<ConstantIndex>,
    ) -> Ast {
        Ast {
            nodes: self.nodes,
            spans: self.spans,
            constants,
            top_level_assigned_ids,
            accessed_non_locals,
        }
    }
}
