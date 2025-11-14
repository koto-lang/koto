use crate::{
    Error, ErrorKind, FormatOptions, Result, Trivia,
    trivia::{TriviaItem, TriviaIterator, TriviaToken},
};
use koto_lexer::Position;
use koto_parser::{
    Ast, AstCatch, AstFor, AstIf, AstIndex, AstNode, AstString, AstTry, AstUnaryOp, ChainNode,
    ConstantIndex, ConstantPool, Function, ImportItem, KString, Node, ParserOptions, Span,
    StringAlignment, StringContents, StringFormatOptions, StringNode,
};
use std::{cell::OnceCell, iter};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Returns the input source formatted according to the provided options
pub fn format(source: &str, options: FormatOptions) -> Result<String> {
    let trivia = Trivia::parse(source)?;
    let ast = koto_parser::Parser::parse_with_options(
        source,
        ParserOptions {
            process_escape_codes: false,
        },
    )?;

    if let Some(entry_point) = ast.entry_point() {
        let context = FormatContext::new(source, &ast, &options);
        let output = format_node(entry_point, &context, &mut trivia.iter());

        let mut result = String::new();
        output.render(&mut result, false, false, &options, 0)?;

        if !result.ends_with('\n') {
            result.push('\n');
        }

        Ok(result)
    } else {
        Ok(String::new())
    }
}

fn format_node<'source>(
    node_index: AstIndex,
    ctx: &'source FormatContext<'source>,
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
            let force_break = should_chain_be_broken(root_node, next, ctx);
            let chain_start_line = ctx.span(node).start.line;

            let mut group = GroupBuilder::new(2, node, ctx, trivia);
            let mut node = node;
            let mut chain_node = root_node;
            let mut chain_next = next;
            let mut chain_index = node_index;

            let mut first_id = true;

            loop {
                match chain_node {
                    ChainNode::Root(root) => {
                        group = group.sub_group_start().node(*root);
                    }
                    ChainNode::Id(id) => {
                        // The first id access can be allowed to stay on the start line when force
                        // breaking.
                        group = group.sub_group_end();
                        if force_break
                            && (!first_id || ctx.span(node).start.line > chain_start_line)
                        {
                            group = group
                                .add_trailing_trivia()
                                .indented_break()
                                .add_preceding_trivia(chain_index);
                        } else if first_id {
                            group = group.indent_if_necessary();
                        } else {
                            group = group.maybe_indent();
                        }
                        group = group.sub_group_start().char('.').string_constant(*id);
                        first_id = false;
                    }
                    ChainNode::Str(s) => {
                        group = group.maybe_force_indent(force_break).char('.').nested(
                            3,
                            node,
                            |nested| format_string(s, nested),
                        );
                    }
                    ChainNode::Index(index) => {
                        group = group.nested(3, node, |nested| {
                            nested.char('[').node(*index).char(']').build()
                        })
                    }
                    ChainNode::Call { args, with_parens } => {
                        group = group.nested(args.len() * 3, node, |mut nested| {
                            let force_break_args = match args.as_slice() {
                                &[first, .., last] => {
                                    ctx.span(ctx.node(first)).end.line
                                        < ctx.span(ctx.node(last)).start.line
                                }
                                _ => false,
                            };

                            if *with_parens {
                                nested = nested.char('(');
                                if force_break_args {
                                    nested = nested.indented_break();
                                }
                            } else if force_break_args {
                                nested = nested.indented_break();
                            } else {
                                nested = nested.space_or_indent();
                            }

                            for (i, arg) in args.iter().enumerate() {
                                nested = nested.node(*arg);

                                if i < args.len() - 1 {
                                    nested = nested.char(',');
                                    if force_break_args {
                                        nested = nested.indented_break();
                                    } else {
                                        nested = nested.space_or_indent();
                                    }
                                }
                            }

                            if *with_parens {
                                if force_break_args {
                                    nested = nested.maybe_return();
                                }
                                nested = nested.char(')');
                            }
                            nested.build()
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
                            chain_index = *next;
                            chain_node = next_chain_node;
                            chain_next = next_next;
                        }
                        other => {
                            return Error::new(
                                ErrorKind::UnexpectedNode {
                                    expected: "ChainNode".into(),
                                    unexpected: other.clone(),
                                },
                                *ctx.span(node),
                            )
                            .into();
                        }
                    }
                } else {
                    break;
                }
            }

            group.build()
        }
        Node::BoolTrue => "true".into(),
        Node::BoolFalse => "false".into(),
        Node::SmallInt(_) | Node::Int(_) | Node::Float(_) => {
            // Take the number's representation directly from the source
            FormatItem::Str(ctx.source_slice(ctx.span(node)))
        }
        Node::Str(s) => format_string(s, GroupBuilder::new(3, node, ctx, trivia)),
        Node::List(elements) => GroupBuilder::new(elements.len() * 2 + 2, node, ctx, trivia)
            .char('[')
            .maybe_indent()
            .list_elements(elements)
            .maybe_return()
            .char(']')
            .build(),
        Node::Tuple {
            elements,
            parentheses,
        } => {
            if *parentheses {
                GroupBuilder::new(5, node, ctx, trivia)
                    .char('(')
                    .maybe_indent()
                    .nested(elements.len() * 3, node, |nested| {
                        nested.tuple_elements(elements).build()
                    })
                    .maybe_return()
                    .char(')')
                    .build()
            } else {
                GroupBuilder::new(elements.len() * 3 + 2, node, ctx, trivia)
                    .maybe_indent()
                    .tuple_elements(elements)
                    .maybe_return()
                    .build()
            }
        }
        Node::TempTuple(elements) => GroupBuilder::new(elements.len() * 3, node, ctx, trivia)
            .maybe_indent()
            .tuple_elements(elements)
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
                let span = ctx.span(node);
                let force_break = span.start.line < span.end.line;

                let mut group = GroupBuilder::new(entries.len() * 2 + 4, node, ctx, trivia)
                    .char('{')
                    .maybe_force_indent(force_break);

                let mut previous_line = ctx.span(node).start.line;
                for (i, entry) in entries.iter().enumerate() {
                    let entry_start_line = ctx.span(ctx.node(*entry)).start.line;

                    // Space or indent following the previous entry?
                    if i > 0 {
                        if entry_start_line > previous_line {
                            group = group.indented_break();
                        } else {
                            group = group.space_or_indent_if_necessary();
                        }
                    }
                    previous_line = entry_start_line;

                    group = group.node(*entry);
                    if i < entries.len() - 1 {
                        group = group.char(',');
                    } else {
                        group = group.maybe_char(',');
                    }
                }

                group.maybe_return().char('}').build()
            } else {
                let mut group =
                    GroupBuilder::new(entries.len() * 4 + 1, node, ctx, trivia).start_block();

                for entry in entries.iter() {
                    // Use the entry's key as the line start node to collect the entry's
                    // leading trivia before rendering the entry as a sub-group.
                    let key = match &ctx.node(*entry).node {
                        Node::MapEntry(key, _) => key,
                        other => {
                            return Error::new(
                                ErrorKind::UnexpectedNode {
                                    expected: "MapEntry".into(),
                                    unexpected: other.clone(),
                                },
                                *ctx.span(node),
                            )
                            .into();
                        }
                    };
                    group = group.line_start(*key).node(*entry).line_break();
                }

                group.build_block()
            }
        }
        Node::MapEntry(key, value) => GroupBuilder::new(4, node, ctx, trivia)
            .node(*key)
            .char(':')
            .space_or_indent()
            .node(*value)
            .build(),
        Node::MapPattern { entries, type_hint } => {
            let span = ctx.span(node);
            let force_break = span.start.line < span.end.line;
            let type_hint_capacity = if type_hint.is_some() { 3 } else { 0 };

            let mut group = GroupBuilder::new(
                entries.len() * 3 + 4 + type_hint_capacity,
                node,
                ctx,
                trivia,
            )
            .char('{')
            .maybe_force_indent(force_break);

            let mut previous_line = ctx.span(node).start.line;
            for (i, entry) in entries.iter().enumerate() {
                let entry_start_line = ctx.span(ctx.node(*entry)).start.line;

                // Space or indent following the previous entry?
                if i > 0 {
                    if entry_start_line > previous_line {
                        group = group.indented_break();
                    } else {
                        group = group.space_or_indent_if_necessary();
                    }
                }
                previous_line = entry_start_line;

                group = group.node(*entry);
                if i < entries.len() - 1 {
                    group = group.char(',');
                } else {
                    group = group.maybe_char(',');
                }
            }

            group = group.maybe_return().char('}');

            if let Some(type_hint) = type_hint {
                group = group.char(':').space_or_indent().node(*type_hint);
            }

            group.build()
        }
        Node::MapKeyRebind { key, id_or_ignored } => GroupBuilder::new(5, node, ctx, trivia)
            .node(*key)
            .space_or_indent()
            .str("as")
            .space_or_indent()
            .node(*id_or_ignored)
            .build(),
        Node::Self_ => "self".into(),
        Node::MainBlock { body, .. } => {
            let mut group = GroupBuilder::new(body.len() * 3, node, ctx, trivia);
            for block_node in body {
                group = group
                    .line_start(*block_node)
                    .node(*block_node)
                    .add_trailing_trivia()
                    .line_break()
            }

            group.build_main_block()
        }
        Node::Block(body) => match body.as_slice() {
            [single] if matches!(ctx.node(*single).node, Node::Map { braces: false, .. }) => {
                format_node(*single, ctx, trivia)
            }
            _ => {
                let mut group = GroupBuilder::new(body.len() * 3, node, ctx, trivia).start_block();
                for block_node in body {
                    group = group
                        .line_start(*block_node)
                        .node(*block_node)
                        .add_trailing_trivia()
                        .line_break()
                }
                group.build_block()
            }
        },
        Node::Function(Function { args, body, .. }) => {
            if matches!(ctx.node(*body).node, Node::Block(..)) {
                GroupBuilder::new(3, node, ctx, trivia)
                    .node(*args)
                    .node(*body)
                    .build()
            } else {
                GroupBuilder::new(3, node, ctx, trivia)
                    .node(*args)
                    .space_or_indent()
                    .node(*body)
                    .build()
            }
        }
        Node::FunctionArgs {
            args,
            variadic,
            output_type,
        } => {
            let mut group = GroupBuilder::new(4 + args.len() * 2, node, ctx, trivia)
                .char('|')
                .maybe_indent();
            for (i, arg) in args.iter().enumerate() {
                group = group.node(*arg);
                if i < args.len() - 1 {
                    group = group.char(',').space_or_indent_if_necessary();
                } else if *variadic {
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
        }
        Node::Import { from, items } => {
            let mut group =
                GroupBuilder::new(5 + from.len() * 2 - 1 + items.len() * 2, node, ctx, trivia);

            if !from.is_empty() {
                group = group.str("from").space_or_indent();

                for (i, from_node) in from.iter().enumerate() {
                    group = group.node(*from_node);
                    if i < from.len() - 1 {
                        group = group.char('.');
                    }
                }

                group = group.space_or_return();
            }

            group = group.str("import").space_or_indent();

            for (i, ImportItem { item, name }) in items.iter().enumerate() {
                group = group.nested(0, node, |mut nested| {
                    nested = nested.node(*item);
                    if let Some(name) = name {
                        nested = nested.str(" as ").node(*name);
                    }

                    if i < items.len() - 1 {
                        nested = nested.char(',');
                    }

                    nested.build()
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
        Node::Assign {
            target,
            expression,
            let_assignment,
        } => {
            let mut group = GroupBuilder::new(5, node, ctx, trivia);

            if *let_assignment {
                group = group.str("let ");
            }

            group
                .node(*target)
                .space_or_indent_if_necessary()
                .char('=')
                .space_or_indent_respecting_existing_break(target, expression)
                .node(*expression)
                .build()
        }
        Node::MultiAssign {
            targets,
            expression,
            let_assignment,
        } => {
            if targets.is_empty() {
                return Error::new(ErrorKind::MissingMultiAssignTargets, *ctx.span(node)).into();
            }

            let mut group = GroupBuilder::new(targets.len() * 3 + 3, node, ctx, trivia);

            if *let_assignment {
                group = group.str("let ");
            }

            for (i, target) in targets.iter().enumerate() {
                group = group.node(*target);
                if i < targets.len() - 1 {
                    group = group.char(',');
                }

                group = group.space_or_indent_if_necessary();
            }

            group
                .char('=')
                .space_or_indent_respecting_existing_break(
                    targets.last().unwrap(), // `targets.is_empty` was checked above
                    expression,
                )
                .node(*expression)
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
        Node::BinaryOp { op, lhs, rhs } => {
            let lhs_span = ctx.span(ctx.node(*lhs));
            let rhs_span = ctx.span(ctx.node(*rhs));
            let chained_op = matches!(ctx.node(*lhs).node, Node::BinaryOp { .. });
            if lhs_span.end.line == rhs_span.start.line {
                GroupBuilder::new(3, node, ctx, trivia)
                    .node_maybe_flattened(*lhs, chained_op)
                    .space_or_indent_if_necessary()
                    .nested(3, node, |nested| {
                        nested
                            .str(op.as_str())
                            .space_or_indent_if_necessary()
                            .node(*rhs)
                            .build()
                    })
                    .build()
            } else {
                // An explicit break is in the op, so insert an indented break
                GroupBuilder::new(3, node, ctx, trivia)
                    .node_maybe_flattened(*lhs, chained_op)
                    .add_trailing_trivia()
                    .return_or_indent()
                    .nested(3, node, |nested| {
                        nested
                            .str(op.as_str())
                            .space_or_indent_if_necessary()
                            .node(*rhs)
                            .build()
                    })
                    .build()
            }
        }
        Node::If(AstIf {
            condition,
            then_node,
            else_if_blocks,
            else_node,
            inline,
        }) => {
            if *inline {
                let mut group = GroupBuilder::new(4, node, ctx, trivia)
                    .str("if ")
                    .node(*condition)
                    .str(" then ")
                    .node(*then_node);

                if let Some(else_block) = else_node {
                    group = group.str(" else ").node(*else_block);
                }

                group.build()
            } else {
                let mut group = GroupBuilder::new(3, node, ctx, trivia).nested(4, node, |nested| {
                    nested.str("if ").node(*condition).node(*then_node).build()
                });

                for (else_if_condition, else_if_block) in else_if_blocks {
                    let else_if_node = ctx.node(*else_if_block);
                    group = group.line_break().line_start(*else_if_condition).nested(
                        4,
                        else_if_node,
                        |nested| {
                            nested
                                .str("else if ")
                                .node(*else_if_condition)
                                .node(*else_if_block)
                                .build()
                        },
                    );
                }

                if let Some(else_block) = else_node {
                    let else_node = ctx.node(*else_block);
                    group =
                        group
                            .line_break()
                            .line_start(*condition)
                            .nested(2, else_node, |nested| {
                                nested.str("else").node(*else_block).build()
                            });
                }

                group.build_block()
            }
        }
        Node::Match { expression, arms } => GroupBuilder::new(2, node, ctx, trivia)
            .nested(2, node, |nested| {
                nested.str("match ").node(*expression).build()
            })
            .add_trailing_trivia()
            .nested(arms.len() * 2, node, |mut nested| {
                nested = nested.start_block();

                for arm in arms.iter() {
                    nested = nested.line_start(*arm).node(*arm).line_break();
                }

                nested.build_block()
            })
            .build(),
        Node::MatchArm {
            patterns,
            condition,
            expression,
        } => {
            let mut group = GroupBuilder::new(patterns.len() * 2 + 4, node, ctx, trivia);

            if patterns.is_empty() {
                group = group.str("else");
            } else {
                for (i, pattern) in patterns.iter().enumerate() {
                    group = group.node(*pattern);
                    if i < patterns.len() - 1 {
                        group = group
                            .space_or_indent_if_necessary()
                            .str("or")
                            .space_or_indent_if_necessary();
                    }
                }
                if let Some(condition) = condition {
                    group = group
                        .space_or_indent_if_necessary()
                        .str("if")
                        .space_or_indent_if_necessary()
                        .node(*condition);
                }
                group = group.space_or_indent_if_necessary().str("then");
            }

            if ctx.options.always_indent_arms {
                // If we're force breaking and the body is on the same line as the arm,
                // then allow trivia to move with the body.
                if ctx.span(ctx.node(*expression)).start.line == ctx.span(node).start.line {
                    group = group.indented_break_without_trivia();
                } else {
                    group = group.indented_break();
                }
            } else {
                group = group.space_or_indent();
            }

            group.node(*expression).add_trailing_trivia().build()
        }
        Node::Switch(arms) => GroupBuilder::new(2, node, ctx, trivia)
            .str("switch")
            .nested(arms.len() * 2, node, |mut nested| {
                nested = nested.start_block();

                for (i, arm) in arms.iter().enumerate() {
                    nested = nested.line_start(*arm).node(*arm);

                    if i < arms.len() - 1 {
                        nested = nested.line_break();
                    }
                }

                nested.build_block()
            })
            .build(),
        Node::SwitchArm {
            condition,
            expression,
        } => {
            let mut group = GroupBuilder::new(4, node, ctx, trivia);

            group = if let Some(condition) = condition {
                group.node(*condition).str(" then")
            } else {
                group.str("else")
            };

            if ctx.options.always_indent_arms {
                let expression_span = ctx.span(ctx.node(*expression));
                let arm_span = ctx.span(node);
                // If we're force breaking and the body is on the same line as the arm,
                // then allow trivia to move with the body.
                if expression_span.start.line == arm_span.start.line {
                    group = group.indented_break_without_trivia();
                } else {
                    group = group.indented_break();
                }
            } else {
                group = group.space_or_indent();
            }

            group.node(*expression).add_trailing_trivia().build()
        }
        Node::Ignored(id, type_hint) => {
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
            let mut group =
                GroupBuilder::new((args.len() * 3 - 1) + 6, node, ctx, trivia).str("for ");
            for (i, arg) in args.iter().enumerate() {
                group = group.node(*arg);
                if i < args.len() - 1 {
                    group = group.char(',').space_or_indent();
                }
            }
            group.str(" in ").node(*iterable).node(*body).build()
        }
        Node::Loop { body } => GroupBuilder::new(2, node, ctx, trivia)
            .str("loop")
            .node(*body)
            .build(),
        Node::While { condition, body } | Node::Until { condition, body } => {
            GroupBuilder::new(4, node, ctx, trivia)
                .str(if matches!(&node.node, Node::While { .. }) {
                    "while "
                } else {
                    "until "
                })
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
                .nested(2, ctx.node(*try_block), |nested| {
                    nested.str("try").node(*try_block).build()
                });

            for AstCatch { arg, block } in catch_blocks.iter() {
                group = group.line_break().nested(4, ctx.node(*block), |nested| {
                    nested
                        .line_start(*arg)
                        .str("catch ")
                        .node(*arg)
                        .node(*block)
                        .build()
                })
            }

            if let Some(finally) = finally_block {
                group = group.line_break().nested(3, ctx.node(*finally), |nested| {
                    nested
                        .line_start(*finally)
                        .str("finally")
                        .node(*finally)
                        .build()
                })
            }

            group.build_block()
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
    group: GroupBuilder<'source, '_>,
) -> FormatItem<'source> {
    let quote = string.quote.as_char();

    match &string.contents {
        StringContents::Literal(constant) => group
            .char(quote)
            .string_constant(*constant)
            .char(quote)
            .build(),
        StringContents::Raw {
            constant,
            hash_count,
        } => {
            let hashes: KString = "#".repeat(*hash_count as usize).into();

            group
                .char('r')
                .kstring(hashes.clone())
                .char(quote)
                .string_constant(*constant)
                .char(quote)
                .kstring(hashes)
                .build()
        }
        StringContents::Interpolated(nodes) => {
            let mut group = group.char(quote);
            for node in nodes {
                match node {
                    StringNode::Literal(constant) => group = group.string_constant(*constant),
                    StringNode::Expression { expression, format } => {
                        let format_string =
                            render_format_options(format, group.ctx.ast.constants());
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

#[derive(Clone)]
struct FormatContext<'source> {
    source: &'source str,
    ast: &'source Ast,
    options: &'source FormatOptions,
    // The byte offset of each line's start
    line_offsets: Vec<u32>,
}

impl<'source> FormatContext<'source> {
    fn new(source: &'source str, ast: &'source Ast, options: &'source FormatOptions) -> Self {
        let line_offsets = iter::once(0)
            .chain(
                source
                    .char_indices()
                    .filter_map(|(i, c)| if c == '\n' { Some(i as u32 + 1) } else { None }),
            )
            .collect();

        Self {
            source,
            ast,
            options,
            line_offsets,
        }
    }

    fn node(&self, ast_index: AstIndex) -> &AstNode {
        self.ast.node(ast_index)
    }

    fn span(&self, node: &AstNode) -> &Span {
        self.ast.span(node.span)
    }

    fn string_constant(&self, constant: ConstantIndex) -> &'source str {
        self.ast.constants().get_str(constant)
    }

    fn source_slice(&self, span: &Span) -> &'source str {
        let start = self.line_offsets[span.start.line as usize] + span.start.column;
        let end = self.line_offsets[span.end.line as usize] + span.end.column;
        &self.source[start as usize..end as usize]
    }
}

/// A helper for building a [FormatItem] group.
struct GroupBuilder<'source, 'trivia> {
    // The items added the group
    items: Vec<FormatItem<'source>>,
    // The start index in `self.items` of an active sub group,
    // see `sub_group_start()` / `sub_group_end()`.
    sub_group_start: Option<usize>,
    // The group node's span.
    group_span: Span,
    // The formatting context passed into the initializer.
    ctx: &'source FormatContext<'source>,
    // The source's trivia items.
    trivia: &'trivia mut TriviaIterator<'source>,
    // The current line in the input, updated as nodes are added to the group.
    current_line: u32,
    // True if a #[fmt:skip] directive was just encountered.
    skip_next_node: bool,
}

impl<'source, 'trivia> GroupBuilder<'source, 'trivia> {
    fn new(
        capacity: usize,
        group_node: &AstNode,
        ctx: &'source FormatContext<'source>,
        trivia: &'trivia mut TriviaIterator<'source>,
    ) -> Self {
        let group_span = *ctx.span(group_node);
        let current_line = group_span.start.line;
        Self {
            items: Vec::with_capacity(capacity),
            sub_group_start: None,
            group_span,
            ctx,
            trivia,
            current_line,
            skip_next_node: false,
        }
    }

    fn build(mut self) -> FormatItem<'source> {
        self.add_trivia(self.group_span.end, TriviaPosition::Any);

        FormatItem::make_group(self.items)
    }

    fn build_block(mut self) -> FormatItem<'source> {
        self.add_trivia(self.group_span.end, TriviaPosition::LineStart);
        self.strip_trailing_whitespace();

        FormatItem::make_group(self.items)
    }

    fn build_main_block(mut self) -> FormatItem<'source> {
        self.add_trivia(self.group_span.end, TriviaPosition::ScriptEnd);
        self.strip_trailing_whitespace();

        FormatItem::make_group(self.items)
    }

    fn strip_trailing_whitespace(&mut self) {
        while self.items.last().is_some_and(|item| item.is_break()) {
            self.items.pop();
        }
    }

    fn strip_trailing_breaks(&mut self) {
        while self.items.last().is_some_and(|item| match item {
            FormatItem::GroupBreak(group_break) => group_break.needs_linebreak(false, false, false),
            _ => false,
        }) {
            self.items.pop();
        }
    }

    fn sub_group_start(mut self) -> Self {
        debug_assert!(self.sub_group_start.is_none());
        self.sub_group_start = Some(self.items.len());
        self
    }

    fn sub_group_end(mut self) -> Self {
        if let Some(sub_group_start) = self.sub_group_start.take() {
            let sub_group_items = self.items.drain(sub_group_start..).collect();
            self.items.push(FormatItem::Group {
                items: sub_group_items,
                line_length: OnceCell::new(),
            });
        }

        self
    }

    fn char(mut self, c: char) -> Self {
        self.items.push(FormatItem::Char(c));
        self
    }

    fn maybe_char(mut self, c: char) -> Self {
        self.items.push(FormatItem::OptionalChar(c));
        self
    }

    fn str(mut self, s: &'source str) -> Self {
        self.items.push(s.into());
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

    fn group_break(&mut self, group_break: GroupBreak) {
        self.items.push(FormatItem::GroupBreak(group_break));
    }

    fn space_or_indent(mut self) -> Self {
        self.group_break(GroupBreak::SpaceOrIndent);
        self
    }

    fn space_or_return(mut self) -> Self {
        self.group_break(GroupBreak::SpaceOrReturn);
        self
    }

    fn space_or_indent_if_necessary(mut self) -> Self {
        self.group_break(GroupBreak::SpaceOrIndentIfNecessary);
        self
    }

    fn space_or_indent_respecting_existing_break(mut self, lhs: &AstIndex, rhs: &AstIndex) -> Self {
        let lhs_span = self.ctx.span(self.ctx.node(*lhs));
        let rhs_span = self.ctx.span(self.ctx.node(*rhs));
        if lhs_span.end.line < rhs_span.start.line {
            self.group_break(GroupBreak::IndentedBreak);
        } else {
            self.group_break(GroupBreak::SpaceOrIndentIfNecessary);
        }
        self
    }

    fn maybe_indent(mut self) -> Self {
        self.group_break(GroupBreak::MaybeIndent);
        self
    }

    fn indent_if_necessary(mut self) -> Self {
        self.group_break(GroupBreak::IndentIfNecessary);
        self
    }

    fn maybe_force_indent(mut self, force: bool) -> Self {
        if force {
            self.group_break(GroupBreak::IndentedBreak);
        } else {
            self.group_break(GroupBreak::MaybeIndent);
        }
        self
    }

    fn maybe_return(mut self) -> Self {
        self.group_break(GroupBreak::MaybeReturn);
        self
    }

    fn return_or_indent(mut self) -> Self {
        self.group_break(GroupBreak::ReturnOrIndent);
        self
    }

    fn indented_break(mut self) -> Self {
        // Add any trailing comments for the current line before adding the break.
        self = self.add_trailing_trivia();
        self.group_break(GroupBreak::IndentedBreak);
        self
    }

    fn indented_break_without_trivia(mut self) -> Self {
        self.group_break(GroupBreak::IndentedBreak);
        self
    }

    fn line_break(mut self) -> Self {
        self = self.add_trailing_trivia();
        self.strip_trailing_breaks();
        self.items.push(FormatItem::LineBreak);
        self
    }

    fn line_start(mut self, node_index: AstIndex) -> Self {
        let node = self.ctx.node(node_index);
        let node_span = self.ctx.span(node);
        self.add_trivia(node_span.start, TriviaPosition::LineStart);
        self.group_break(GroupBreak::LineStart);
        self
    }

    fn start_block(mut self) -> Self {
        self.strip_trailing_breaks();
        self.group_break(GroupBreak::StartBlock);
        self
    }

    // Add any trailing comments for the current line.
    fn add_trailing_trivia(mut self) -> Self {
        self.add_trivia(
            if self.items.is_empty() {
                self.current_line()
            } else {
                self.next_line()
            },
            TriviaPosition::LineEnd,
        );
        self
    }

    // Add any trivia that precedes the given node.
    fn add_preceding_trivia(mut self, node_index: AstIndex) -> Self {
        let node = self.ctx.node(node_index);
        let node_span = self.ctx.span(node);
        self.add_trivia(node_span.start, TriviaPosition::Any);
        self
    }

    fn node(mut self, node_index: AstIndex) -> Self {
        let node = self.ctx.node(node_index);
        let node_span = self.ctx.span(node);
        let node_end_line = node_span.end.line;

        // Add any trivia that should appear before the node
        self.add_trivia(node_span.start, TriviaPosition::Any);

        // Check to see if the node was preceded skip commands
        if self.skip_next_node {
            self.skip_next_node = false;

            // Skip rendering and add the node's source region directly
            // to the output.
            self.add_source_region(node_span);

            // Skip over any trivia that's already captured in the node's span
            while let Some(item) = self.trivia.peek() {
                if item.span.start < node_span.end {
                    self.trivia.next();
                } else {
                    break;
                }
            }
        } else {
            self.items
                .push(format_node(node_index, self.ctx, self.trivia));
        }

        self.current_line = node_end_line;
        self
    }

    fn add_source_region(&mut self, span: &Span) {
        self.items.push(self.ctx.source_slice(span).into());
    }

    // Adds the node, and then if it's a group, flattens its contents into this group
    fn node_maybe_flattened(mut self, node_index: AstIndex, flatten: bool) -> Self {
        if flatten {
            self = self.node(node_index);
            if let Some(FormatItem::Group { items, .. }) = self
                .items
                .pop_if(|item| matches!(item, FormatItem::Group { .. }))
            {
                self.items.extend(items);
            }
            self
        } else {
            self.node(node_index)
        }
    }

    fn nested(
        mut self,
        capacity: usize,
        nested_node: &AstNode,
        nested_fn: impl Fn(GroupBuilder<'source, '_>) -> FormatItem<'source>,
    ) -> Self {
        self.items.push(nested_fn(GroupBuilder::new(
            capacity,
            nested_node,
            self.ctx,
            self.trivia,
        )));
        self.current_line = self.ctx.span(nested_node).end.line;
        self
    }

    fn list_elements(mut self, elements: &[AstIndex]) -> Self {
        for (i, element) in elements.iter().enumerate() {
            self = self.node(*element);
            if i < elements.len() - 1 {
                self = self.char(',').space_or_indent_if_necessary();
            } else {
                self = self.maybe_char(',');
            }
        }
        self
    }

    fn tuple_elements(self, elements: &[AstIndex]) -> Self {
        if elements.len() == 1 {
            self.node(elements[0]).char(',')
        } else {
            self.list_elements(elements)
        }
    }

    // Add any trivia that needs to be inserted before the given position
    //
    // If a skip command is encountered, then the function exits and the command's span is returned.
    fn add_trivia(&mut self, position: Position, position_info: TriviaPosition) {
        // Add any trivia items that belong before the format item to the group
        while let Some(item) = self.trivia.peek() {
            let item_start = item.span.start;

            let consume_item = match position_info {
                TriviaPosition::LineStart => item_start.line < position.line,
                TriviaPosition::Any | TriviaPosition::LineEnd => item_start < position,
                TriviaPosition::ScriptEnd => true,
            };
            if !consume_item {
                break;
            }

            let item = *item;
            self.trivia.next();

            self.add_trivia_item(item, position, position_info);
        }
    }

    fn add_trivia_item(
        &mut self,
        item: &TriviaItem,
        position: Position,
        position_info: TriviaPosition,
    ) {
        match item.token {
            TriviaToken::EmptyLine => {
                self.strip_trailing_breaks();
                self.items.push(FormatItem::LineBreak);
            }
            TriviaToken::CommentSingle | TriviaToken::SkipNode => {
                if item.token == TriviaToken::SkipNode {
                    self.skip_next_node = true;
                }

                match position_info {
                    TriviaPosition::LineStart => {
                        if item.span.end.line < position.line {
                            self.group_break(GroupBreak::LineStart);
                        }
                    }
                    _ if self.items.last().is_some_and(|item| !item.is_break()) => {
                        self.group_break(GroupBreak::SpaceOrIndentIfNecessary);
                    }
                    _ => {}
                }

                self.add_source_region(&item.span);

                match position_info {
                    TriviaPosition::LineStart | TriviaPosition::ScriptEnd => {
                        self.items.push(FormatItem::LineBreak)
                    }
                    TriviaPosition::Any | TriviaPosition::LineEnd => {
                        self.group_break(GroupBreak::IndentedBreak)
                    }
                }
            }
            TriviaToken::CommentMulti => {
                match position_info {
                    TriviaPosition::LineStart if item.span.end.line < position.line => {
                        self.group_break(GroupBreak::LineStart);
                    }
                    TriviaPosition::LineEnd
                        if self.items.last().is_some_and(|item| !item.is_break()) =>
                    {
                        self.group_break(GroupBreak::SpaceOrIndentIfNecessary);
                    }
                    _ => {}
                }

                self.add_source_region(&item.span);

                match position_info {
                    TriviaPosition::LineStart if item.span.end.line < position.line => {
                        self.items.push(FormatItem::LineBreak);
                    }
                    _ => {
                        self.group_break(GroupBreak::SpaceOrIndentIfNecessary);
                    }
                }
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
    // A single character that's only included in the output when the line is being broken up
    OptionalChar(char),
    // A `&str`, either static or from the source file
    Str(&'source str),
    // A KString
    KString(KString),
    // A grouped sequence of items
    // The group will be rendered on a single line by default, or if the group doesn't fit in the
    // remaining space then it will be rendered with breaks replaced with appropriate indentation.
    Group {
        items: Vec<Self>,
        // The group's length if it was rendered on a single line.
        // This gets calculated and cached during rendering to avoid nested group recalculations.
        line_length: OnceCell<usize>,
    },
    // An explicit linebreak
    LineBreak,
    // A group break
    GroupBreak(GroupBreak),
    // An error that occurred while preparing the item tree
    //
    // Errors will be very rare (only occurring with a malformed AST), so rather than checking for
    // errors everywhere in `format_node` the error gets propagated during rendering.
    Error(Error),
}

impl<'source> FormatItem<'source> {
    // Used for keyword/value expressions like `return true`
    fn from_keyword_and_value<'trivia>(
        keyword: &'source str,
        value: &AstIndex,
        group_node: &AstNode,
        ctx: &'source FormatContext<'source>,
        trivia: &'trivia mut TriviaIterator<'source>,
    ) -> Self {
        GroupBuilder::new(3, group_node, ctx, trivia)
            .str(keyword)
            .space_or_indent()
            .node(*value)
            .build()
    }

    fn make_group(items: Vec<Self>) -> Self {
        Self::Group {
            items,
            line_length: OnceCell::new(),
        }
    }

    // Renders the format item, appending to the provided output string
    fn render(
        &self,
        output: &mut String,
        indented: bool,
        render_optional: bool,
        options: &FormatOptions,
        column: usize,
    ) -> Result<()> {
        match self {
            Self::Char(c) => output.push(*c),
            Self::OptionalChar(c) => {
                if render_optional {
                    output.push(*c)
                }
            }
            Self::Str(s) => output.push_str(s),
            Self::KString(s) => output.push_str(s),
            Self::Group { items, .. } => {
                self.render_group(items, indented, output, options, column)?
            }
            Self::LineBreak => output.push('\n'),
            Self::GroupBreak(group_break) => match group_break {
                GroupBreak::SpaceOrIndent
                | GroupBreak::SpaceOrIndentIfNecessary
                | GroupBreak::SpaceOrReturn => output.push(' '),
                _ => {}
            },
            Self::Error(error) => return Err(error.clone()),
        }

        Ok(())
    }

    fn is_indented_block(&self) -> bool {
        match self {
            Self::Group { items, .. } => matches!(
                items.first(),
                Some(Self::GroupBreak(GroupBreak::StartBlock))
            ),
            _ => false,
        }
    }

    fn render_group(
        &self,
        items: &[FormatItem<'source>],
        indented: bool, // true when the group has been force-indented by the parent
        output: &mut String,
        options: &FormatOptions,
        mut column: usize,
    ) -> Result<()> {
        let columns_remaining = (options.line_length as usize).saturating_sub(column);
        let too_long = self.line_length() > columns_remaining;
        let force_break = items.iter().any(FormatItem::force_break);

        // Use indent logic if the line is too long, if one of the group contains a forced break,
        // or if the last item is an indented block.
        if too_long || force_break || items.last().is_some_and(FormatItem::is_indented_block) {
            let mut group_start_indent = " ".repeat(column);
            let extra_indent = " ".repeat(options.indent_width as usize);
            let mut group_column = column;
            let mut group_break = GroupBreak::None;
            let mut item_buffer = String::new();
            let mut line_width = group_column;
            let mut child_is_indented = false;
            let mut first_item = true;

            for item in items {
                let accept_optional_linebreak = !(first_item && indented);

                match item {
                    Self::GroupBreak(new_break) => {
                        group_break = *new_break;
                        match group_break {
                            GroupBreak::StartBlock => {
                                // Reinitialize the group's start indent
                                column += extra_indent.len();
                                group_column = column;
                                group_start_indent = " ".repeat(column);
                                output.push('\n');
                                group_break = GroupBreak::None;
                            }
                            GroupBreak::IndentedBreak => {
                                if indented {
                                    group_break = GroupBreak::MaybeIndent;
                                }
                            }
                            _ => {}
                        }
                    }
                    Self::LineBreak => {
                        output.push('\n');
                        group_break = GroupBreak::None;
                    }
                    _ if item.is_indented_block() => {
                        // No need to worry about adjusting the line width here,
                        // an indented block is always the last item in a group.
                        item.render(&mut item_buffer, false, false, options, group_column)?;
                        output.extend(item_buffer.drain(..));
                        group_break = GroupBreak::None;
                    }
                    _ => {
                        // Adjust the column for the item to be rendered
                        if group_break.needs_linebreak(
                            too_long,
                            force_break,
                            accept_optional_linebreak,
                        ) {
                            group_column = column;

                            if group_break.needs_indent(too_long, force_break, indented) {
                                group_column += extra_indent.len();
                                child_is_indented = true;
                            }
                        }

                        // Render the item into a temporary buffer
                        let render_optional = too_long || child_is_indented;
                        item.render(
                            &mut item_buffer,
                            child_is_indented,
                            render_optional,
                            options,
                            group_column,
                        )?;

                        // Get the width of the item's first line
                        // (multiline items are possible, and we only need the first line's width
                        // to decide if a linebreak is necessary).
                        let mut item_first_line_width = first_line_length(&item_buffer);

                        // Check for 'indented break if necessary' items
                        match group_break {
                            GroupBreak::SpaceOrIndentIfNecessary => {
                                // +1 for a space
                                let line_width_with_item = line_width + item_first_line_width + 1;
                                if line_width_with_item > options.line_length as usize {
                                    group_break = if indented {
                                        GroupBreak::MaybeReturn
                                    } else {
                                        GroupBreak::IndentedBreak
                                    };
                                } else {
                                    if item_first_line_width > 0 {
                                        output.push(' ');
                                        item_first_line_width += 1;
                                    }
                                    group_break = GroupBreak::None;
                                }
                            }
                            GroupBreak::IndentIfNecessary => {
                                let line_width_with_item = line_width + item_first_line_width;
                                if line_width_with_item > options.line_length as usize {
                                    group_break = if indented {
                                        GroupBreak::MaybeReturn
                                    } else {
                                        GroupBreak::IndentedBreak
                                    };
                                } else {
                                    group_break = GroupBreak::None;
                                }
                            }
                            _ => {}
                        }

                        // Emit linebreaks if necessary
                        if group_break.needs_linebreak(
                            too_long,
                            force_break,
                            accept_optional_linebreak,
                        ) {
                            output.push('\n');
                            output.push_str(&group_start_indent);
                            group_column = column;
                            line_width = group_start_indent.len();

                            if group_break.needs_indent(too_long, force_break, indented) {
                                group_column = column + extra_indent.len();
                                output.push_str(&extra_indent);
                                line_width = group_column;
                                child_is_indented = true;
                            }
                        } else if group_break.needs_return(too_long, force_break, indented) {
                            output.push_str(&group_start_indent);
                            group_column = column;
                            line_width = group_start_indent.len();
                            child_is_indented = false;
                        } else if group_break.needs_space(too_long) {
                            output.push(' ');
                        }

                        group_break = GroupBreak::None;

                        // Add the item to the output
                        let item_last_line_width = item_buffer
                            .rsplit_once('\n')
                            .map(|(_rest, last)| last.width())
                            .unwrap_or(item_first_line_width);
                        output.extend(item_buffer.drain(..));
                        line_width += item_last_line_width;
                        first_item = false;
                    }
                }
            }
        } else {
            for item in items {
                item.render(output, false, false, options, column)?;
            }
        }

        Ok(())
    }

    // Gets the length of the format item
    fn line_length(&self) -> usize {
        match self {
            Self::Char(c) | Self::OptionalChar(c) => c.width().unwrap_or(0),
            Self::Str(s) => first_line_length(s),
            Self::KString(s) => first_line_length(s),
            Self::Group {
                line_length, items, ..
            } => *line_length.get_or_init(|| {
                items
                    .iter()
                    .take_while(|item| match item {
                        Self::GroupBreak(group_break) => {
                            !group_break.needs_linebreak(false, false, false)
                        }
                        _ => true,
                    })
                    .map(Self::line_length)
                    .sum()
            }),
            Self::GroupBreak(group_break) => group_break.line_length(),
            Self::LineBreak | Self::Error(_) => 0,
        }
    }

    fn force_break(&self) -> bool {
        match self {
            Self::LineBreak => true,
            Self::GroupBreak(group_break) => group_break.needs_linebreak(false, false, false),
            _ => false,
        }
    }

    fn is_break(&self) -> bool {
        matches!(self, Self::LineBreak | Self::GroupBreak(_))
    }
}

impl<'source> From<&'source str> for FormatItem<'source> {
    fn from(s: &'source str) -> Self {
        Self::Str(s)
    }
}

impl From<Error> for FormatItem<'_> {
    fn from(error: Error) -> Self {
        Self::Error(error)
    }
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

#[derive(Copy, Clone, Debug)]
enum GroupBreak {
    // No break necessary
    None,
    // A space or indented linebreak, always breaking when the line is too long
    SpaceOrIndent,
    // A space or indented linebreak, only breaking if necessary
    SpaceOrIndentIfNecessary,
    // A space, or a point where a long line can be broken with a return to the start column
    SpaceOrReturn,
    // A point where a long line can be broken with an indent
    MaybeIndent,
    // An indented linebreak, only breaking when necessary
    IndentIfNecessary,
    // A point where a long line can be broken with a return to the start column
    MaybeReturn,
    // Forces a group to be broken onto multiple lines if followed by anything other than
    // an indented block.
    IndentedBreak,
    // A break, with either an indent, or a return to the already-indented start column
    ReturnOrIndent,
    // Forces the start of a new line, indented to the group's column
    LineStart,
    // The start of an indented block
    StartBlock,
}

impl GroupBreak {
    fn needs_space(&self, line_is_too_long: bool) -> bool {
        match self {
            Self::SpaceOrIndent | Self::SpaceOrIndentIfNecessary | Self::SpaceOrReturn => {
                !line_is_too_long
            }
            Self::None
            | Self::MaybeIndent
            | Self::IndentIfNecessary
            | Self::MaybeReturn
            | Self::IndentedBreak
            | Self::ReturnOrIndent
            | Self::LineStart
            | Self::StartBlock => false,
        }
    }

    fn needs_linebreak(
        &self,
        line_is_too_long: bool,
        force_break: bool,
        accept_optional_linebreak: bool,
    ) -> bool {
        match self {
            Self::None => false,
            // Handled separately in render_group
            Self::SpaceOrIndentIfNecessary | Self::IndentIfNecessary | Self::LineStart => false,
            Self::IndentedBreak | Self::ReturnOrIndent | Self::StartBlock => true,
            Self::SpaceOrIndent | Self::SpaceOrReturn => line_is_too_long,
            Self::MaybeReturn | Self::MaybeIndent => {
                force_break || accept_optional_linebreak && line_is_too_long
            }
        }
    }

    fn needs_indent(
        &self,
        line_is_too_long: bool,
        force_break: bool,
        already_indented: bool,
    ) -> bool {
        match self {
            Self::IndentedBreak | Self::LineStart => true,
            Self::SpaceOrIndent => line_is_too_long,
            Self::MaybeIndent => line_is_too_long || force_break,
            Self::None | Self::StartBlock | Self::SpaceOrReturn | Self::MaybeReturn => false,
            // Handled separately in render_group
            Self::SpaceOrIndentIfNecessary | Self::IndentIfNecessary => false,
            Self::ReturnOrIndent => !already_indented,
        }
    }

    fn needs_return(
        &self,
        line_is_too_long: bool,
        force_break: bool,
        already_indented: bool,
    ) -> bool {
        match self {
            Self::LineStart | Self::SpaceOrReturn => true,
            Self::MaybeReturn => line_is_too_long || force_break,
            Self::None
            | Self::IndentedBreak
            | Self::SpaceOrIndent
            | Self::MaybeIndent
            | Self::StartBlock => false,
            // Handled separately in render_group
            Self::SpaceOrIndentIfNecessary | Self::IndentIfNecessary => false,
            Self::ReturnOrIndent => already_indented,
        }
    }

    fn line_length(&self) -> usize {
        match self {
            // Rendered in single lines as a space
            Self::SpaceOrIndent | Self::SpaceOrIndentIfNecessary => 1,
            _ => 0,
        }
    }
}

/// An indicator used to define how trivia should be added to the output.
#[derive(Copy, Clone, Debug)]
enum TriviaPosition {
    /// Used when trivia could be anywhere in the output, e.g. inline comments between nodes
    Any,
    /// Used when a new line is being started.
    LineStart,
    /// Used when a line is being ended, and any trailing trivia should be added.
    LineEnd,
    /// Used at the end of the script to capture any remaining trivia.
    ScriptEnd,
}

fn should_chain_be_broken<'source>(
    root_node: &ChainNode,
    mut chain_next: &'source Option<AstIndex>,
    ctx: &'source FormatContext<'source>,
) -> bool {
    let mut chain_node = root_node;
    let mut dot_access_count = 0;
    let mut last_node_was_access = false;
    let mut chain_line = 0;

    loop {
        match chain_node {
            ChainNode::Root(root_node) => {
                chain_line = ctx.span(ctx.node(*root_node)).end.line;
                last_node_was_access = false;
            }
            ChainNode::Call { with_parens, .. } => {
                if !with_parens && chain_next.is_some() {
                    return true;
                }
                if last_node_was_access {
                    dot_access_count += 1;
                }
                last_node_was_access = false;
            }
            ChainNode::Id(_) | ChainNode::Str(_) => {
                last_node_was_access = true;
            }
            ChainNode::Index(_) => {
                if last_node_was_access {
                    dot_access_count += 1;
                }
                last_node_was_access = false;
            }
            _ => last_node_was_access = false,
        }

        if dot_access_count >= ctx.options.chain_break_threshold {
            return true;
        }

        if let Some(next) = chain_next {
            let next_node = ctx.node(*next);
            match &next_node.node {
                Node::Chain((next_chain_node, next_next)) => {
                    chain_node = next_chain_node;
                    chain_next = next_next;
                }
                _other => panic!("Expected chain node"),
            }
            if ctx.span(next_node).end.line > chain_line {
                return true;
            }
        } else {
            break;
        }
    }

    false
}

fn first_line_length(s: &str) -> usize {
    s.split_once('\n')
        .map(|(first, _rest)| first)
        .unwrap_or(s)
        .width()
}
