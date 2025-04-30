#![expect(unused)]

use std::{cell::OnceCell, iter::Peekable, thread::LocalKey};

use crate::{
    FormatOptions, Result, Trivia,
    trivia::{TriviaItem, TriviaIterator, TriviaToken},
};
use koto_lexer::Position;
use koto_parser::{
    Ast, AstIndex, AstNode, AstUnaryOp, AstVec, ConstantIndex, KString, Node, Span, StringSlice,
};

/// Returns the input source formatted according to the provided options
pub fn format(source: &str, options: FormatOptions) -> Result<String> {
    let trivia = Trivia::parse(source)?;
    let ast = koto_parser::Parser::parse(source)?;

    if let Some(entry_point) = ast.entry_point() {
        let context = FormatContext { source, ast: &ast };
        let output = format_node(entry_point, context, &mut trivia.iter());
        let mut result = String::new();
        output.render(&mut result, &options, 0);
        Ok(result)
    } else {
        Ok(String::new())
    }
}

fn format_node<'source>(
    node_index: AstIndex,
    ctx: FormatContext<'source>,
    trivia: &mut TriviaIterator<'source>,
) -> FormatItem<'source> {
    let node = ctx.node(node_index);

    match &node.node {
        Node::Null => FormatItem::Str("null"),
        Node::Nested(nested) => GroupBuilder::new(3, node, ctx, trivia)
            .char('(')
            .node(*nested)
            .char(')')
            .build(),
        Node::Id(index, type_hint) => {
            if let Some(type_hint) = type_hint {
                todo!()
            } else {
                ctx.string_constant(*index).into()
            }
        }
        Node::Meta(meta_key_id, maybe_name) => {
            if let Some(name) = maybe_name {
                GroupBuilder::new(3, node, ctx, trivia)
                    .string(meta_key_id.as_str())
                    .soft_break()
                    .string_constant(*name)
                    .build()
            } else {
                meta_key_id.as_str().into()
            }
        }
        Node::Chain(_) => todo!(),
        Node::BoolTrue => "true".into(),
        Node::BoolFalse => "false".into(),
        Node::SmallInt(n) => FormatItem::SmallInt(*n),
        Node::Int(index) => FormatItem::Int(ctx.ast.constants().get_i64(*index)),
        Node::Float(index) => FormatItem::Float(ctx.ast.constants().get_f64(*index)),
        Node::Str(ast_string) => todo!(),
        Node::List(elements) => GroupBuilder::new(elements.len() * 2 + 2, node, ctx, trivia)
            .char('[')
            .elements(elements)
            .char(']')
            .build(),
        Node::Tuple(elements) => GroupBuilder::new(elements.len() * 2 + 2, node, ctx, trivia)
            .char('(')
            .elements(elements)
            .char(')')
            .build(),
        Node::TempTuple(elements) => GroupBuilder::new(elements.len() * 2, node, ctx, trivia)
            .elements(elements)
            .build(),
        Node::Range {
            start,
            end,
            inclusive,
        } => todo!(),
        Node::RangeFrom { start } => todo!(),
        Node::RangeTo { end, inclusive } => todo!(),
        Node::RangeFull => todo!(),
        Node::Map(small_vec) => todo!(),
        Node::Self_ => "self".into(),
        Node::MainBlock { body, .. } => {
            let mut group = GroupBuilder::new(body.len(), node, ctx, trivia);
            for block_node in body {
                group = group.node(*block_node).line_break()
            }
            group.build_main_block()
        }
        Node::Block(body) => {
            let mut group = GroupBuilder::new(body.len(), node, ctx, trivia).line_break();
            for block_node in body {
                group = group.node(*block_node).line_break()
            }
            group.build_block()
        }
        Node::Function(function) => todo!(),
        Node::Import { from, items } => todo!(),
        Node::Export(ast_index) => todo!(),
        Node::Assign { target, expression } => GroupBuilder::new(3, node, ctx, trivia)
            .node(*target)
            .soft_break()
            .nested(|trivia| {
                GroupBuilder::new(3, node, ctx, trivia)
                    .char('=')
                    .soft_break()
                    .node(*expression)
                    .build()
            })
            .build(),
        Node::MultiAssign {
            targets,
            expression,
        } => todo!(),
        Node::UnaryOp { op, value } => match op {
            AstUnaryOp::Negate => GroupBuilder::new(2, node, ctx, trivia)
                .string(op.as_str())
                .node(*value)
                .build(),
            AstUnaryOp::Not => GroupBuilder::new(3, node, ctx, trivia)
                .string(op.as_str())
                .soft_break()
                .node(*value)
                .build(),
        },
        Node::BinaryOp { op, lhs, rhs } => GroupBuilder::new(3, node, ctx, trivia)
            .node_flattened(*lhs)
            .soft_break()
            .nested(|trivia| {
                GroupBuilder::new(3, node, ctx, trivia)
                    .string(op.as_str())
                    .soft_break()
                    .node(*rhs)
                    .build()
            })
            .build(),
        Node::If(ast_if) => todo!(),
        Node::Match { expression, arms } => todo!(),
        Node::Switch(small_vec) => todo!(),
        Node::Wildcard(constant_index, ast_index) => todo!(),
        Node::PackedId(constant_index) => todo!(),
        Node::PackedExpression(ast_index) => todo!(),
        Node::For(ast_for) => todo!(),
        Node::Loop { body } => GroupBuilder::new(2, node, ctx, trivia)
            .string("loop")
            .node(*body)
            .build(),
        Node::While { condition, body } => todo!(),
        Node::Until { condition, body } => todo!(),
        Node::Break(value) => match value {
            Some(value) => GroupBuilder::new(3, node, ctx, trivia)
                .string("break")
                .soft_break()
                .node(*value)
                .build(),
            None => "break".into(),
        },
        Node::Continue => "continue".into(),
        Node::Return(value) => match value {
            Some(value) => GroupBuilder::new(3, node, ctx, trivia)
                .string("return")
                .soft_break()
                .node(*value)
                .build(),
            None => "return".into(),
        },
        Node::Try(ast_try) => todo!(),
        Node::Throw(ast_index) => todo!(),
        Node::Yield(ast_index) => todo!(),
        Node::Debug {
            expression_string,
            expression,
        } => todo!(),
        Node::Type {
            type_index,
            allow_null,
        } => {
            let type_string = ctx.string_constant(*type_index).into();
            if *allow_null {
                GroupBuilder::new(2, node, ctx, trivia)
                    .string_constant(*type_index)
                    .char('?')
                    .build()
            } else {
                type_string
            }
        }
    }
}

#[derive(Clone, Copy)]
struct FormatContext<'source> {
    source: &'source str,
    ast: &'source Ast,
}

impl<'source> FormatContext<'source> {
    fn node(&self, ast_index: AstIndex) -> &AstNode {
        self.ast.node(ast_index)
    }

    fn span(&self, node: &AstNode) -> &Span {
        self.ast.span(node.span)
    }

    fn string_constant(&self, constant: ConstantIndex) -> &'source str {
        self.ast.constants().get_str(constant)
    }
}

struct GroupBuilder<'source, 'trivia> {
    items: Vec<FormatItem<'source>>,
    group_span: Span,
    ctx: FormatContext<'source>,
    trivia: &'trivia mut TriviaIterator<'source>,
    current_line: u32,
}

impl<'source, 'trivia> GroupBuilder<'source, 'trivia> {
    fn new(
        capacity: usize,
        group_node: &AstNode,
        ctx: FormatContext<'source>,
        trivia: &'trivia mut TriviaIterator<'source>,
    ) -> Self {
        let group_span = *ctx.span(group_node);
        let current_line = group_span.start.line;
        Self {
            items: Vec::with_capacity(capacity),
            group_span,
            ctx,
            trivia,
            current_line,
        }
    }

    fn build(mut self) -> FormatItem<'source> {
        self.add_trivia(self.group_span.end);

        FormatItem::Group {
            items: self.items,
            line_length: OnceCell::new(),
        }
    }

    fn build_block(mut self) -> FormatItem<'source> {
        self.add_trivia(self.group_span.end);
        FormatItem::Block(self.items)
    }

    fn build_main_block(mut self) -> FormatItem<'source> {
        self.add_trivia(self.group_span.end);
        // Remove trailing linebreaks
        while self
            .items
            .last()
            .is_some_and(|item| matches!(item, FormatItem::LineBreak))
        {
            self.items.pop();
        }
        FormatItem::MainBlock(self.items)
    }

    fn char(mut self, c: char) -> Self {
        self.items.push(c.into());
        self
    }

    fn string(mut self, s: &'source str) -> Self {
        self.items.push(s.into());
        self
    }

    fn string_constant(mut self, constant: ConstantIndex) -> Self {
        self.items.push(self.ctx.string_constant(constant).into());
        self
    }

    fn soft_break(mut self) -> Self {
        self.items.push(FormatItem::SoftBreak);
        self
    }

    fn line_break(mut self) -> Self {
        // Add any trailing comments for the current line
        self.add_trivia(self.next_line());
        self.items.push(FormatItem::LineBreak);
        self
    }

    fn node(mut self, node_index: AstIndex) -> Self {
        let node_span = self.ctx.span(self.ctx.node(node_index));
        let node_end_line = node_span.end.line;
        self.add_trivia(node_span.start);
        self.items
            .push(format_node(node_index, self.ctx, self.trivia));
        self.current_line = node_end_line;
        self
    }

    // Adds the node, and then if it's a group, flattens its contents into this group
    fn node_flattened(mut self, node_index: AstIndex) -> Self {
        self = self.node(node_index);
        if let Some(FormatItem::Group { items, .. }) = self
            .items
            .pop_if(|item| matches!(item, FormatItem::Group { .. }))
        {
            self.items.extend(items);
        }
        self
    }

    fn nested(
        mut self,
        nested_fn: impl Fn(&mut TriviaIterator<'source>) -> FormatItem<'source>,
    ) -> Self {
        self.items.push(nested_fn(self.trivia));
        self
    }

    fn elements(mut self, elements: &[AstIndex]) -> Self {
        for (i, element) in elements.iter().enumerate() {
            self = self.node(*element);
            if i < elements.len() - 1 {
                self = self.char(',').soft_break();
            }
        }
        self
    }

    fn add_trivia(&mut self, position: Position) {
        // Add any trivia items that belong before the format item to the group
        while let Some(item) = self.trivia.peek() {
            let item_start = item.span.start;
            if item_start < position {
                // Advance the trivia iterator
                let item = *item;
                self.trivia.next();

                match item.token {
                    TriviaToken::EmptyLine => {
                        self.items.push(FormatItem::LineBreak);
                    }
                    TriviaToken::CommentSingle(text) => {
                        // Insert a space if it's a trailing comment
                        if item_start.line == self.current_line {
                            self.items.push(' '.into());
                        }

                        self.items.push(text.into());
                    }
                    TriviaToken::CommentMulti(text) => {
                        self.items.push(text.into());
                        self.items.push(FormatItem::SoftBreak);
                    }
                }
            } else {
                break;
            }
        }
    }

    fn next_line(&self) -> Position {
        Position {
            line: self.current_line + 1,
            column: 0,
        }
    }
}

#[derive(Debug)]
enum FormatItem<'source> {
    // A single character
    Char(char),
    // A `&str`, either static or from the source file
    Str(&'source str),
    // A small int
    SmallInt(i16),
    /// An integer outside of the range -255..=255
    Int(i64),
    /// A float literal
    Float(f64),
    // A linebreak
    LineBreak,
    // A space or indented linebreak
    SoftBreak,
    // A grouped sequence of items
    // The group will be rendered on a single line, or if the group doesn't fit in the remaining
    // space then it will be rendered with softbreaks replaced by indented newlines
    Group {
        items: Vec<FormatItem<'source>>,
        // The group's length if it was rendered on a single line
        // This gets calculated and cached during rendering to avoid nested group recalculations
        line_length: OnceCell<usize>,
    },
    // A sequence of expressions, each on a separate line.
    MainBlock(Vec<FormatItem<'source>>),
    // An indented sequence of expressions, each on a separate line.
    Block(Vec<FormatItem<'source>>),
}

impl FormatItem<'_> {
    fn render(&self, output: &mut String, options: &FormatOptions, column: usize) {
        match self {
            Self::Char(c) => output.push(*c),
            Self::Str(s) => output.push_str(s),
            Self::SmallInt(n) => output.push_str(&n.to_string()),
            Self::Int(n) => output.push_str(&n.to_string()),
            Self::Float(n) => output.push_str(&n.to_string()),
            Self::LineBreak => output.push('\n'),
            Self::SoftBreak => output.push(' '),
            Self::Group { items, .. } => {
                let columns_remaining = (options.line_length as usize).saturating_sub(column);
                if self.line_length() <= columns_remaining {
                    for item in items {
                        item.render(output, options, column);
                    }
                } else {
                    let indented_column = column + options.indent_width as usize;
                    let indent = " ".repeat(indented_column);
                    for item in items {
                        match item {
                            Self::SoftBreak => {
                                output.push('\n');
                                output.push_str(&indent);
                            }
                            _ => {
                                item.render(output, options, indented_column);
                            }
                        }
                    }
                }
            }
            Self::MainBlock(items) => {
                for item in items {
                    item.render(output, options, column);
                }
                // A newline may already be present at the end of the output, if not add one now.
                if !output.ends_with('\n') {
                    output.push('\n');
                }
            }
            Self::Block(items) => {
                let block_column = column + options.indent_width as usize;
                let indent = " ".repeat(block_column);
                let mut last_item_was_linebreak = false;
                for item in items {
                    last_item_was_linebreak =
                        if !matches!(item, FormatItem::LineBreak) && last_item_was_linebreak {
                            output.push_str(&indent);
                            false
                        } else {
                            true
                        };
                    item.render(output, options, block_column);
                }
            }
        }
    }

    // Gets the length of the format item
    fn line_length(&self) -> usize {
        use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

        match self {
            Self::Char(c) => c.width().unwrap_or(0),
            Self::Str(s) => s.width(),
            Self::SmallInt(n) => integer_length(*n as i64),
            Self::Int(i) => integer_length(*i),
            Self::Float(f) => todo!(),
            Self::LineBreak => 0,
            Self::SoftBreak => 1, // Rendered as a space when used in a line
            Self::Group { line_length, items } => {
                *line_length.get_or_init(|| items.iter().map(Self::line_length).sum())
            }
            // Don't bother calculating lengths of items that are broken over lines
            Self::MainBlock(_) | Self::Block(_) | Self::LineBreak => 0,
        }
    }
}

impl From<char> for FormatItem<'_> {
    fn from(c: char) -> Self {
        Self::Char(c)
    }
}

impl<'source> From<&'source str> for FormatItem<'source> {
    fn from(s: &'source str) -> Self {
        Self::Str(s)
    }
}

fn integer_length(i: i64) -> usize {
    (i / 10 + if i >= 0 { 1 } else { 2 }) as usize
}
