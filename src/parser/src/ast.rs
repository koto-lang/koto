use {
    crate::{error::*, ConstantPool, Node},
    koto_lexer::Span,
    std::convert::TryFrom,
};

/// The index type used by nodes in the [Ast]
pub type AstIndex = u32;

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
    entry_point: u32,
}

impl Ast {
    /// Initializes an Ast with the given capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(capacity),
            spans: Vec::with_capacity(capacity),
            constants: ConstantPool::default(),
            entry_point: 0,
        }
    }

    /// Pushes a node and corresponding span onto the tree
    pub fn push(&mut self, node: Node, span: Span) -> Result<AstIndex, ParserError> {
        // We could potentially achieve some compression by
        // using a set for the spans, for now a Vec will do.
        self.spans.push(span);
        let span_index = AstIndex::try_from(self.spans.len() - 1)
            .map_err(|_| ParserError::new(InternalError::AstCapacityOverflow.into(), span))?;

        self.nodes.push(AstNode {
            node,
            span: span_index,
        });
        AstIndex::try_from(self.nodes.len() - 1)
            .map_err(|_| ParserError::new(InternalError::AstCapacityOverflow.into(), span))
    }

    /// Pushes a node onto the tree, associating it with an existing span
    pub fn push_with_span_index(
        &mut self,
        node: Node,
        span_index: AstIndex,
    ) -> Result<AstIndex, ParserError> {
        self.nodes.push(AstNode {
            node,
            span: span_index,
        });
        AstIndex::try_from(self.nodes.len() - 1).map_err(|_| {
            ParserError::new(
                InternalError::AstCapacityOverflow.into(),
                *self.span(span_index),
            )
        })
    }

    /// Returns a node for a given node index
    pub fn node(&self, index: AstIndex) -> &AstNode {
        &self.nodes[index as usize]
    }

    /// Returns a span for a given span index
    pub fn span(&self, index: AstIndex) -> &Span {
        &self.spans[index as usize]
    }

    /// Returns the constant pool referred to by the AST
    pub fn constants(&self) -> &ConstantPool {
        &self.constants
    }

    /// Moves the constants out of the AST
    ///
    /// This is used when building a [Chunk] after compilation. The constants get transferred to
    /// the Chunk once the AST has been converted into bytecode.
    pub fn consume_constants(self) -> ConstantPool {
        self.constants
    }

    pub(crate) fn set_constants(&mut self, constants: ConstantPool) {
        self.constants = constants
    }

    /// Returns the root node in the tree
    pub fn entry_point(&self) -> Option<&AstNode> {
        self.nodes.get(self.entry_point as usize)
    }

    // Sets the entry point for the AST
    //
    // In practice this will always be the last node in the nodes list,
    // so this could likely be removed.
    pub(crate) fn set_entry_point(&mut self, index: AstIndex) {
        self.entry_point = index;
    }

    /// Used in testing to validate the tree's contents
    pub fn nodes(&self) -> &[AstNode] {
        &self.nodes
    }
}
