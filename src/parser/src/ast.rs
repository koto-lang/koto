use {
    crate::{error::*, Node},
    koto_lexer::Span,
    std::convert::TryFrom,
};

pub type AstIndex = u32;

#[derive(Clone, Default)]
pub struct AstNode {
    pub node: Node,
    pub span: AstIndex,
}

#[derive(Default)]
pub struct Ast {
    nodes: Vec<AstNode>,
    spans: Vec<Span>,
    entry_point: u32,
}

impl Ast {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(capacity),
            spans: Vec::with_capacity(capacity),
            entry_point: 0,
        }
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
        self.spans.clear();
        self.entry_point = 0;
    }

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

    pub fn node(&self, index: AstIndex) -> &AstNode {
        &self.nodes[index as usize]
    }

    pub fn span(&self, index: AstIndex) -> &Span {
        &self.spans[index as usize]
    }

    pub fn set_entry_point(&mut self, index: AstIndex) {
        self.entry_point = index;
    }

    pub fn entry_point(&self) -> Option<&AstNode> {
        self.nodes.get(self.entry_point as usize)
    }

    #[cfg(test)]
    pub fn nodes(&self) -> &[AstNode] {
        &self.nodes
    }
}
