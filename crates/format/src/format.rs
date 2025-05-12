#![expect(unused)]

use std::{cell::OnceCell, iter::Peekable, thread::LocalKey};

use crate::{
    FormatOptions, Result, Trivia,
    trivia::{TriviaItem, TriviaIterator, TriviaToken},
};
use koto_lexer::Position;
use koto_parser::{
    Ast, AstCatch, AstFor, AstIf, AstIndex, AstNode, AstString, AstTry, AstUnaryOp, AstVec,
    ChainNode, ConstantIndex, ConstantPool, Function, ImportItem, KString, MatchArm, Node, Span,
    StringAlignment, StringContents, StringFormatOptions, StringNode, StringSlice,
};
use unicode_width::UnicodeWidthStr;

/// Returns the input source formatted according to the provided options
pub fn format(source: &str, options: FormatOptions) -> Result<String> {
    let trivia = Trivia::parse(source)?;
    let ast = koto_parser::Parser::parse(source)?;

    if let Some(entry_point) = ast.entry_point() {
        let context = FormatContext {
            source,
            ast: &ast,
            options: &options,
        };
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
                GroupBuilder::new(4, node, ctx, trivia)
                    .string_constant(*index)
                    .char(':')
                    .space_or_indent()
                    .node(*type_hint)
                    .build()
            } else {
                ctx.string_constant(*index).into()
            }
        }
        Node::Meta(meta_key_id, maybe_name) => {
            if let Some(name) = maybe_name {
                GroupBuilder::new(3, node, ctx, trivia)
                    .str(meta_key_id.as_str())
                    .space_or_indent()
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
                        group = group.maybe_indent().char('.').string_constant(*id);
                    }
                    ChainNode::Str(s) => {
                        group = group
                            .maybe_indent()
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
                                group = group.space_or_indent();
                            }

                            for (i, arg) in args.iter().enumerate() {
                                group = group.node(*arg);

                                if i < args.len() - 1 {
                                    group = group.char(',').space_or_indent();
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
            .maybe_indent()
            .elements(elements)
            .maybe_return()
            .char(']')
            .build(),
        Node::Tuple(elements) => GroupBuilder::new(elements.len() * 2 + 2, node, ctx, trivia)
            .char('(')
            .maybe_indent()
            .elements(elements)
            .maybe_return()
            .char(')')
            .build(),
        Node::TempTuple(elements) => GroupBuilder::new(elements.len() * 2, node, ctx, trivia)
            .maybe_indent()
            .elements(elements)
            .build(),
        Node::Range {
            start,
            end,
            inclusive,
        } => GroupBuilder::new(3, node, ctx, trivia)
            .node(*start)
            .str(if *inclusive { "..=" } else { ".." })
            .node(*end)
            .build(),
        Node::RangeFrom { start } => GroupBuilder::new(2, node, ctx, trivia)
            .node(*start)
            .str("..")
            .build(),
        Node::RangeTo { end, inclusive } => GroupBuilder::new(2, node, ctx, trivia)
            .str(if *inclusive { "..=" } else { ".." })
            .node(*end)
            .build(),
        Node::RangeFull => "..".into(),
        Node::Map { entries, braces } => {
            if *braces {
                let mut group = GroupBuilder::new(entries.len() * 2 + 4, node, ctx, trivia)
                    .char('{')
                    .maybe_indent();

                for (i, entry) in entries.iter().enumerate() {
                    group = group.node(*entry);
                    if i < entries.len() - 1 {
                        group = group.char(',').space_or_indent_if_necessary();
                    }
                }

                group.maybe_return().char('}').build()
            } else {
                let mut group =
                    GroupBuilder::new(entries.len() * 2 + 1, node, ctx, trivia).indented_break();

                for entry in entries.iter() {
                    group = group.node(*entry).indented_break();
                }

                group.build()
            }
        }
        Node::MapEntry(key, value) => GroupBuilder::new(4, node, ctx, trivia)
            .node(*key)
            .char(':')
            .space_or_indent()
            .node(*value)
            .build(),
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
        Node::Function(Function {
            args,
            body,
            is_variadic,
            output_type,
            ..
        }) => {
            let mut group = GroupBuilder::new(3, node, ctx, trivia);

            // Args
            group = group.nested(|trivia| {
                let mut group = GroupBuilder::new(3 + args.len() * 2, node, ctx, trivia);

                group = group.char('|').maybe_indent();
                for (i, arg) in args.iter().enumerate() {
                    group = group.node(*arg);
                    if i < args.len() - 1 {
                        group = group.char(',').space_or_indent_if_necessary();
                    } else if *is_variadic {
                        group = group.str("...");
                    }
                }
                group = group.maybe_return().char('|');

                // Output type
                if let Some(output_type) = output_type {
                    group = group
                        .space_or_indent_if_necessary()
                        .str("->")
                        .space_or_indent_if_necessary()
                        .node(*output_type);
                }

                group.build()
            });

            // Body
            group.space_or_indent().node(*body).build()
        }
        Node::Import { from, items } => {
            let mut group =
                GroupBuilder::new(5 + from.len() * 2 - 1 + items.len() * 2, node, ctx, trivia)
                    .str("from")
                    .space_or_indent();

            for (i, from_node) in from.iter().enumerate() {
                group = group.node(*from_node);
                if i < from.len() - 1 {
                    group = group.char('.');
                }
            }

            group = group.space_or_return().str("import").space_or_indent();

            for (i, ImportItem { item, name }) in items.iter().enumerate() {
                group = group.nested(|trivia| {
                    let mut group = GroupBuilder::new(0, node, ctx, trivia).node(*item);
                    if let Some(name) = name {
                        group = group.str(" as ").node(*name);
                    }

                    if i < items.len() - 1 {
                        group = group.char(',');
                    }

                    group.build()
                });

                if i < items.len() - 1 {
                    group = group.space_or_indent_if_necessary();
                }
            }

            group.build()
        }
        Node::Export(value) => {
            FormatItem::from_keyword_and_value("export", value, node, ctx, trivia)
        }
        Node::Assign { target, expression } => GroupBuilder::new(3, node, ctx, trivia)
            .node(*target)
            .space_or_indent_if_necessary()
            .nested(|trivia| {
                GroupBuilder::new(3, node, ctx, trivia)
                    .char('=')
                    .space_or_indent_if_necessary()
                    .node(*expression)
                    .build()
            })
            .build(),
        Node::MultiAssign {
            targets,
            expression,
        } => {
            let mut group = GroupBuilder::new(targets.len() * 3, node, ctx, trivia);

            for (i, target) in targets.iter().enumerate() {
                group = group.node(*target);
                if i < targets.len() - 1 {
                    group = group.char(',');
                }

                group = group.space_or_indent_if_necessary();
            }

            group
                .nested(|trivia| {
                    GroupBuilder::new(3, node, ctx, trivia)
                        .char('=')
                        .space_or_indent_if_necessary()
                        .node(*expression)
                        .build()
                })
                .build()
        }
        Node::UnaryOp { op, value } => match op {
            AstUnaryOp::Negate => GroupBuilder::new(2, node, ctx, trivia)
                .str(op.as_str())
                .node(*value)
                .build(),
            AstUnaryOp::Not => GroupBuilder::new(3, node, ctx, trivia)
                .str(op.as_str())
                .space_or_indent()
                .node(*value)
                .build(),
        },
        Node::BinaryOp { op, lhs, rhs } => GroupBuilder::new(3, node, ctx, trivia)
            .node_flattened(*lhs)
            .space_or_indent()
            .nested(|trivia| {
                GroupBuilder::new(3, node, ctx, trivia)
                    .str(op.as_str())
                    .space_or_indent()
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
                    .space_or_indent()
                    .node(*condition)
                    .node(*then_node)
                    .build()
            });

            for (else_if_condition, else_if_block) in else_if_blocks {
                group = group.line_break().nested(|trivia| {
                    GroupBuilder::new(3, node, ctx, trivia)
                        .str("else if")
                        .space_or_indent()
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
        Node::Match { expression, arms } => {
            let mut group = GroupBuilder::new(3 + arms.len() * 2, node, ctx, trivia)
                .nested(|trivia| {
                    GroupBuilder::new(2, node, ctx, trivia)
                        .str("match ")
                        .node(*expression)
                        .build()
                })
                .add_trailing_trivia();

            for (i, arm) in arms.iter().enumerate() {
                group = group.indented_break().nested(|trivia| {
                    let mut group =
                        GroupBuilder::new(arm.patterns.len() * 2 + 4, node, ctx, trivia);

                    if arm.is_else() {
                        group = group.str("else");
                    } else {
                        for (i, pattern) in arm.patterns.iter().enumerate() {
                            group = group.node(*pattern);
                            if i < arm.patterns.len() - 1 {
                                group = group
                                    .space_or_indent_if_necessary()
                                    .str("or")
                                    .space_or_indent_if_necessary();
                            }
                        }
                        if let Some(condition) = arm.condition {
                            group = group
                                .space_or_indent_if_necessary()
                                .str("if")
                                .space_or_indent_if_necessary()
                                .node(condition);
                        }
                        group = group.space_or_indent_if_necessary().str("then");
                    }

                    if ctx.options.match_and_switch_always_indent_arm_bodies {
                        group = group.indented_break();
                    } else {
                        group = group.space_or_indent();
                    }

                    group.node(arm.expression).add_trailing_trivia().build()
                });
            }

            group.build()
        }
        Node::Switch(arms) => {
            let mut group = GroupBuilder::new(2 + arms.len() * 2, node, ctx, trivia)
                .str("switch")
                .indented_break();

            for (i, arm) in arms.iter().enumerate() {
                group = group.nested(|trivia| {
                    let mut group = GroupBuilder::new(3, node, ctx, trivia);
                    group = if let Some(condition) = arm.condition {
                        group.node(condition).str(" then")
                    } else {
                        group.str("else")
                    };

                    if ctx.options.match_and_switch_always_indent_arm_bodies {
                        group = group.indented_break();
                    } else {
                        group = group.space_or_indent();
                    }

                    group.node(arm.expression).add_trailing_trivia().build()
                });

                if i < arms.len() - 1 {
                    group = group.indented_break();
                }
            }

            group.build()
        }
        Node::Wildcard(id, type_hint) => {
            let mut group = GroupBuilder::new(1, node, ctx, trivia).char('_');
            if let Some(id) = id {
                group = group.string_constant(*id);
            }
            if let Some(type_hint) = type_hint {
                group = group.char(':').space_or_indent().node(*type_hint);
            }
            group.build()
        }
        Node::PackedId(id) => {
            if let Some(id) = id {
                GroupBuilder::new(2, node, ctx, trivia)
                    .string_constant(*id)
                    .str("...")
                    .build()
            } else {
                "...".into()
            }
        }
        Node::PackedExpression(expression) => GroupBuilder::new(2, node, ctx, trivia)
            .node(*expression)
            .str("...")
            .build(),
        Node::For(AstFor {
            args,
            iterable,
            body,
        }) => {
            let mut group = GroupBuilder::new((args.len() * 3 - 1) + 6, node, ctx, trivia)
                .str("for")
                .space_or_indent();
            for (i, arg) in args.iter().enumerate() {
                group = group.node(*arg);
                if i < args.len() - 1 {
                    group = group.char(',');
                }
                group = group.space_or_indent()
            }
            group
                .str("in")
                .space_or_indent()
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
                .space_or_indent()
                .node(*condition)
                .node(*body)
                .build()
        }
        Node::Break(value) => match value {
            Some(value) => FormatItem::from_keyword_and_value("break", value, node, ctx, trivia),
            None => "break".into(),
        },
        Node::Continue => "continue".into(),
        Node::Return(value) => match value {
            Some(value) => FormatItem::from_keyword_and_value("return", value, node, ctx, trivia),
            None => "return".into(),
        },
        Node::Try(AstTry {
            try_block,
            catch_blocks,
            finally_block,
        }) => {
            let mut group = GroupBuilder::new(2 + 2 * catch_blocks.len() + 2, node, ctx, trivia)
                .nested(|trivia| {
                    GroupBuilder::new(3, node, ctx, trivia)
                        .str("try")
                        .indented_break()
                        .node(*try_block)
                        .build()
                });

            for AstCatch { arg, block } in catch_blocks.iter() {
                group = group.line_break().nested(|trivia| {
                    GroupBuilder::new(3, node, ctx, trivia)
                        .str("catch ")
                        .node(*arg)
                        .indented_break()
                        .node(*block)
                        .build()
                })
            }

            if let Some(finally) = finally_block {
                group = group.line_break().nested(|trivia| {
                    GroupBuilder::new(2, node, ctx, trivia)
                        .str("finally")
                        .indented_break()
                        .node(*finally)
                        .build()
                })
            }

            group.build_block(false)
        }
        Node::Throw(value) => FormatItem::from_keyword_and_value("throw", value, node, ctx, trivia),
        Node::Yield(value) => FormatItem::from_keyword_and_value("yield", value, node, ctx, trivia),
        Node::Debug { expression, .. } => {
            FormatItem::from_keyword_and_value("debug", expression, node, ctx, trivia)
        }
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
    options: &'source FormatOptions,
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
            .is_some_and(|item| matches!(item, FormatItem::LineBreak | FormatItem::SpaceOrIndent))
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

    fn space_or_indent(mut self) -> Self {
        self.items.push(FormatItem::SpaceOrIndent);
        self
    }

    fn space_or_return(mut self) -> Self {
        self.items.push(FormatItem::SpaceOrReturn);
        self
    }

    fn space_or_indent_if_necessary(mut self) -> Self {
        self.items.push(FormatItem::SpaceOrIndentIfNecessary);
        self
    }

    fn maybe_indent(mut self) -> Self {
        self.items.push(FormatItem::MaybeIndent);
        self
    }

    fn maybe_return(mut self) -> Self {
        self.items.push(FormatItem::MaybeReturn);
        self
    }

    fn indented_break(mut self) -> Self {
        // Add any trailing comments for the current line
        self = self.add_trailing_trivia();
        self.items.push(FormatItem::ForceBreak);
        self
    }

    fn line_break(mut self) -> Self {
        self = self.add_trailing_trivia();
        self.items.push(FormatItem::LineBreak);
        self
    }

    fn add_trailing_trivia(mut self) -> Self {
        // Add any trailing comments for the current line
        self.add_trivia(if self.items.is_empty() {
            self.current_line()
        } else {
            self.next_line()
        });
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
                self = self.char(',').space_or_indent_if_necessary();
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
                                self.items.push(FormatItem::SpaceOrIndentIfNecessary);
                            }
                        }

                        self.items.push(text.into());
                        self.items.push(FormatItem::ForceBreak);
                    }
                    TriviaToken::CommentMulti(text) => {
                        self.items.push(text.into());
                        self.items.push(FormatItem::SpaceOrIndentIfNecessary);
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
    // A linebreak
    LineBreak,
    // A space or indented linebreak, always breaking when the line is too long
    SpaceOrIndent,
    // A space or indented linebreak, only breaking if necessary
    SpaceOrIndentIfNecessary,
    // A space, or a point where a long line can be broken with a return to the start column
    SpaceOrReturn,
    // A point where a long line can be broken with an indent
    MaybeIndent,
    // A point where a long line can be broken with a return to the start column
    MaybeReturn,
    // Forces a group to be broken onto multiple lines if followed by anything other than
    // an indented block.
    ForceBreak,
}

impl<'source> FormatItem<'source> {
    // Used for keyword/value expressions like `return true`
    fn from_keyword_and_value<'trivia>(
        keyword: &'source str,
        value: &AstIndex,
        group_node: &AstNode,
        ctx: FormatContext<'source>,
        trivia: &'trivia mut TriviaIterator<'source>,
    ) -> Self {
        GroupBuilder::new(3, group_node, ctx, trivia)
            .str(keyword)
            .space_or_indent()
            .node(*value)
            .build()
    }

    // Renders the format item, appending to the provided output string
    fn render(&self, output: &mut String, options: &FormatOptions, column: usize) {
        match self {
            Self::Char(c) => output.push(*c),
            Self::Str(s) => output.push_str(s),
            Self::String(s) => output.push_str(s),
            Self::KString(s) => output.push_str(s),
            Self::SmallInt(n) => output.push_str(&n.to_string()),
            Self::Int(n) => output.push_str(&n.to_string()),
            Self::Float(n) => output.push_str(&n.to_string()),
            Self::Group { items, .. } => self.render_group(items, output, options, column),
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
            Self::LineBreak => output.push('\n'),
            // Optional breaks are rendered as a space when the group fits within the remaining width
            Self::SpaceOrIndent | Self::SpaceOrIndentIfNecessary | Self::SpaceOrReturn => {
                output.push(' ')
            }
            // Forced or optional breaks are handled by group rendering
            Self::ForceBreak | Self::MaybeIndent | Self::MaybeReturn => {}
        }
    }

    fn render_group(
        &self,
        items: &[FormatItem<'source>],
        output: &mut String,
        options: &FormatOptions,
        column: usize,
    ) {
        let columns_remaining = (options.line_length as usize).saturating_sub(column);
        let too_long = self.line_length() > columns_remaining;

        // Use indent logic if the line is too long, or if the group contains a forced break
        if too_long || self.force_break() {
            let group_start_indent = " ".repeat(column);
            let extra_indent = " ".repeat(options.indent_width as usize);
            let mut group_column = column;
            let mut group_break = GroupBreak::None;
            let mut current_line_width = 0;
            let mut item_buffer = String::new();
            let mut line_width = group_column;

            for item in items {
                match item {
                    Self::SpaceOrIndent | Self::MaybeIndent => {
                        if too_long {
                            group_break = GroupBreak::IndentedBreak;
                        } else {
                            item.render(&mut item_buffer, options, group_column);
                        }
                    }
                    Self::SpaceOrIndentIfNecessary if too_long => {
                        group_break = GroupBreak::IndentedBreakIfNecessary;
                    }
                    Self::SpaceOrReturn | Self::MaybeReturn if too_long => {
                        group_break = GroupBreak::ReturnBreak;
                    }
                    Self::ForceBreak => {
                        group_break = GroupBreak::IndentedBreak;
                    }
                    Self::LineBreak => {
                        output.push('\n');
                        group_break = GroupBreak::IndentedBreak;
                    }
                    Self::Block { .. } => {
                        // A block can only be the last item in a group and renders its own linebreak
                        item.render(output, options, column);
                        return;
                    }
                    _ => {
                        // Adjust the column for the item to be rendered
                        if group_break.needs_linebreak() {
                            group_column = column;
                            if group_break.needs_indent() {
                                group_column += extra_indent.len();
                            }
                        }

                        // Render the item into a temporary buffer
                        item.render(&mut item_buffer, options, group_column);

                        // Get the width of the item's first line
                        // (multiline items are possible, and we only need the first line's width
                        // to decide if a linebreak is necessary).
                        let mut item_first_line_width = item_buffer
                            .split_once('\n')
                            .map(|(first, _rest)| first)
                            .unwrap_or(&item_buffer)
                            .width();

                        // Check for 'indented break if necessary' items
                        if matches!(group_break, GroupBreak::IndentedBreakIfNecessary) {
                            // +1 for a space
                            let line_width_with_item = line_width + item_first_line_width + 1;
                            if line_width_with_item > options.line_length as usize {
                                group_break = GroupBreak::IndentedBreak;
                            } else if item_first_line_width > 0 {
                                output.push(' ');
                                item_first_line_width += 1;
                            }
                        }

                        // Emit linebreaks if necessary
                        if group_break.needs_linebreak() {
                            output.push('\n');
                            output.push_str(&group_start_indent);
                            group_column = column;
                            line_width = group_start_indent.len();
                            if group_break.needs_indent() {
                                group_column += extra_indent.len();
                                output.push_str(&extra_indent);
                                line_width = group_column;
                            }
                        }
                        group_break = GroupBreak::None;

                        // Add the item to the output
                        let item_last_line_width = item_buffer
                            .rsplit_once('\n')
                            .map(|(_rest, last)| last.width())
                            .unwrap_or(item_first_line_width);
                        output.extend(item_buffer.drain(..));
                        line_width += item_last_line_width;
                    }
                }
            }
        } else {
            for item in items {
                item.render(output, options, column);
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
            Self::Group {
                line_length, items, ..
            } => *line_length.get_or_init(|| items.iter().map(Self::line_length).sum()),
            // Rendered in single lines as space
            Self::SpaceOrIndent | Self::SpaceOrIndentIfNecessary => 1,
            // Optional linebreaks that don't take up space in a single line
            Self::SpaceOrReturn | Self::MaybeIndent | Self::MaybeReturn => 0,
            // Don't bother calculating lengths of items that are broken over lines
            Self::MainBlock(_) | Self::Block { .. } | Self::LineBreak | Self::ForceBreak => 0,
        }
    }

    fn force_break(&self) -> bool {
        match self {
            Self::LineBreak | Self::ForceBreak | Self::Block { .. } => true,
            Self::Group {
                items, force_break, ..
            } => *force_break.get_or_init(|| items.iter().any(Self::force_break)),
            _ => false,
        }
    }

    fn is_break(&self) -> bool {
        matches!(
            self,
            Self::LineBreak | Self::SpaceOrIndent | Self::ForceBreak | Self::MaybeIndent
        )
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

enum GroupBreak {
    None,
    IndentedBreak,
    ReturnBreak,
    IndentedBreakIfNecessary,
}

impl GroupBreak {
    fn needs_linebreak(&self) -> bool {
        matches!(self, Self::IndentedBreak | Self::ReturnBreak)
    }

    fn needs_indent(&self) -> bool {
        matches!(self, Self::IndentedBreak)
    }
}
