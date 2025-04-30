#![expect(unused)]

use std::{cell::OnceCell, iter::Peekable, thread::LocalKey};

use crate::{
    FormatOptions, Result, Trivia,
    trivia::{TriviaItem, TriviaIterator, TriviaToken},
};
use koto_lexer::Position;
use koto_parser::{
    Ast, AstFor, AstIf, AstIndex, AstNode, AstString, AstUnaryOp, AstVec, ChainNode, ConstantIndex,
    ConstantPool, KString, Node, Span, StringAlignment, StringContents, StringFormatOptions,
    StringNode, StringSlice,
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
                    .str(meta_key_id.as_str())
                    .soft_break()
                    .string_constant(*name)
                    .build()
            } else {
                meta_key_id.as_str().into()
            }
        }
        Node::Chain((root_node, next)) => {
            let mut group = GroupBuilder::new(2, node, ctx, trivia);
            let mut node = node;
            let mut chain_node = root_node;
            let mut chain_next = next;

            loop {
                match chain_node {
                    ChainNode::Root(root) => {
                        group = group.node(*root);
                    }
                    ChainNode::Id(id) => {
                        group = group.optional_break().char('.').string_constant(*id);
                    }
                    ChainNode::Str(s) => {
                        group = group
                            .optional_break()
                            .char('.')
                            .nested(|trivia| format_string(s, node, ctx, trivia));
                    }
                    ChainNode::Index(index) => {
                        group = group.nested(|trivia| {
                            GroupBuilder::new(3, node, ctx, trivia)
                                .char('[')
                                .node(*index)
                                .char(']')
                                .build()
                        })
                    }
                    ChainNode::Call { args, with_parens } => {
                        group = group.nested(|trivia| {
                            let mut group = GroupBuilder::new(args.len() * 3, node, ctx, trivia);

                            if *with_parens {
                                group = group.char('(');
                            } else {
                                group = group.soft_break();
                            }

                            for (i, arg) in args.iter().enumerate() {
                                group = group.node(*arg);

                                if i < args.len() - 1 {
                                    group = group.char(',').soft_break();
                                }
                            }

                            if *with_parens {
                                group = group.char(')');
                            }
                            group.build()
                        })
                    }
                    ChainNode::NullCheck => {
                        group = group.char('?');
                    }
                }

                if let Some(next) = chain_next {
                    node = ctx.node(*next);
                    match &node.node {
                        Node::Chain((next_chain_node, next_next)) => {
                            chain_node = next_chain_node;
                            chain_next = next_next;
                        }
                        _other => todo!("Expected chain node"),
                    }
                } else {
                    break;
                }
            }

            group.build()
        }
        Node::BoolTrue => "true".into(),
        Node::BoolFalse => "false".into(),
        Node::SmallInt(n) => FormatItem::SmallInt(*n),
        Node::Int(index) => FormatItem::Int(ctx.ast.constants().get_i64(*index)),
        Node::Float(index) => FormatItem::Float(ctx.ast.constants().get_f64(*index)),
        Node::Str(s) => format_string(s, node, ctx, trivia),
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
            group.build_block(true)
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
                .str(op.as_str())
                .node(*value)
                .build(),
            AstUnaryOp::Not => GroupBuilder::new(3, node, ctx, trivia)
                .str(op.as_str())
                .soft_break()
                .node(*value)
                .build(),
        },
        Node::BinaryOp { op, lhs, rhs } => GroupBuilder::new(3, node, ctx, trivia)
            .node_flattened(*lhs)
            .soft_break()
            .nested(|trivia| {
                GroupBuilder::new(3, node, ctx, trivia)
                    .str(op.as_str())
                    .soft_break()
                    .node(*rhs)
                    .build()
            })
            .build(),
        Node::If(AstIf {
            condition,
            then_node,
            else_if_blocks,
            else_node,
        }) => {
            // TODO: Support for single-line if expressions

            let mut group = GroupBuilder::new(4, node, ctx, trivia).nested(|trivia| {
                GroupBuilder::new(3, node, ctx, trivia)
                    .str("if")
                    .soft_break()
                    .node(*condition)
                    .node(*then_node)
                    .build()
            });

            for (else_if_condition, else_if_block) in else_if_blocks {
                group = group.line_break().nested(|trivia| {
                    GroupBuilder::new(3, node, ctx, trivia)
                        .str("else if")
                        .soft_break()
                        .node(*else_if_condition)
                        .node(*else_if_block)
                        .build()
                });
            }

            if let Some(else_block) = else_node {
                group = group.line_break().nested(|trivia| {
                    GroupBuilder::new(3, node, ctx, trivia)
                        .str("else")
                        .node(*else_block)
                        .build()
                });
            }

            group.build_block(false)
        }
        Node::Match { expression, arms } => todo!(),
        Node::Switch(small_vec) => todo!(),
        Node::Wildcard(constant_index, ast_index) => todo!(),
        Node::PackedId(constant_index) => todo!(),
        Node::PackedExpression(ast_index) => todo!(),
        Node::For(AstFor {
            args,
            iterable,
            body,
        }) => {
            let mut group = GroupBuilder::new((args.len() * 3 - 1) + 6, node, ctx, trivia)
                .str("for")
                .soft_break();
            for (i, arg) in args.iter().enumerate() {
                group = group.node(*arg);
                if i < args.len() - 1 {
                    group = group.char(',');
                }
                group = group.soft_break()
            }
            group
                .str("in")
                .soft_break()
                .node(*iterable)
                .node(*body)
                .build()
        }
        Node::Loop { body } => GroupBuilder::new(2, node, ctx, trivia)
            .str("loop")
            .node(*body)
            .build(),
        Node::While { condition, body } | Node::Until { condition, body } => {
            GroupBuilder::new(4, node, ctx, trivia)
                .str(if matches!(&node.node, Node::While { .. }) {
                    "while"
                } else {
                    "until"
                })
                .soft_break()
                .node(*condition)
                .node(*body)
                .build()
        }
        Node::Break(value) => match value {
            Some(value) => GroupBuilder::new(3, node, ctx, trivia)
                .str("break")
                .soft_break()
                .node(*value)
                .build(),
            None => "break".into(),
        },
        Node::Continue => "continue".into(),
        Node::Return(value) => match value {
            Some(value) => GroupBuilder::new(3, node, ctx, trivia)
                .str("return")
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

fn format_string<'source>(
    string: &AstString,
    node: &AstNode,
    ctx: FormatContext<'source>,
    trivia: &mut TriviaIterator<'source>,
) -> FormatItem<'source> {
    let quote = string.quote.as_char();

    match &string.contents {
        StringContents::Literal(constant) => GroupBuilder::new(3, node, ctx, trivia)
            .char(quote)
            .string_constant(*constant)
            .char(quote)
            .build(),
        StringContents::Raw {
            constant,
            hash_count,
        } => {
            let hashes: KString = "#".repeat(*hash_count as usize).into();

            GroupBuilder::new(5, node, ctx, trivia)
                .char('r')
                .kstring(hashes.clone())
                .char(quote)
                .string_constant(*constant)
                .char(quote)
                .kstring(hashes)
                .build()
        }
        StringContents::Interpolated(nodes) => {
            let mut group = GroupBuilder::new(nodes.len(), node, ctx, trivia).char(quote);
            for node in nodes {
                match node {
                    StringNode::Literal(constant) => group = group.string_constant(*constant),
                    StringNode::Expression { expression, format } => {
                        let format_string = render_format_options(format, ctx.ast.constants());
                        if format_string.is_empty() {
                            group = group.char('{').node(*expression).char('}')
                        } else {
                            group = group
                                .char('{')
                                .node(*expression)
                                .char(':')
                                .kstring(format_string.into())
                                .char('}')
                        }
                    }
                }
            }
            group.char(quote).build()
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
            force_break: OnceCell::new(),
        }
    }

    fn build_block(mut self, indented: bool) -> FormatItem<'source> {
        self.add_trivia(self.group_span.end);
        self.strip_trailing_whitespace();
        FormatItem::Block {
            items: self.items,
            indented,
        }
    }

    fn build_main_block(mut self) -> FormatItem<'source> {
        self.add_trivia(self.group_span.end);
        self.strip_trailing_whitespace();
        FormatItem::MainBlock(self.items)
    }

    fn strip_trailing_whitespace(&mut self) {
        // Remove trailing linebreaks
        while self
            .items
            .last()
            .is_some_and(|item| matches!(item, FormatItem::LineBreak | FormatItem::SoftBreak))
        {
            self.items.pop();
        }
    }

    fn char(mut self, c: char) -> Self {
        self.items.push(c.into());
        self
    }

    fn str(mut self, s: &'source str) -> Self {
        self.items.push(s.into());
        self
    }

    fn string(mut self, s: String) -> Self {
        self.items.push(FormatItem::String(s));
        self
    }

    fn kstring(mut self, s: KString) -> Self {
        self.items.push(FormatItem::KString(s));
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

    fn optional_break(mut self) -> Self {
        self.items.push(FormatItem::OptionalBreak);
        self
    }

    fn line_break(mut self) -> Self {
        // Add any trailing comments for the current line
        self.add_trivia(if self.items.is_empty() {
            self.current_line()
        } else {
            self.next_line()
        });
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
                        // Add a softbreak before the comment if necessary
                        if let Some(last_item) = self.items.last() {
                            if !last_item.is_break() {
                                self.items.push(FormatItem::SoftBreak);
                            }
                        }

                        self.items.push(text.into());
                        self.items.push(FormatItem::ForceBreak);
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

    fn current_line(&self) -> Position {
        Position {
            line: self.current_line,
            column: 0,
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
    // A String
    String(String),
    // A KString
    KString(KString),
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
    // A point where a indented linebreak can be used to break a long expression
    OptionalBreak,
    // Forces a group to be broken onto multiple lines if followed by anything other than
    // an indented block.
    ForceBreak,
    // A grouped sequence of items
    // The group will be rendered on a single line, or if the group doesn't fit in the remaining
    // space then it will be rendered with softbreaks replaced by indented newlines
    Group {
        items: Vec<FormatItem<'source>>,
        // The group's length if it was rendered on a single line
        // This gets calculated and cached during rendering to avoid nested group recalculations
        line_length: OnceCell<usize>,
        // True if the group contains a single-line comment that requires the group to be broken
        // across indented lines.
        force_break: OnceCell<bool>,
    },
    // A sequence of expressions, each on a separate line.
    MainBlock(Vec<FormatItem<'source>>),
    // A sequence of expressions, each on a separate line.
    Block {
        items: Vec<FormatItem<'source>>,
        indented: bool,
    },
}

impl FormatItem<'_> {
    fn render(&self, output: &mut String, options: &FormatOptions, column: usize) {
        match self {
            Self::Char(c) => output.push(*c),
            Self::Str(s) => output.push_str(s),
            Self::String(s) => output.push_str(&s),
            Self::KString(s) => output.push_str(&s),
            Self::SmallInt(n) => output.push_str(&n.to_string()),
            Self::Int(n) => output.push_str(&n.to_string()),
            Self::Float(n) => output.push_str(&n.to_string()),
            Self::LineBreak => output.push('\n'),
            Self::SoftBreak => output.push(' '),
            Self::ForceBreak | Self::OptionalBreak => {} // Forced breaks are handled by group rendering
            Self::Group { items, .. } => {
                let columns_remaining = (options.line_length as usize).saturating_sub(column);
                let too_long = self.line_length() > columns_remaining;

                // Use indent logic if the line is too long, or if the group contains a forced break
                if too_long || self.force_break() {
                    let mut group_column = column;
                    let indented_column = column + options.indent_width as usize;
                    let indent = " ".repeat(indented_column);
                    let mut insert_linebreak = false;
                    for item in items {
                        match item {
                            Self::SoftBreak | Self::OptionalBreak if too_long => {
                                insert_linebreak = true;
                            }
                            Self::ForceBreak => {
                                insert_linebreak = true;
                            }
                            Self::Block { .. } => {
                                // Blocks are already indented, so no additional indentation required
                                item.render(output, options, column);
                                insert_linebreak = false;
                            }
                            _ => {
                                if insert_linebreak {
                                    output.push('\n');
                                    output.push_str(&indent);

                                    insert_linebreak = false;
                                    group_column = indented_column;
                                }

                                item.render(output, options, group_column);
                            }
                        }
                    }
                } else {
                    for item in items {
                        item.render(output, options, column);
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
            Self::Block { items, indented } => {
                let block_column = if *indented {
                    (column + options.indent_width as usize)
                } else {
                    column
                };
                let indent = " ".repeat(block_column);
                let mut last_item_was_linebreak = false;
                for item in items {
                    last_item_was_linebreak = if !matches!(item, FormatItem::LineBreak) {
                        if last_item_was_linebreak {
                            output.push_str(&indent);
                        }
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
            Self::String(s) => s.width(),
            Self::KString(s) => s.width(),
            Self::SmallInt(n) => integer_length(*n as i64),
            Self::Int(i) => integer_length(*i),
            Self::Float(f) => todo!(),
            Self::SoftBreak => 1, // Rendered as a space when used in a line
            Self::Group {
                line_length, items, ..
            } => *line_length.get_or_init(|| items.iter().map(Self::line_length).sum()),
            // Don't bother calculating lengths of items that are broken over lines
            Self::MainBlock(_)
            | Self::Block { .. }
            | Self::LineBreak
            | Self::ForceBreak
            | Self::OptionalBreak => 0,
        }
    }

    fn force_break(&self) -> bool {
        match self {
            Self::LineBreak | Self::ForceBreak => true,
            Self::Group {
                items, force_break, ..
            } => *force_break.get_or_init(|| items.iter().any(Self::force_break)),
            _ => false,
        }
    }

    fn is_break(&self) -> bool {
        match self {
            Self::LineBreak | Self::SoftBreak | Self::ForceBreak | Self::OptionalBreak => true,
            _ => false,
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

fn render_format_options(options: &StringFormatOptions, constants: &ConstantPool) -> String {
    let mut result = String::new();

    if let Some(constant_index) = options.fill_character {
        result.push_str(constants.get_str(constant_index));
    }

    match options.alignment {
        StringAlignment::Default => {}
        StringAlignment::Left => {
            result.push('<');
        }
        StringAlignment::Center => {
            result.push('^');
        }
        StringAlignment::Right => {
            result.push('>');
        }
    }

    if let Some(min_width) = options.min_width {
        result.push_str(&min_width.to_string());
    }
    if let Some(precision) = options.precision {
        result.push_str(&format!(".{precision}"));
    }

    result
}
