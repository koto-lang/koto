use crate::{
    Chunk, DebugInfo, FunctionFlags, Op, StringFormatFlags,
    frame::{Arg, AssignedOrReserved, Frame, FrameError},
};
use circular_buffer::CircularBuffer;
use derive_name::VariantName;
use koto_parser::{
    Ast, AstBinaryOp, AstFor, AstIf, AstIndex, AstNode, AstTry, AstUnaryOp, AstVec, ChainNode,
    ConstantIndex, Function, ImportItem, KString, MetaKeyId, Node, Parser, Span, StringContents,
    StringFormatOptions, StringNode,
};
use smallvec::{SmallVec, smallvec};
use thiserror::Error;

/// The different error types that can be thrown by the Koto runtime
#[derive(Error, Clone, Debug)]
#[allow(missing_docs)]
enum ErrorKind {
    #[error("expected {expected}, found '{}'", unexpected.variant_name())]
    UnexpectedNode { expected: String, unexpected: Node },
    #[error("attempting to assign to a temporary value")]
    AssigningToATemporaryValue,
    #[error("all arguments following an optional argument must also be optional")]
    ExpectedOptionalArgumentValue,
    #[error("invalid {kind} op ({op:?})")]
    InvalidBinaryOp { kind: String, op: AstBinaryOp },
    #[error("`{0}` used outside of loop")]
    InvalidLoopKeyword(String),
    #[error("invalid match pattern (found '{0:?}')")]
    InvalidMatchPattern(Node),
    #[error("args with ellipses are only allowed in first or last position")]
    InvalidPositionForArgWithEllipses,
    #[error(
        "the jump offset here is too large. {0} bytes is larger than the maximum of {max}.
             Try breaking up this part of the program a bit",
        max = u16::MAX
    )]
    JumpOffsetIsTooLarge(usize),
    #[error("function has too many {property} ({amount})")]
    FunctionPropertyLimit { property: String, amount: usize },
    #[error("missing argument in for loop")]
    MissingArgumentInForLoop,
    #[error("missing arg register")]
    MissingArgRegister,
    #[error("missing item to import")]
    MissingImportItem,
    #[error("missing next node while compiling a chain")]
    MissingNextChainNode,
    #[error("missing chain parent register")]
    MissingChainParentRegister,
    #[error("missing result register")]
    MissingResultRegister,
    #[error("missing String nodes")]
    MissingStringNodes,
    #[error("a type check is needed when the catch block isn't the last in the try expression")]
    MissingTypeCheckOnCatchBlock,
    #[error("missing value for Map entry")]
    MissingValueForMapEntry,
    #[error("only one ellipsis is allowed in a match arm")]
    MultipleMatchEllipses,
    #[error("the compiled expression has no output")]
    NoResultInExpressionOutput,
    #[error("child chain node out of position")]
    OutOfPositionChildNodeInChain,
    #[error("matching with ellipses is only allowed in first or last position")]
    OutOfPositionMatchEllipsis,
    #[error("root chain node out of position")]
    OutOfPositionRootNodeInChain,
    #[error("the compiled bytecode is larger than the maximum size of 4GB (size: {0} bytes)")]
    ResultingBytecodeIsTooLarge(usize),
    #[error("too many targets in assignment ({0})")]
    TooManyAssignmentTargets(usize),
    #[error(
        "too many container entries, {0} is greater than the maximum of {max}",
        max = u32::MAX
    )]
    TooManyContainerEntries(usize),
    #[error("a type check can't be used on the last catch block in a try expression")]
    TypeCheckOnLastCatchBlock,
    #[error("the result of this `break` expression will be ignored")]
    UnassignedBreakValue,
    #[error("unexpected ellipsis")]
    UnexpectedEllipsis,
    #[error("attempting to access an ignored value")]
    UnexpectedIgnoredValue,
    #[error("expected {expected} patterns in match arm, found {unexpected}")]
    UnexpectedMatchPatternCount { expected: usize, unexpected: usize },

    #[error("{0}")]
    Parser(#[from] koto_parser::Error),
    #[error(transparent)]
    FrameError(#[from] FrameError),
}

type Result<T> = std::result::Result<T, CompilerError>;

/// The error type used to report errors during compilation
#[derive(Error, Clone, Debug)]
#[error("{error}")]
pub struct CompilerError {
    /// The error's message
    error: ErrorKind,
    /// The span in the source where the error occurred
    pub span: Span,
}

impl CompilerError {
    /// Returns true if the error was caused by the expectation of indentation during parsing
    pub fn is_indentation_error(&self) -> bool {
        match &self.error {
            ErrorKind::Parser(e) => e.is_indentation_error(),
            _ => false,
        }
    }
}

impl From<koto_parser::Error> for CompilerError {
    fn from(error: koto_parser::Error) -> Self {
        Self {
            span: error.span,
            error: error.into(),
        }
    }
}

#[derive(Copy, Clone)]
struct CompileNodeContext<'a> {
    ast: &'a Ast,
    result_register: ResultRegister,
}

impl<'a> CompileNodeContext<'a> {
    fn new(ast: &'a Ast, result_register: ResultRegister) -> Self {
        Self {
            ast,
            result_register,
        }
    }

    fn with_register(self, result_register: ResultRegister) -> Self {
        Self {
            result_register,
            ..self
        }
    }

    fn with_fixed_register_or_none(self, register: Option<u8>) -> Self {
        Self {
            result_register: register.map_or(ResultRegister::None, ResultRegister::Fixed),
            ..self
        }
    }

    fn with_any_register(self) -> Self {
        Self {
            result_register: ResultRegister::Any,
            ..self
        }
    }

    fn with_fixed_register(self, register: u8) -> Self {
        Self {
            result_register: ResultRegister::Fixed(register),
            ..self
        }
    }

    fn with_fixed_register_or_any(self) -> Self {
        let result_register = if matches!(self.result_register, ResultRegister::Fixed(_)) {
            self.result_register
        } else {
            ResultRegister::Any
        };
        Self {
            result_register,
            ..self
        }
    }

    fn compile_for_side_effects(self) -> Self {
        Self {
            result_register: ResultRegister::None,
            ..self
        }
    }

    fn node(&self, ast_index: AstIndex) -> &Node {
        &self.ast.node(ast_index).node
    }

    fn node_with_span(&self, ast_index: AstIndex) -> &AstNode {
        self.ast.node(ast_index)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ResultRegister {
    // The result will be ignored, expressions without side-effects can be dropped.
    None,
    // The result can be an assigned register or placed in a temporary register.
    Any,
    // The result must be placed in the specified register.
    Fixed(u8),
}

// ResultRegister::Any might cause a temporary register to be assigned.
// This means that when compiling a node, the result register should always be determined as a first
// step before other temporary registers are assigned, so that the temporary nodes can be discarded
// without removing the result register.
#[derive(Clone, Copy, Debug, Default)]
struct CompileNodeOutput {
    register: Option<u8>,
    // The caller of compile_node is responsible for discarding temporary registers when the result
    // is no longer needed.
    is_temporary: bool,
}

impl CompileNodeOutput {
    fn none() -> Self {
        Self {
            register: None,
            is_temporary: false,
        }
    }

    fn with_assigned(register: u8) -> Self {
        Self {
            register: Some(register),
            is_temporary: false,
        }
    }

    fn with_temporary(register: u8) -> Self {
        Self {
            register: Some(register),
            is_temporary: true,
        }
    }

    fn unwrap(&self, compiler: &Compiler) -> Result<u8> {
        self.register
            .ok_or_else(|| compiler.make_error(ErrorKind::NoResultInExpressionOutput))
    }
}

/// The settings used by the [Compiler]
#[derive(Copy, Clone)]
pub struct CompilerSettings {
    /// Whether or not top-level identifiers should be automatically exported
    ///
    /// The default behaviour in Koto is that `export` expressions are required to make a value
    /// available outside of the current module.
    ///
    /// This is used by the REPL, allowing for incremental compilation and execution of expressions
    /// that need to share declared values.
    pub export_top_level_ids: bool,

    /// When enabled, the compiler will emit type check instructions when type hints are encountered
    /// that will be performed at runtime.
    ///
    /// Enabled by default.
    pub enable_type_checks: bool,
}

impl Default for CompilerSettings {
    fn default() -> Self {
        Self {
            export_top_level_ids: false,
            enable_type_checks: true,
        }
    }
}

/// The compiler used by the Koto language
#[derive(Default)]
pub struct Compiler {
    bytes: Vec<u8>,
    debug_info: DebugInfo,
    frame_stack: Vec<Frame>,
    span_stack: Vec<Span>,
    settings: CompilerSettings,
}

impl Compiler {
    /// Compiles a script using the provided settings, returning a compiled [Chunk]
    pub fn compile(
        script: &str,
        script_path: Option<KString>,
        settings: CompilerSettings,
    ) -> Result<Chunk> {
        let ast = Parser::parse(script)?;
        let mut result = Self::compile_ast(ast, script_path, settings)?;
        result.debug_info.source = script.to_string();
        Ok(result)
    }

    /// Compiles an [Ast] using the provided settings, returning a compiled [Chunk]
    pub fn compile_ast(
        ast: Ast,
        script_path: Option<KString>,
        settings: CompilerSettings,
    ) -> Result<Chunk> {
        let mut compiler = Compiler {
            settings,
            ..Default::default()
        };

        if let Some(entry_point) = ast.entry_point() {
            compiler.compile_node(
                entry_point,
                CompileNodeContext::new(&ast, ResultRegister::None),
            )?;
        }

        if compiler.bytes.len() > u32::MAX as usize {
            return compiler.error(ErrorKind::ResultingBytecodeIsTooLarge(compiler.bytes.len()));
        }

        let result = Chunk {
            bytes: compiler.bytes,
            constants: ast.consume_constants(),
            path: script_path,
            debug_info: compiler.debug_info,
        };

        Ok(result)
    }

    fn compile_node(
        &mut self,
        node_index: AstIndex,
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        use Op::*;

        let node = ctx.node_with_span(node_index);

        self.push_span(node, ctx.ast);

        if !self.frame_stack.is_empty() {
            self.frame_mut().last_node_was_return = matches!(&node.node, Node::Return(_));
        }

        let result = match &node.node {
            Node::Null => {
                let result = self.assign_result_register(ctx)?;
                if let Some(result) = result.register {
                    self.push_op(SetNull, &[result]);
                }
                result
            }
            Node::Nested(nested) => self.compile_node(*nested, ctx)?,
            Node::Id(index, ..) => self.compile_load_id(*index, ctx)?,
            Node::Chain(chain) => self.compile_chain(chain, None, None, None, ctx)?,
            Node::BoolTrue => {
                let result = self.assign_result_register(ctx)?;
                if let Some(result) = result.register {
                    self.push_op(SetTrue, &[result]);
                }
                result
            }
            Node::BoolFalse => {
                let result = self.assign_result_register(ctx)?;
                if let Some(result) = result.register {
                    self.push_op(SetFalse, &[result]);
                }
                result
            }
            Node::SmallInt(n) => {
                let result = self.assign_result_register(ctx)?;
                if let Some(result) = result.register {
                    match *n {
                        0 => self.push_op(Set0, &[result]),
                        1 => self.push_op(Set1, &[result]),
                        n if n >= 0 => self.push_op(SetNumberU8, &[result, n as u8]),
                        n => self.push_op(SetNumberNegU8, &[result, n.unsigned_abs() as u8]),
                    }
                }
                result
            }
            Node::Float(constant) => {
                let result = self.assign_result_register(ctx)?;
                if let Some(result) = result.register {
                    self.compile_constant_op(result, *constant, LoadFloat);
                }
                result
            }
            Node::Int(constant) => {
                let result = self.assign_result_register(ctx)?;
                if let Some(result) = result.register {
                    self.compile_constant_op(result, *constant, LoadInt);
                }
                result
            }
            Node::Str(string) => self.compile_string(&string.contents, ctx)?,
            Node::List(elements) => {
                self.compile_make_sequence(elements, Op::SequenceToList, ctx)?
            }
            Node::Map { entries, .. } => self.compile_make_map(entries, false, ctx)?,
            Node::MapEntry(_, value) => {
                // We only get here when evaluating map entries for side-effects,
                // so we only need to compile the value here.
                self.compile_node(*value, ctx)?
            }
            Node::Self_ => {
                // self is always in register 0
                match ctx.result_register {
                    ResultRegister::None => CompileNodeOutput::none(),
                    ResultRegister::Any => CompileNodeOutput::with_assigned(0),
                    ResultRegister::Fixed(register) => {
                        self.push_op(Op::Copy, &[register, 0]);
                        CompileNodeOutput::with_assigned(register)
                    }
                }
            }
            Node::Range {
                start,
                end,
                inclusive,
            } => {
                let result = self.assign_result_register(ctx)?;

                if let Some(result_register) = result.register {
                    let start_result = self.compile_node(*start, ctx.with_any_register())?;
                    let end_result = self.compile_node(*end, ctx.with_any_register())?;

                    let op = if *inclusive { RangeInclusive } else { Range };
                    self.push_op(
                        op,
                        &[
                            result_register,
                            start_result.unwrap(self)?,
                            end_result.unwrap(self)?,
                        ],
                    );

                    if start_result.is_temporary {
                        self.pop_register()?;
                    }
                    if end_result.is_temporary {
                        self.pop_register()?;
                    }

                    result
                } else {
                    self.compile_node(*start, ctx.compile_for_side_effects())?;
                    self.compile_node(*end, ctx.compile_for_side_effects())?
                }
            }
            Node::RangeFrom { start } => {
                let result = self.assign_result_register(ctx)?;
                match result.register {
                    Some(result_register) => {
                        let start_result = self.compile_node(*start, ctx.with_any_register())?;

                        self.push_op(RangeFrom, &[result_register, start_result.unwrap(self)?]);

                        if start_result.is_temporary {
                            self.pop_register()?;
                        }

                        result
                    }
                    None => self.compile_node(*start, ctx.compile_for_side_effects())?,
                }
            }
            Node::RangeTo { end, inclusive } => {
                let result = self.assign_result_register(ctx)?;
                match result.register {
                    Some(result_register) => {
                        let end_result = self.compile_node(*end, ctx.with_any_register())?;

                        let op = if *inclusive {
                            RangeToInclusive
                        } else {
                            RangeTo
                        };
                        self.push_op(op, &[result_register, end_result.unwrap(self)?]);

                        if end_result.is_temporary {
                            self.pop_register()?;
                        }

                        result
                    }
                    None => self.compile_node(*end, ctx.compile_for_side_effects())?,
                }
            }
            Node::RangeFull => {
                let result = self.assign_result_register(ctx)?;
                if let Some(result_register) = result.register {
                    self.push_op(RangeFull, &[result_register]);
                }
                result
            }
            Node::MainBlock { body, local_count } => {
                self.compile_frame(
                    FrameParameters {
                        local_count: *local_count as u8,
                        expressions: body,
                        args: &[],
                        captures: &[],
                        allow_implicit_return: true,
                        output_type: None,
                        is_generator: false,
                    },
                    ctx,
                )?;
                CompileNodeOutput::none()
            }
            Node::Block(expressions) => self.compile_block(expressions, ctx)?,
            Node::Tuple { elements, .. } => {
                self.compile_make_sequence(elements, Op::SequenceToTuple, ctx)?
            }
            Node::TempTuple(elements) => self.compile_make_temp_tuple(elements, ctx)?,
            Node::Function(f) => self.compile_function(f, ctx)?,
            Node::Import { from, items } => self.compile_import(from, items, ctx)?,
            Node::Export(expression) => self.compile_export(*expression, ctx)?,
            Node::Assign {
                target, expression, ..
            } => self.compile_assign(*target, *expression, false, ctx)?,
            Node::MultiAssign {
                targets,
                expression,
                ..
            } => self.compile_multi_assign(targets, *expression, false, ctx)?,
            Node::UnaryOp { op, value } => self.compile_unary_op(*op, *value, ctx)?,
            Node::BinaryOp { op, lhs, rhs } => self.compile_binary_op(*op, *lhs, *rhs, ctx)?,
            Node::If(ast_if) => self.compile_if(ast_if, ctx)?,
            Node::Match { expression, arms } => self.compile_match(*expression, arms, ctx)?,
            Node::MatchArm { .. } => {
                // Match arms are only compiled in `self.compile_match`.
                unreachable!();
            }
            Node::Switch(arms) => self.compile_switch(arms, ctx)?,
            Node::SwitchArm { .. } => {
                // Switch arms are only compiled in `self.compile_switch`.
                unreachable!();
            }
            Node::PackedId(_) => return self.error(ErrorKind::UnexpectedEllipsis),
            Node::PackedExpression(_) => return self.error(ErrorKind::UnexpectedEllipsis),
            Node::Ignored(..) => return self.error(ErrorKind::UnexpectedIgnoredValue),
            Node::For(ast_for) => self.compile_for(ast_for, ctx)?,
            Node::While { condition, body } => {
                self.compile_loop(Some((*condition, false)), *body, ctx)?
            }
            Node::Until { condition, body } => {
                self.compile_loop(Some((*condition, true)), *body, ctx)?
            }
            Node::Loop { body } => self.compile_loop(None, *body, ctx)?,
            Node::Break(expression) => match self.frame().current_loop() {
                Some(loop_info) => {
                    let loop_result_register = loop_info.result_register;

                    match (loop_result_register, expression) {
                        (Some(loop_result_register), Some(expression)) => {
                            self.compile_node(
                                *expression,
                                ctx.with_fixed_register(loop_result_register),
                            )?;
                        }
                        (Some(loop_result_register), None) => {
                            self.push_op(SetNull, &[loop_result_register]);
                        }
                        (None, Some(_)) => return self.error(ErrorKind::UnassignedBreakValue),
                        (None, None) => {}
                    }

                    self.push_op(Jump, &[]);
                    self.push_loop_jump_placeholder()?;

                    CompileNodeOutput::none()
                }
                None => return self.error(ErrorKind::InvalidLoopKeyword("break".into())),
            },
            Node::Continue => match self.frame().current_loop() {
                Some(loop_info) => {
                    let loop_result_register = loop_info.result_register;
                    let loop_start_ip = loop_info.start_ip;

                    if let Some(result_register) = loop_result_register {
                        self.push_op(SetNull, &[result_register]);
                    }
                    self.push_jump_back_op(JumpBack, &[], loop_start_ip);

                    CompileNodeOutput::none()
                }
                None => return self.error(ErrorKind::InvalidLoopKeyword("continue".into())),
            },
            Node::Return(expression) => self.compile_return(*expression, node_index, ctx)?,
            Node::Yield(expression) => self.compile_yield(*expression, node_index, ctx)?,
            Node::Throw(expression) => {
                // A throw will prevent the result from being used, but the caller should be
                // provided with a result register regardless.
                let result = self.assign_result_register(ctx)?;

                let expression_result = self.compile_node(*expression, ctx.with_any_register())?;
                let expression_register = expression_result.unwrap(self)?;

                self.push_op(Throw, &[expression_register]);

                if expression_result.is_temporary {
                    self.pop_register()?;
                }

                result
            }
            Node::Try(try_expression) => self.compile_try_expression(try_expression, ctx)?,
            Node::Debug {
                expression_string,
                expression,
            } => {
                let expression_context = match ctx.result_register {
                    ResultRegister::None => ctx.with_any_register(),
                    _ => ctx,
                };

                let expression_result = self.compile_node(*expression, expression_context)?;
                let expression_register = expression_result.unwrap(self)?;

                self.push_op(Debug, &[expression_register]);
                self.push_var_u32(u32::from(*expression_string));

                expression_result
            }
            Node::Meta(_, _) => {
                // Meta nodes are currently only compiled in the context of an export assignment,
                // see compile_assign().
                unreachable!();
            }
            Node::Type { .. } => {
                // Type hints are only compiled in the context of typed identifiers.
                unreachable!();
            }
            Node::FunctionArgs { .. } => {
                // FunctionArgs are only compiled in compile_function.
                unreachable!();
            }
        };

        self.pop_span();

        Ok(result)
    }

    fn assign_result_register(&mut self, ctx: CompileNodeContext) -> Result<CompileNodeOutput> {
        let result = match ctx.result_register {
            ResultRegister::Fixed(register) => CompileNodeOutput::with_assigned(register),
            ResultRegister::Any => CompileNodeOutput::with_temporary(self.push_register()?),
            ResultRegister::None => CompileNodeOutput::none(),
        };

        Ok(result)
    }

    fn compile_frame(&mut self, params: FrameParameters, ctx: CompileNodeContext) -> Result<()> {
        // Push a NewFrame op, and keep track of the register count's byte index so that it can be
        // updated after the frame has been compiled.
        self.push_op(Op::NewFrame, &[0]);
        let register_count_byte_index = self.bytes.len() - 1;

        let FrameParameters {
            local_count,
            expressions,
            args,
            captures,
            allow_implicit_return,
            output_type,
            is_generator,
        } = params;

        self.frame_stack.push(Frame::new(
            local_count,
            &self.collect_args(args, ctx)?,
            captures,
            output_type,
            is_generator,
        ));

        // Check argument types and unpack nested args
        for (arg_index, arg) in args.iter().enumerate() {
            let arg_register = arg_index as u8 + 1; // self is in register 0, args start from 1
            let arg_node = ctx.node_with_span(*arg);

            // Get the LHS node for default arguments
            let arg_node = match &arg_node.node {
                Node::Assign { target, .. } => ctx.node_with_span(*target),
                _ => arg_node,
            };

            match &arg_node.node {
                Node::Id(_, maybe_type) | Node::Ignored(_, maybe_type) => {
                    if let Some(type_hint) = maybe_type {
                        self.compile_assert_type(arg_register, *type_hint, Some(*arg), ctx)?;
                    }
                }
                Node::Tuple {
                    elements: nested_args,
                    ..
                } => {
                    self.push_span(arg_node, ctx.ast);

                    let (size_op, size_to_check) = args_size_op(nested_args, ctx.ast);
                    self.push_op(size_op, &[arg_register, size_to_check as u8]);
                    self.compile_unpack_nested_args(arg_register, nested_args, ctx)?;

                    self.pop_span();
                }
                unexpected => {
                    return self.error(ErrorKind::UnexpectedNode {
                        expected: "ID or Tuple as function arg".into(),
                        unexpected: unexpected.clone(),
                    });
                }
            }
        }

        let result_register = if allow_implicit_return {
            ResultRegister::Any
        } else {
            ResultRegister::None
        };

        let block_result = self.compile_block(expressions, ctx.with_register(result_register))?;

        if let Some(block_register) = block_result.register {
            if !self.frame().last_node_was_return {
                if !is_generator {
                    self.compile_check_output_type(
                        block_register,
                        expressions.last().copied(),
                        ctx,
                    )?;
                }
                self.push_op_without_span(Op::Return, &[block_register]);
            }
            if block_result.is_temporary {
                self.pop_register()?;
            }
        } else {
            let register = self.push_register()?;
            self.push_op(Op::SetNull, &[register]);
            if !is_generator {
                self.compile_check_output_type(register, expressions.last().copied(), ctx)?;
            }
            self.push_op_without_span(Op::Return, &[register]);
            self.pop_register()?;
        }

        let frame = self.frame_stack.pop().unwrap();
        self.bytes[register_count_byte_index] = frame.registers_used();

        Ok(())
    }

    fn compile_return(
        &mut self,
        expression: Option<AstIndex>,
        return_node: AstIndex,
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        use Op::*;

        let check_return_type = !self.frame().is_generator;

        let result = if let Some(expression) = expression {
            let expression_result = self.compile_node(expression, ctx.with_any_register())?;
            let expression_register = expression_result.unwrap(self)?;
            if check_return_type {
                self.compile_check_output_type(expression_register, Some(return_node), ctx)?;
            }

            match ctx.result_register {
                ResultRegister::Any => {
                    self.push_op(Return, &[expression_register]);
                    expression_result
                }
                ResultRegister::Fixed(result) => {
                    self.push_op(Copy, &[result, expression_register]);
                    self.push_op(Return, &[result]);
                    if expression_result.is_temporary {
                        self.pop_register()?;
                    }
                    CompileNodeOutput::with_assigned(result)
                }
                ResultRegister::None => {
                    self.push_op(Return, &[expression_register]);
                    if expression_result.is_temporary {
                        self.pop_register()?;
                    }
                    CompileNodeOutput::none()
                }
            }
        } else {
            let result = self.assign_result_register(ctx)?;
            match result.register {
                Some(result_register) => {
                    self.push_op(SetNull, &[result_register]);
                    if check_return_type {
                        self.compile_check_output_type(result_register, None, ctx)?;
                    }
                    self.push_op(Return, &[result_register]);
                }
                None => {
                    let register = self.push_register()?;
                    self.push_op(SetNull, &[register]);
                    if check_return_type {
                        self.compile_check_output_type(register, None, ctx)?;
                    }
                    self.push_op(Return, &[register]);
                    self.pop_register()?;
                }
            }
            result
        };

        Ok(result)
    }

    fn compile_yield(
        &mut self,
        expression: AstIndex,
        yield_node: AstIndex,
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        let result = self.assign_result_register(ctx)?;

        let expression_result = self.compile_node(expression, ctx.with_any_register())?;
        let expression_register = expression_result.unwrap(self)?;

        self.compile_check_output_type(expression_register, Some(yield_node), ctx)?;
        self.push_op(Op::Yield, &[expression_register]);

        if let Some(result_register) = result.register {
            self.push_op(Op::Copy, &[result_register, expression_register]);
        }

        if expression_result.is_temporary {
            self.pop_register()?;
        }

        Ok(result)
    }

    fn compile_check_output_type(
        &mut self,
        register: u8,
        span: Option<AstIndex>,
        ctx: CompileNodeContext,
    ) -> Result<()> {
        if let Some(output_type) = self.frame().output_type {
            self.compile_assert_type(register, output_type, span, ctx)?;
        }
        Ok(())
    }

    fn collect_args(&self, args: &[AstIndex], ctx: CompileNodeContext) -> Result<Vec<Arg>> {
        // Collect args for local assignment in the new frame
        // Top-level args need to match the arguments as they appear in the arg list,
        // with `Placeholder`s for ignored values and containers that are being unpacked.
        // Unpacked IDs have registers assigned for them after the top-level IDs.
        //
        // E.g.:
        // Given:
        //   f = |a, (b, (c, d)), _, e|
        // Args should then appear as:
        //   [Local(a), Placeholder, Placeholder, Local(e), Unpacked(b), Unpacked(c), Unpacked(d)]
        //
        // Note that the value stack at runtime will have the function's captures loaded in after
        // the top-level locals and placeholders, and before any unpacked args (e.g. in the example
        // above, captures will be placed after Local(e) and before Unpacked(b)).

        let mut result = Vec::new();
        let mut nested_args = Vec::new();

        for arg in args.iter() {
            // Get the LHS node for default arguments
            let node = match ctx.node(*arg) {
                Node::Assign { target, .. } => ctx.node(*target),
                other => other,
            };

            match node {
                Node::Id(id_index, ..) => result.push(Arg::Local(*id_index)),
                Node::Ignored(..) => result.push(Arg::Placeholder),
                Node::Tuple {
                    elements: nested, ..
                } => {
                    result.push(Arg::Placeholder);
                    nested_args.extend(self.collect_nested_args(nested, ctx.ast)?);
                }
                unexpected => {
                    return self.error(ErrorKind::UnexpectedNode {
                        expected: "ID in function args".into(),
                        unexpected: (*unexpected).clone(),
                    });
                }
            }
        }

        result.extend(nested_args);
        Ok(result)
    }

    fn collect_nested_args(&self, args: &[AstIndex], ast: &Ast) -> Result<Vec<Arg>> {
        let mut result = Vec::new();

        for arg in args.iter() {
            match &ast.node(*arg).node {
                Node::Id(id, ..) => result.push(Arg::Unpacked(*id)),
                Node::Ignored(..) => {}
                Node::Tuple {
                    elements: nested_args,
                    ..
                } => {
                    result.extend(self.collect_nested_args(nested_args, ast)?);
                }
                Node::PackedId(Some(id)) => result.push(Arg::Unpacked(*id)),
                Node::PackedId(None) => {}
                unexpected => {
                    return self.error(ErrorKind::UnexpectedNode {
                        expected: "ID in function args".into(),
                        unexpected: unexpected.clone(),
                    });
                }
            }
        }

        Ok(result)
    }

    fn compile_unpack_nested_args(
        &mut self,
        container_register: u8,
        args: &[AstIndex],
        ctx: CompileNodeContext,
    ) -> Result<()> {
        use Op::*;

        let mut index_from_end = false;

        for (arg_index, arg) in args.iter().enumerate() {
            let is_first_arg = arg_index == 0;
            let is_last_arg = arg_index == args.len() - 1;
            let arg_index = if index_from_end {
                -((args.len() - arg_index) as i8) as u8
            } else {
                arg_index as u8
            };

            match ctx.node(*arg) {
                Node::Ignored(_, Some(type_hint)) => {
                    let temp_register = self.push_register()?;
                    self.push_op(TempIndex, &[temp_register, container_register, arg_index]);
                    self.compile_assert_type(temp_register, *type_hint, Some(*arg), ctx)?;
                    self.pop_register()?; // temp_register
                }
                Node::Id(constant_index, maybe_type) => {
                    let local_register = self.assign_local_register(*constant_index)?;
                    self.push_op(TempIndex, &[local_register, container_register, arg_index]);
                    if let Some(type_hint) = maybe_type {
                        self.compile_assert_type(local_register, *type_hint, Some(*arg), ctx)?;
                    }
                }
                Node::Tuple {
                    elements: nested_args,
                    ..
                } => {
                    let tuple_register = self.push_register()?;
                    self.push_op(TempIndex, &[tuple_register, container_register, arg_index]);
                    let (size_op, size_to_check) = args_size_op(nested_args, ctx.ast);
                    self.push_op(size_op, &[tuple_register, size_to_check as u8]);
                    self.compile_unpack_nested_args(tuple_register, nested_args, ctx)?;
                    self.pop_register()?; // tuple_register
                }
                Node::PackedId(maybe_id) if is_first_arg => {
                    if let Some(id) = maybe_id {
                        // e.g. [first..., x, y]
                        // We want to assign the slice containing all but the last two items to
                        // the given id.
                        let id_register = self.assign_local_register(*id)?;
                        let to_index = -(args.len() as i8 - 1) as u8;
                        self.push_op(SliceTo, &[id_register, container_register, to_index]);
                    }

                    index_from_end = true;
                }
                Node::PackedId(Some(id)) if is_last_arg => {
                    // e.g. [x, y, z, rest...]
                    // We want to assign the slice containing all but the first three items
                    // to the given id.
                    let id_register = self.assign_local_register(*id)?;
                    self.push_op(SliceFrom, &[id_register, container_register, arg_index]);
                }
                Node::PackedId(None) if is_last_arg => {}
                Node::PackedId(_) => {
                    return self.error(ErrorKind::InvalidPositionForArgWithEllipses);
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn compile_block(
        &mut self,
        expressions: &[AstIndex],
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        use Op::SetNull;

        let result = match expressions {
            [] => {
                let result = self.assign_result_register(ctx)?;
                if let Some(result_register) = result.register {
                    self.push_op(SetNull, &[result_register]);
                } else {
                    return self.error(ErrorKind::MissingResultRegister);
                }
                result
            }
            [expression] => self.compile_node(*expression, ctx)?,
            [expressions @ .., last_expression] => {
                for expression in expressions.iter() {
                    self.compile_node(*expression, ctx.compile_for_side_effects())?;
                }

                self.compile_node(*last_expression, ctx)?
            }
        };

        Ok(result)
    }

    fn force_export_assignment(&self) -> bool {
        self.settings.export_top_level_ids && self.frame_stack.len() == 1
    }

    fn local_register_for_assign_target(
        &mut self,
        target: AstIndex,
        ctx: CompileNodeContext,
    ) -> Result<Option<u8>> {
        let result = match ctx.node(target) {
            Node::Id(constant_index, ..) => Some(self.reserve_local_register(*constant_index)?),
            Node::Meta { .. } | Node::Chain(_) | Node::Ignored(..) => None,
            unexpected => {
                return self.error(ErrorKind::UnexpectedNode {
                    expected: "ID".into(),
                    unexpected: unexpected.clone(),
                });
            }
        };

        Ok(result)
    }

    fn compile_assign(
        &mut self,
        target: AstIndex,
        expression: AstIndex,
        export_assignment: bool,
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        use Op::*;

        let local_assign_register = self.local_register_for_assign_target(target, ctx)?;
        let value_result_register = match local_assign_register {
            Some(local) => ResultRegister::Fixed(local),
            None => ResultRegister::Any,
        };

        let value_result =
            self.compile_node(expression, ctx.with_register(value_result_register))?;
        let value_register = value_result.unwrap(self)?;

        let target_node = ctx.node_with_span(target);
        self.push_span(target_node, ctx.ast);

        match &target_node.node {
            Node::Id(id_index, type_hint) => {
                if !value_result.is_temporary {
                    // The ID being assigned to must be committed now that the RHS has been resolved.
                    self.commit_local_register(value_register)?;
                }

                if let Some(type_hint) = type_hint {
                    self.compile_assert_type(value_register, *type_hint, Some(target), ctx)?;
                }

                if export_assignment || self.force_export_assignment() {
                    self.compile_value_export(*id_index, value_register)?;
                }
            }
            Node::Chain(chain) => {
                self.compile_chain(
                    chain,
                    None,
                    Some(value_register),
                    None,
                    ctx.compile_for_side_effects(),
                )?;
            }
            Node::Meta(meta_id, name) => {
                self.compile_meta_export(*meta_id, *name, value_register)?;
            }
            Node::Ignored(_id, type_hint) => {
                if let Some(type_hint) = type_hint {
                    self.compile_assert_type(value_register, *type_hint, Some(target), ctx)?;
                }
            }
            unexpected => {
                return self.error(ErrorKind::UnexpectedNode {
                    expected: "ID or Chain".into(),
                    unexpected: unexpected.clone(),
                });
            }
        };

        let result = match ctx.result_register {
            ResultRegister::Fixed(register) => {
                if register != value_register {
                    self.push_op(Copy, &[register, value_register]);
                }
                CompileNodeOutput::with_assigned(register)
            }
            ResultRegister::Any => value_result,
            ResultRegister::None => CompileNodeOutput::none(),
        };

        self.pop_span();

        Ok(result)
    }

    fn compile_multi_assign(
        &mut self,
        targets: &[AstIndex],
        expression: AstIndex,
        export_assignment: bool,
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        use Op::*;

        if targets.len() >= u8::MAX as usize {
            return self.error(ErrorKind::TooManyAssignmentTargets(targets.len()));
        }

        let result = self.assign_result_register(ctx)?;
        let stack_count = self.stack_count();

        // Reserve any assignment registers for IDs on the LHS before compiling the RHS
        let target_registers = targets
            .iter()
            .map(|target| self.local_register_for_assign_target(*target, ctx))
            .collect::<Result<Vec<_>>>()?;

        let rhs_node = ctx.node_with_span(expression);
        let rhs_is_temp_tuple = matches!(rhs_node.node, Node::TempTuple(_));
        let rhs = self.compile_node(expression, ctx.with_any_register())?;
        let rhs_register = rhs.unwrap(self)?;

        // If the result is needed then prepare the creation of a tuple
        if result.register.is_some() {
            self.push_op(SequenceStart, &[targets.len() as u8]);
        }

        // If the RHS is a single value then convert it into an iterator
        let iter_register = if rhs_is_temp_tuple {
            rhs_register
        } else {
            let iter_register = if rhs.is_temporary {
                rhs_register
            } else {
                self.push_register()?
            };
            self.push_op(MakeIterator, &[iter_register, rhs_register]);
            iter_register
        };

        for (i, (target, target_register)) in
            targets.iter().zip(target_registers.iter()).enumerate()
        {
            match ctx.node(*target) {
                Node::Id(id_index, type_hint) => {
                    let target_register =
                        target_register.expect("Missing target register for assignment");
                    if rhs_is_temp_tuple {
                        self.push_op(TempIndex, &[target_register, iter_register, i as u8]);
                    } else {
                        self.push_op(IterUnpack, &[target_register, iter_register]);
                    }
                    // The register was reserved before the RHS was compiled, and now it
                    // needs to be committed.
                    self.commit_local_register(target_register)?;

                    if let Some(type_hint) = type_hint {
                        self.compile_assert_type(target_register, *type_hint, Some(*target), ctx)?;
                    }

                    // Multi-assignments typically aren't exported, but exporting
                    // assignments might be forced, e.g. in REPL mode.
                    if export_assignment || self.force_export_assignment() {
                        self.compile_value_export(*id_index, target_register)?;
                    }

                    if result.register.is_some() {
                        self.push_op(SequencePush, &[target_register]);
                    }
                }
                Node::Chain(chain) => {
                    let value_register = self.push_register()?;

                    if rhs_is_temp_tuple {
                        self.push_op(TempIndex, &[value_register, iter_register, i as u8]);
                    } else {
                        self.push_op(IterUnpack, &[value_register, iter_register]);
                    }

                    let chain_context = ctx.compile_for_side_effects();
                    self.compile_chain(chain, None, Some(value_register), None, chain_context)?;

                    if result.register.is_some() {
                        self.push_op(SequencePush, &[value_register]);
                    }

                    self.pop_register()?; // value_register
                }
                Node::Ignored(_id, type_hint) => {
                    if result.register.is_some() || type_hint.is_some() {
                        let value_register = self.push_register()?;

                        if rhs_is_temp_tuple {
                            self.push_op(TempIndex, &[value_register, iter_register, i as u8]);
                        } else {
                            self.push_op(IterUnpack, &[value_register, iter_register]);
                        }

                        if let Some(type_hint) = type_hint {
                            self.compile_assert_type(
                                value_register,
                                *type_hint,
                                Some(*target),
                                ctx,
                            )?;
                        }

                        if result.register.is_some() {
                            self.push_op(SequencePush, &[value_register]);
                        }

                        self.pop_register()?; // value_register
                    } else if !rhs_is_temp_tuple {
                        // If the RHS is an iterator then we need to move it along
                        self.push_op(IterNextQuiet, &[iter_register, 0, 0]);
                    }
                }
                unexpected => {
                    return self.error(ErrorKind::UnexpectedNode {
                        expected: "ID or Chain".into(),
                        unexpected: unexpected.clone(),
                    });
                }
            };
        }

        if let Some(result_register) = result.register {
            self.push_op(SequenceToTuple, &[result_register]);
        }

        self.truncate_register_stack(stack_count)?;

        Ok(result)
    }

    fn compile_load_id(
        &mut self,
        id: ConstantIndex,
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        let result = if let Some(local_register) = self.frame().get_local_assigned_register(id) {
            match ctx.result_register {
                ResultRegister::None => CompileNodeOutput::none(),
                ResultRegister::Any => CompileNodeOutput::with_assigned(local_register),
                ResultRegister::Fixed(register) => {
                    self.push_op(Op::Copy, &[register, local_register]);
                    CompileNodeOutput::with_assigned(register)
                }
            }
        } else {
            let result = self.assign_result_register(ctx)?;
            if let Some(result_register) = result.register {
                self.compile_load_non_local(result_register, id);
                result
            } else {
                let register = self.push_register()?;
                self.compile_load_non_local(register, id);
                CompileNodeOutput::with_temporary(register)
            }
        };

        Ok(result)
    }

    // Compiles a type check using the AssertType instruction
    //
    // If the type check fails then an exception will be thrown.
    //
    // If the `enable_type_checks` flag is disabled then no instructions are emitted.
    //
    // See also: compile_check_type
    fn compile_assert_type(
        &mut self,
        value_register: u8,
        type_hint: AstIndex,
        span: Option<AstIndex>, // The assertion should be made using this node's span
        ctx: CompileNodeContext,
    ) -> Result<()> {
        let type_node = ctx.node_with_span(type_hint);
        match &type_node.node {
            Node::Type {
                type_index,
                allow_null,
            } => {
                if self.settings.enable_type_checks {
                    if let Some(span_node_index) = span {
                        let span_node = ctx.node_with_span(span_node_index);
                        self.push_span(span_node, ctx.ast);
                    }

                    let op = if *allow_null {
                        Op::AssertOptionalType
                    } else {
                        Op::AssertType
                    };
                    self.push_op(op, &[value_register]);
                    self.push_var_u32((*type_index).into());

                    if span.is_some() {
                        self.pop_span();
                    }
                }
                Ok(())
            }
            unexpected => self.error(ErrorKind::UnexpectedNode {
                expected: "Type".into(),
                unexpected: unexpected.clone(),
            }),
        }
    }

    // Compiles a type check using the CheckType instruction
    //
    // Returns the jump placeholder for a failed type check; the caller needs to update the
    // placeholder with the offset to the jump target.
    //
    // This is used for type checks that conditionally affect logic flow,
    // and are therefore emitted without checking the `enable_type_checks` flag.
    //
    // See also: compile_assert_type
    fn compile_check_type(
        &mut self,
        value_register: u8,
        type_hint: AstIndex,
        ctx: CompileNodeContext,
    ) -> Result<usize> {
        let type_node = ctx.node_with_span(type_hint);
        match &type_node.node {
            Node::Type {
                type_index,
                allow_null,
            } => {
                self.push_span(type_node, ctx.ast);

                let op = if *allow_null {
                    Op::CheckOptionalType
                } else {
                    Op::CheckType
                };
                self.push_op(op, &[value_register]);
                self.push_var_u32((*type_index).into());

                let jump_placeholder = self.push_offset_placeholder();
                self.pop_span();
                Ok(jump_placeholder)
            }
            unexpected => self.error(ErrorKind::UnexpectedNode {
                expected: "Type".into(),
                unexpected: unexpected.clone(),
            }),
        }
    }

    fn compile_value_export(&mut self, id: ConstantIndex, value_register: u8) -> Result<()> {
        let id_register = self.push_register()?;
        self.compile_load_string_constant(id_register, id);
        self.push_op(Op::ExportValue, &[id_register, value_register]);
        self.pop_register()?;
        self.frame_mut().add_to_exported_ids(id);
        Ok(())
    }

    fn compile_meta_export(
        &mut self,
        meta_id: MetaKeyId,
        name: Option<ConstantIndex>,
        value_register: u8,
    ) -> Result<()> {
        if let Some(name) = name {
            let name_register = self.push_register()?;
            self.compile_load_string_constant(name_register, name);
            self.push_op_without_span(
                Op::MetaExportNamed,
                &[meta_id as u8, name_register, value_register],
            );
            self.pop_register()?;
        } else {
            self.push_op(Op::MetaExport, &[meta_id as u8, value_register]);
        }
        Ok(())
    }

    fn compile_load_string_constant(&mut self, result_register: u8, index: ConstantIndex) {
        self.compile_constant_op(result_register, index, Op::LoadString);
    }

    fn compile_load_non_local(&mut self, result_register: u8, id: ConstantIndex) {
        self.compile_constant_op(result_register, id, Op::LoadNonLocal);
    }

    fn compile_constant_op(&mut self, result_register: u8, id: ConstantIndex, op: Op) {
        self.push_op(op, &[result_register]);
        self.push_var_u32(id.into());
    }

    fn push_var_u32(&mut self, mut n: u32) {
        loop {
            let mut byte = (n & 0x7f) as u8;

            n >>= 7;

            if n != 0 {
                byte |= 0x80;
            }

            self.bytes.push(byte);

            if n == 0 {
                break;
            }
        }
    }

    fn compile_import(
        &mut self,
        from: &[AstIndex],
        items: &[ImportItem],
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        use Op::*;

        let result = self.assign_result_register(ctx)?;
        let stack_count = self.stack_count();

        let wildcard_import = items.is_empty();

        let mut imported = vec![];

        if from.is_empty() {
            for item in items.iter() {
                let maybe_as = item.name.and_then(|name| match ctx.node(name) {
                    Node::Id(id, ..) => Some(*id),
                    _ => None,
                });

                match ctx.node(item.item) {
                    Node::Id(import_id, ..) => {
                        let import_register = if result.register.is_some() {
                            let import_register = if let Some(name) = maybe_as {
                                self.assign_local_register(name)?
                            } else {
                                // The result of the import expression is being assigned,
                                // so import the item into a temporary register.
                                self.push_register()?
                            };

                            self.compile_import_item(
                                import_register,
                                item.item,
                                wildcard_import,
                                ctx,
                            )?;

                            if result.register.is_some() {
                                imported.push(import_register);
                            }

                            import_register
                        } else {
                            // Reserve a local for the imported item.
                            // The register must only be reserved for now otherwise it'll show up in
                            // the import search.
                            let local_id = maybe_as.unwrap_or(*import_id);

                            let import_register = self.reserve_local_register(local_id)?;
                            self.compile_import_item(
                                import_register,
                                item.item,
                                wildcard_import,
                                ctx,
                            )?;

                            // Commit the register now that the import is complete
                            self.commit_local_register(import_register)?;
                            import_register
                        };

                        // Should we export the imported ID?
                        if self.settings.export_top_level_ids && self.frame_stack.len() == 1 {
                            self.compile_value_export(*import_id, import_register)?;
                        }
                    }
                    Node::Str(_) => {
                        let import_register = if let Some(Node::Id(name, ..)) =
                            item.name.map(|name| ctx.node(name))
                        {
                            self.assign_local_register(*name)?
                        } else {
                            self.push_register()?
                        };
                        self.compile_import_item(import_register, item.item, wildcard_import, ctx)?;

                        if result.register.is_some() {
                            imported.push(import_register);
                        }
                    }
                    unexpected => {
                        return self.error(ErrorKind::UnexpectedNode {
                            expected: "import ID".into(),
                            unexpected: unexpected.clone(),
                        });
                    }
                };
            }
        } else {
            let from_register = self.push_register()?;
            self.compile_from(from_register, from, wildcard_import, ctx)?;

            if wildcard_import {
                if self.settings.export_top_level_ids && self.frame_stack.len() == 1 {
                    self.compile_export_iterable(from_register)?;
                }
                imported.push(from_register);
            } else {
                for item in items.iter() {
                    let maybe_as = item.name.and_then(|name| match ctx.node(name) {
                        Node::Id(id, ..) => Some(*id),
                        _ => None,
                    });

                    match ctx.node(item.item) {
                        Node::Id(import_id, ..) => {
                            let import_register = if let Some(name) = maybe_as {
                                // 'import as' has been used, so assign a register for the given name
                                self.assign_local_register(name)?
                            } else if result.register.is_some() {
                                // The result of the import is being assigned,
                                // so import the item into a temporary register.
                                self.push_register()?
                            } else {
                                // Assign the leaf item to a local with a matching name.
                                self.assign_local_register(*import_id)?
                            };

                            // Access the item from from_register
                            self.compile_access_id(import_register, from_register, *import_id);

                            if result.register.is_some() {
                                imported.push(import_register);
                            }

                            // Should we export the imported ID?
                            if self.settings.export_top_level_ids && self.frame_stack.len() == 1 {
                                self.compile_value_export(*import_id, import_register)?;
                            }
                        }
                        Node::Str(string) => {
                            let import_register = if let Some(name) = maybe_as {
                                self.assign_local_register(name)?
                            } else {
                                self.push_register()?
                            };

                            // Access the item from `from_register`, incrementally accessing any
                            // nested items
                            self.compile_access_string(
                                import_register,
                                from_register,
                                &string.contents,
                                ctx,
                            )?;

                            if result.register.is_some() {
                                imported.push(import_register);
                            }
                        }
                        unexpected => {
                            return self.error(ErrorKind::UnexpectedNode {
                                expected: "import ID".into(),
                                unexpected: unexpected.clone(),
                            });
                        }
                    };
                }
            }
        }

        if let Some(result_register) = result.register {
            match imported.as_slice() {
                [] => return self.error(ErrorKind::MissingImportItem),
                [single_item] => self.push_op(Copy, &[result_register, *single_item]),
                _ => {
                    self.push_op(SequenceStart, &[imported.len() as u8]);
                    for item in imported.iter() {
                        self.push_op(SequencePush, &[*item]);
                    }
                    self.push_op(SequenceToTuple, &[result_register]);
                }
            }
        }

        self.truncate_register_stack(stack_count)?;

        Ok(result)
    }

    fn compile_export(
        &mut self,
        expression: AstIndex,
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        let expression_node = ctx.node_with_span(expression);

        match &expression_node.node {
            Node::Assign {
                target, expression, ..
            } => self.compile_assign(*target, *expression, true, ctx),
            Node::MultiAssign {
                targets,
                expression,
                ..
            } => self.compile_multi_assign(targets, *expression, true, ctx),
            // Maps can be exported directly rather than relying on the iterator logic below
            Node::Map { entries, .. } => self.compile_make_map(entries, true, ctx),
            // Other expressions can be evaluated and then assumed to be iterable
            _ => {
                // Evaluate the expression and convert the result into an iterator
                let result = self.compile_node(expression, ctx.with_fixed_register_or_any())?;
                let expression_register = result.unwrap(self)?;
                self.compile_export_iterable(expression_register)?;
                Ok(result)
            }
        }
    }

    fn compile_export_iterable(&mut self, iterable_register: u8) -> Result<()> {
        let stack_count = self.stack_count();
        let iterator_register = self.push_register()?;
        self.push_op(Op::MakeIterator, &[iterator_register, iterable_register]);

        // Consume the expression iterator, exporting each output entry
        let iter_start_ip = self.bytes.len();
        let output_register = self.push_register()?;
        self.push_op(Op::IterNextTemp, &[output_register, iterator_register]);
        let iter_finished_offset = self.push_offset_placeholder();

        self.push_op(Op::ExportEntry, &[output_register]);

        // Jump back to get more iterator output
        self.push_jump_back_op(Op::JumpBack, &[], iter_start_ip);

        // Finished, update the IterNextTemp offset and clean up the temporary registers
        self.update_offset_placeholder(iter_finished_offset)?;
        self.truncate_register_stack(stack_count)?;

        Ok(())
    }

    fn compile_from(
        &mut self,
        result_register: u8,
        path: &[AstIndex],
        wildcard_import: bool,
        ctx: CompileNodeContext,
    ) -> Result<()> {
        match path {
            [] => return self.error(ErrorKind::MissingImportItem),
            [root] => {
                self.compile_import_item(result_register, *root, wildcard_import, ctx)?;
            }
            [root, nested @ ..] => {
                self.compile_import_item(result_register, *root, false, ctx)?;

                for nested_item in nested.iter() {
                    match ctx.node(*nested_item) {
                        Node::Id(id, ..) => {
                            self.compile_access_id(result_register, result_register, *id)
                        }
                        Node::Str(string) => self.compile_access_string(
                            result_register,
                            result_register,
                            &string.contents,
                            ctx,
                        )?,
                        unexpected => {
                            return self.error(ErrorKind::UnexpectedNode {
                                expected: "import ID".into(),
                                unexpected: unexpected.clone(),
                            });
                        }
                    }
                }

                if wildcard_import {
                    self.push_op(Op::ImportAll, &[result_register]);
                }
            }
        }

        Ok(())
    }

    fn compile_import_item(
        &mut self,
        result_register: u8,
        item: AstIndex,
        wildcard_import: bool,
        ctx: CompileNodeContext,
    ) -> Result<()> {
        use Op::{Copy, Import, ImportAll};

        let import_op = if wildcard_import { ImportAll } else { Import };

        match ctx.node(item) {
            Node::Id(id, ..) => {
                if let Some(local_register) = self.frame().get_local_assigned_register(*id) {
                    // The item to be imported is already locally assigned.
                    if local_register != result_register {
                        if wildcard_import {
                            self.push_op(ImportAll, &[local_register]);
                        } else {
                            self.push_op(Copy, &[result_register, local_register]);
                        }
                    }
                    Ok(())
                } else {
                    // If the id isn't a local then it needs to be imported
                    self.compile_load_string_constant(result_register, *id);
                    self.push_op(import_op, &[result_register]);
                    Ok(())
                }
            }
            Node::Str(string) => {
                self.compile_string(&string.contents, ctx.with_fixed_register(result_register))?;
                self.push_op(import_op, &[result_register]);
                Ok(())
            }
            unexpected => self.error(ErrorKind::UnexpectedNode {
                expected: "import ID".into(),
                unexpected: unexpected.clone(),
            }),
        }
    }

    fn compile_try_expression(
        &mut self,
        try_expression: &AstTry,
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        use Op::*;

        let AstTry {
            try_block,
            catch_blocks,
            finally_block,
        } = &try_expression;

        let result = self.assign_result_register(ctx)?;

        let catch_register = self.push_register()?;

        self.push_op(TryStart, &[catch_register]);
        // The catch block start point is defined via an offset from the current byte
        let catch_offset = self.push_offset_placeholder();

        let try_result_register = match result.register {
            Some(result_register) if finally_block.is_none() => {
                ResultRegister::Fixed(result_register)
            }
            _ => ResultRegister::None,
        };

        self.compile_node(*try_block, ctx.with_register(try_result_register))?;

        // Clear the catch point at the end of the try block
        // - if the end of the try block has been reached then the catch block is no longer needed.
        // A dummy byte is appended to TryEnd as required by the bytecode format.
        let dummy_byte = 0;
        self.push_op_without_span(TryEnd, &[dummy_byte]);

        // The try block hasn't thrown, so jump to the finally block
        let mut finally_jump_placeholders = SmallVec::<[usize; 4]>::new();
        self.push_op_without_span(Jump, &[]);
        finally_jump_placeholders.push(self.push_offset_placeholder());

        // Compile the catch block
        self.update_offset_placeholder(catch_offset)?;

        // Clear the catch point at the start of the catch block
        // - if the catch block has been entered, then it needs to be de-registered in case there
        //   are errors thrown in the catch block.
        self.push_op(TryEnd, &[dummy_byte]);

        for (i, catch_block) in catch_blocks.iter().enumerate() {
            let is_last_catch = i == catch_blocks.len() - 1;

            let mut type_check_jump_placeholder = None;

            self.push_span(ctx.node_with_span(catch_block.arg), ctx.ast);
            match ctx.node(catch_block.arg) {
                Node::Id(id, maybe_type) => {
                    if let Some(type_hint) = maybe_type {
                        if !is_last_catch {
                            type_check_jump_placeholder =
                                Some(self.compile_check_type(catch_register, *type_hint, ctx)?);
                        } else {
                            return self.error(ErrorKind::TypeCheckOnLastCatchBlock);
                        }
                    } else if !is_last_catch {
                        return self.error(ErrorKind::MissingTypeCheckOnCatchBlock);
                    }

                    let assigned_catch_register = self.assign_local_register(*id)?;
                    self.push_op(Op::Copy, &[assigned_catch_register, catch_register]);
                }
                Node::Ignored(_id, maybe_type) => {
                    if let Some(type_hint) = maybe_type {
                        if !is_last_catch {
                            type_check_jump_placeholder =
                                Some(self.compile_check_type(catch_register, *type_hint, ctx)?);
                        } else {
                            return self.error(ErrorKind::TypeCheckOnLastCatchBlock);
                        }
                    } else if !is_last_catch {
                        return self.error(ErrorKind::MissingTypeCheckOnCatchBlock);
                    }
                }
                unexpected => {
                    return self.error(ErrorKind::UnexpectedNode {
                        expected: "ID as catch arg".into(),
                        unexpected: unexpected.clone(),
                    });
                }
            };

            self.pop_span(); // catch arg
            self.push_span(ctx.node_with_span(catch_block.block), ctx.ast);

            self.compile_node(catch_block.block, ctx.with_register(try_result_register))?;

            if !is_last_catch {
                // Jump to the finally block at the end of the catch block
                self.push_op_without_span(Jump, &[]);
                finally_jump_placeholders.push(self.push_offset_placeholder());
            }

            if let Some(placeholder) = type_check_jump_placeholder {
                self.update_offset_placeholder(placeholder)?;
            }

            self.pop_span(); // catch block
        }

        self.pop_register()?; // catch_register

        // Compile the finally block
        for placeholder in finally_jump_placeholders {
            self.update_offset_placeholder(placeholder)?;
        }

        if let Some(finally_block) = finally_block {
            // If there's a finally block then the result of the expression is derived from there
            let finally_result_register = match result.register {
                Some(result_register) => ResultRegister::Fixed(result_register),
                _ => ResultRegister::None,
            };
            self.compile_node(*finally_block, ctx.with_register(finally_result_register))
        } else {
            Ok(result)
        }
    }

    fn compile_unary_op(
        &mut self,
        op: AstUnaryOp,
        value: AstIndex,
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        let result = self.assign_result_register(ctx)?;

        let value_result = self.compile_node(value, ctx.with_any_register())?;
        let value_register = value_result.unwrap(self)?;

        if let Some(result_register) = result.register {
            let op_code = match op {
                AstUnaryOp::Negate => Op::Negate,
                AstUnaryOp::Not => Op::Not,
            };

            self.push_op(op_code, &[result_register, value_register]);
        }

        if value_result.is_temporary {
            self.pop_register()?;
        }

        Ok(result)
    }

    fn compile_binary_op(
        &mut self,
        op: AstBinaryOp,
        lhs: AstIndex,
        rhs: AstIndex,
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        use AstBinaryOp::*;

        match op {
            Add | Subtract | Multiply | Divide | Remainder | Power => {
                self.compile_arithmetic_op(op, lhs, rhs, ctx)
            }
            AddAssign | SubtractAssign | MultiplyAssign | DivideAssign | RemainderAssign
            | PowerAssign => self.compile_compound_assignment_op(op, lhs, rhs, ctx),
            Less | LessOrEqual | Greater | GreaterOrEqual | Equal | NotEqual => {
                self.compile_comparison_op(op, lhs, rhs, ctx)
            }
            And | Or => self.compile_logic_op(op, lhs, rhs, ctx),
            Pipe => self.compile_piped_call(lhs, rhs, ctx),
        }
    }

    fn compile_arithmetic_op(
        &mut self,
        op: AstBinaryOp,
        lhs: AstIndex,
        rhs: AstIndex,
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        use AstBinaryOp::*;

        let op = match op {
            Add => Op::Add,
            Subtract => Op::Subtract,
            Multiply => Op::Multiply,
            Divide => Op::Divide,
            Remainder => Op::Remainder,
            Power => Op::Power,
            _ => {
                return self.error(ErrorKind::InvalidBinaryOp {
                    kind: "arithmetic".into(),
                    op,
                });
            }
        };

        let result = self.assign_result_register(ctx)?;

        if let Some(result_register) = result.register {
            let lhs = self.compile_node(lhs, ctx.with_any_register())?;
            let lhs_register = lhs.unwrap(self)?;
            let rhs = self.compile_node(rhs, ctx.with_any_register())?;
            let rhs_register = rhs.unwrap(self)?;

            self.push_op(op, &[result_register, lhs_register, rhs_register]);

            if lhs.is_temporary {
                self.pop_register()?;
            }
            if rhs.is_temporary {
                self.pop_register()?;
            }
        } else {
            self.compile_node(lhs, ctx.compile_for_side_effects())?;
            self.compile_node(rhs, ctx.compile_for_side_effects())?;
        };

        Ok(result)
    }

    fn compile_compound_assignment_op(
        &mut self,
        ast_op: AstBinaryOp,
        lhs: AstIndex,
        rhs: AstIndex,
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        use AstBinaryOp::*;

        let op = match ast_op {
            AddAssign => Op::AddAssign,
            SubtractAssign => Op::SubtractAssign,
            MultiplyAssign => Op::MultiplyAssign,
            DivideAssign => Op::DivideAssign,
            RemainderAssign => Op::RemainderAssign,
            PowerAssign => Op::PowerAssign,
            _ => {
                return self.error(ErrorKind::InvalidBinaryOp {
                    kind: "compound assignment".into(),
                    op: ast_op,
                });
            }
        };

        let result = self.assign_result_register(ctx)?;

        let rhs = self.compile_node(rhs, ctx.with_any_register())?;
        let rhs_register = rhs.unwrap(self)?;

        let lhs_node = ctx.node(lhs);
        if let Node::Chain(chain_node) = lhs_node {
            // Place the chain's result the result register
            // e.g. `x[0] += 1` - The new value of x[0] should end up in the result register
            self.compile_chain(
                chain_node,
                None,
                Some(rhs_register),
                Some(op),
                ctx.with_fixed_register_or_none(result.register),
            )?;
        } else {
            let lhs = self.compile_node(lhs, ctx.with_any_register())?;
            let lhs_register = lhs.unwrap(self)?;

            self.push_op(op, &[lhs_register, rhs_register]);

            // If the LHS is a top-level ID and the export flag is enabled, then export the result
            if let Node::Id(id, ..) = lhs_node {
                if self.settings.export_top_level_ids && self.frame_stack.len() == 1 {
                    self.compile_value_export(*id, lhs_register)?;
                }
            }

            // If there's a result register, then copy the result into it
            if let Some(result_register) = result.register {
                self.push_op(Op::Copy, &[result_register, lhs_register]);
            }

            if lhs.is_temporary {
                self.pop_register()?;
            }
        };

        if rhs.is_temporary {
            self.pop_register()?;
        }

        Ok(result)
    }

    fn compile_comparison_op(
        &mut self,
        ast_op: AstBinaryOp,
        lhs: AstIndex,
        rhs: AstIndex,
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        use AstBinaryOp::*;

        let get_comparision_op = |ast_op| {
            Ok(match ast_op {
                Less => Op::Less,
                LessOrEqual => Op::LessOrEqual,
                Greater => Op::Greater,
                GreaterOrEqual => Op::GreaterOrEqual,
                Equal => Op::Equal,
                NotEqual => Op::NotEqual,
                _ => {
                    return Err(ErrorKind::InvalidBinaryOp {
                        kind: "comparison".into(),
                        op: ast_op,
                    });
                }
            })
        };

        let result = self.assign_result_register(ctx)?;
        let stack_count = self.stack_count();

        // Use the result register for comparisons, or a temporary
        let comparison_register = result.register.map_or_else(|| self.push_register(), Ok)?;

        let mut jump_offsets = Vec::new();

        let mut lhs_register = self
            .compile_node(lhs, ctx.with_any_register())?
            .unwrap(self)?;
        let mut rhs = rhs;
        let mut ast_op = ast_op;

        while let Node::BinaryOp {
            op: rhs_ast_op,
            lhs: rhs_lhs,
            rhs: rhs_rhs,
        } = ctx.node(rhs)
        {
            match rhs_ast_op {
                Less | LessOrEqual | Greater | GreaterOrEqual | Equal | NotEqual => {
                    // If the rhs is also a comparison, then chain the operations.
                    // e.g.
                    //   `a < b < c`
                    // needs to become equivalent to:
                    //   `(a < b) and (b < c)`
                    // To achieve this:
                    //   1. Use the lhs of the rhs as the rhs of the current operation.
                    //   2. Use the temp value as the lhs for the current operation.
                    //   3. Chain the two comparisons together with an And.

                    let rhs_lhs_register = self
                        .compile_node(*rhs_lhs, ctx.with_any_register())?
                        .unwrap(self)?;

                    // Place the lhs comparison result in the comparison_register
                    let op = get_comparision_op(ast_op).map_err(|e| self.make_error(e))?;
                    self.push_op(op, &[comparison_register, lhs_register, rhs_lhs_register]);

                    // Skip evaluating the rhs if the lhs result is false
                    self.push_op(Op::JumpIfFalse, &[comparison_register]);
                    jump_offsets.push(self.push_offset_placeholder());

                    lhs_register = rhs_lhs_register;
                    rhs = *rhs_rhs;
                    ast_op = *rhs_ast_op;
                }
                _ => break,
            }
        }

        // Compile the rhs for the final rhs in the comparison chain
        let rhs_register = self
            .compile_node(rhs, ctx.with_any_register())?
            .unwrap(self)?;

        // We only need to perform the final comparison if there's a result register
        if let Some(result_register) = result.register {
            let op = get_comparision_op(ast_op).map_err(|e| self.make_error(e))?;
            self.push_op(op, &[result_register, lhs_register, rhs_register]);
        }

        for jump_offset in jump_offsets.iter() {
            self.update_offset_placeholder(*jump_offset)?;
        }

        self.truncate_register_stack(stack_count)?;

        Ok(result)
    }

    fn compile_logic_op(
        &mut self,
        op: AstBinaryOp,
        lhs: AstIndex,
        rhs: AstIndex,
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        let result = self.assign_result_register(ctx)?;

        // A register is needed to perform the jump,
        // so if there's no result register use a temporary register
        let register = result.register.map_or_else(|| self.push_register(), Ok)?;
        self.compile_node(lhs, ctx.with_fixed_register(register))?;

        let jump_op = match op {
            AstBinaryOp::And => Op::JumpIfFalse,
            AstBinaryOp::Or => Op::JumpIfTrue,
            _ => unreachable!(),
        };

        self.push_op(jump_op, &[register]);

        // If the lhs caused a jump then that's the result, otherwise the result is the rhs
        self.compile_node_with_jump_offset(rhs, ctx.with_fixed_register(register))?;

        if result.register.is_none() {
            self.pop_register()?;
        }

        Ok(result)
    }

    fn compile_string(
        &mut self,
        contents: &StringContents,
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        let result = self.assign_result_register(ctx)?;

        match contents {
            StringContents::Raw {
                constant: constant_index,
                ..
            }
            | StringContents::Literal(constant_index) => {
                if let Some(result_register) = result.register {
                    self.compile_load_string_constant(result_register, *constant_index);
                }
            }
            StringContents::Interpolated(nodes) => {
                let size_hint = nodes.iter().fold(0, |result, node| {
                    match node {
                        StringNode::Literal(constant_index) => {
                            result + ctx.ast.constants().get_str(*constant_index).len()
                        }
                        StringNode::Expression {
                            format:
                                StringFormatOptions {
                                    min_width: Some(min_width),
                                    ..
                                },
                            ..
                        } => result + *min_width as usize,
                        StringNode::Expression { .. } => {
                            // Q. Why use '1' here?
                            // A. The expression can result in a displayed string of any length,
                            //    We can make an assumption that the expression will almost always
                            //    produce at least 1 character to display, but it's unhealthy to
                            //    over-allocate, so for now let's leave it at that.
                            result + 1
                        }
                    }
                });

                match nodes.as_slice() {
                    [] => return self.error(ErrorKind::MissingStringNodes),
                    [StringNode::Literal(constant_index)] => {
                        if let Some(result_register) = result.register {
                            self.compile_load_string_constant(result_register, *constant_index);
                        }
                    }
                    _ => {
                        if result.register.is_some() {
                            self.push_op(Op::StringStart, &[]);
                            // Limit the size hint to u32::MAX, u64 size hinting can be added later if
                            // it would be useful in practice.
                            self.push_var_u32(size_hint as u32);
                        }

                        for node in nodes.iter() {
                            match node {
                                StringNode::Literal(constant_index) => {
                                    if result.register.is_some() {
                                        let node_register = self.push_register()?;

                                        self.compile_load_string_constant(
                                            node_register,
                                            *constant_index,
                                        );
                                        self.push_op_without_span(
                                            Op::StringPush,
                                            &[node_register, StringFormatFlags::default().into()],
                                        );

                                        self.pop_register()?;
                                    }
                                }
                                StringNode::Expression { expression, format } => {
                                    if result.register.is_some() {
                                        let expression_result = self
                                            .compile_node(*expression, ctx.with_any_register())?;

                                        let format_flags = StringFormatFlags::from(*format);
                                        self.push_op_without_span(
                                            Op::StringPush,
                                            &[expression_result.unwrap(self)?, format_flags.into()],
                                        );
                                        if let Some(min_width) = format.min_width {
                                            self.push_var_u32(min_width);
                                        }
                                        if let Some(precision) = format.precision {
                                            self.push_var_u32(precision);
                                        }
                                        if let Some(fill_constant) = format.fill_character {
                                            self.push_var_u32(fill_constant.into());
                                        }
                                        if let Some(style) = format.representation {
                                            self.bytes.push(style as u8);
                                        }

                                        if expression_result.is_temporary {
                                            self.pop_register()?;
                                        }
                                    } else {
                                        // Compile the expression even though we don't need the
                                        // result, so that side-effects can take place.
                                        self.compile_node(
                                            *expression,
                                            ctx.compile_for_side_effects(),
                                        )?;
                                    }
                                }
                            }
                        }

                        if let Some(result_register) = result.register {
                            self.push_op(Op::StringFinish, &[result_register]);
                        }
                    }
                }
            }
        }

        Ok(result)
    }

    fn compile_make_temp_tuple(
        &mut self,
        elements: &[AstIndex],
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        let result = self.assign_result_register(ctx)?;

        if let Some(result_register) = result.register {
            for element in elements.iter() {
                let element_register = self.push_register()?;
                self.compile_node(*element, ctx.with_fixed_register(element_register))?;
            }

            let start_register = self.peek_register(elements.len() - 1)?;

            self.push_op(
                Op::MakeTempTuple,
                &[result_register, start_register, elements.len() as u8],
            );

            // If we're making a temp tuple then the registers need to be kept around,
            // and they should be removed by the caller.
        } else {
            // Compile the element nodes for side-effects
            for element in elements.iter() {
                self.compile_node(*element, ctx.compile_for_side_effects())?;
            }
        };

        Ok(result)
    }

    fn compile_make_sequence(
        &mut self,
        elements: &[AstIndex],
        finish_op: Op,
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        use Op::*;

        let result = self.assign_result_register(ctx)?;

        if let Some(result_register) = result.register {
            let Ok(size_hint) = u32::try_from(elements.len()) else {
                return self.error(ErrorKind::TooManyContainerEntries(elements.len()));
            };

            self.push_op(SequenceStart, &[]);
            self.push_var_u32(size_hint);

            match elements {
                [] => {}
                [single_element] => {
                    let element = self.compile_node(*single_element, ctx.with_any_register())?;
                    self.push_op_without_span(SequencePush, &[element.unwrap(self)?]);
                    if element.is_temporary {
                        self.pop_register()?;
                    }
                }
                _ => {
                    let max_batch_size = self.frame().available_registers_count() as usize;
                    for elements_batch in elements.chunks(max_batch_size) {
                        let stack_count = self.stack_count();
                        let start_register = self.frame().next_temporary_register();

                        for element_node in elements_batch {
                            let element_register = self.push_register()?;
                            self.compile_node(
                                *element_node,
                                ctx.with_fixed_register(element_register),
                            )?;
                        }

                        self.push_op_without_span(
                            SequencePushN,
                            &[start_register, elements_batch.len() as u8],
                        );

                        self.truncate_register_stack(stack_count)?;
                    }
                }
            }

            // Now that the elements have been added to the sequence builder,
            // add the finishing op.
            self.push_op(finish_op, &[result_register]);
        } else {
            // Compile the element nodes for side-effects
            for element_node in elements.iter() {
                self.compile_node(*element_node, ctx.compile_for_side_effects())?;
            }
        };

        Ok(result)
    }

    fn compile_make_map(
        &mut self,
        entries: &[AstIndex],
        export_entries: bool,
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        let result = self.assign_result_register(ctx)?;

        // Create the map with an appropriate size hint
        if let Some(result_register) = result.register {
            let Ok(size_hint) = u32::try_from(entries.len()) else {
                return self.error(ErrorKind::TooManyContainerEntries(entries.len()));
            };
            self.push_op(Op::MakeMap, &[result_register]);
            self.push_var_u32(size_hint);
        }

        // Process the map's entries
        if result.register.is_some() || export_entries {
            for entry in entries.iter() {
                let (key, value) = match ctx.node(*entry) {
                    Node::MapEntry(key, value) => {
                        let result = match ctx.node(*key) {
                            Node::Id(id, _) if export_entries => {
                                // The value is being exported, and should be made available in scope
                                let value_register = self.reserve_local_register(*id)?;
                                let value = *value;
                                let result = self
                                    .compile_node(value, ctx.with_fixed_register(value_register))?;
                                // Commit the register now that the value has been compiled.
                                self.commit_local_register(value_register)?;
                                result
                            }
                            _ => {
                                // A regular entry
                                let value_node = *value;
                                self.compile_node(value_node, ctx.with_any_register())?
                            }
                        };
                        (*key, result)
                    }
                    Node::Id(key, _) => {
                        // An ID key without a value, a value with matching ID should be available
                        let value = match self.frame().get_local_assigned_register(*key) {
                            Some(register) => CompileNodeOutput::with_assigned(register),
                            None => {
                                let register = self.push_register()?;
                                self.compile_load_non_local(register, *key);
                                CompileNodeOutput::with_temporary(register)
                            }
                        };
                        (*entry, value)
                    }
                    _ => return self.error(ErrorKind::MissingValueForMapEntry), // todo - update error
                };

                let value_register = value.unwrap(self)?;

                let key_node = ctx.node(key);
                self.compile_map_insert(
                    value_register,
                    key_node,
                    result.register,
                    export_entries,
                    ctx,
                )?;

                if value.is_temporary {
                    self.pop_register()?;
                }
            }
        } else {
            // The map is unused, but the entry values should be compiled for side-effects
            for entry in entries.iter() {
                self.compile_node(*entry, ctx.compile_for_side_effects())?;
            }
        }

        Ok(result)
    }

    fn compile_function(
        &mut self,
        function: &Function,
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        use Op::*;

        let result = self.assign_result_register(ctx)?;

        let Node::FunctionArgs {
            args,
            variadic,
            output_type,
        } = &ctx.node(function.args)
        else {
            return self.error(ErrorKind::UnexpectedNode {
                expected: "FunctionArgs".into(),
                unexpected: ctx.node(function.args).clone(),
            });
        };

        let Ok(arg_count) = u8::try_from(args.len()) else {
            return self.error(ErrorKind::FunctionPropertyLimit {
                property: "args".into(),
                amount: args.len(),
            });
        };

        // Gather any default value expressions for optional args.
        // The number of optional args is needed now, although the expressions get compiled
        // below, after the function itself is compiled.
        let mut optional_args = AstVec::new();
        for (i, arg) in args.iter().enumerate() {
            let is_last_arg = i == args.len() - 1;
            match ctx.node(*arg) {
                Node::Assign { expression, .. } => {
                    optional_args.push(*expression);
                }
                _ => {
                    if !(optional_args.is_empty() || *variadic && is_last_arg) {
                        return self.error(ErrorKind::ExpectedOptionalArgumentValue);
                    }
                }
            }
        }

        let captures = self
            .frame()
            .captures_for_nested_frame(&function.accessed_non_locals);
        if optional_args.len() + captures.len() > u8::MAX as usize {
            return self.error(ErrorKind::FunctionPropertyLimit {
                property: "captures".into(),
                amount: optional_args.len() + captures.len(),
            });
        }

        let arg_is_unpacked_tuple = match args.as_slice() {
            &[single_arg] => matches!(ctx.node(single_arg), Node::Tuple { .. }),
            _ => false,
        };
        let non_local_access = function.accessed_non_locals.len() > captures.len();

        let flags = FunctionFlags::new(
            *variadic,
            function.is_generator,
            arg_is_unpacked_tuple,
            non_local_access,
        );

        let function_size_ip = if let Some(result_register) = result.register {
            self.push_op(
                Function,
                &[
                    result_register,
                    arg_count,
                    optional_args.len() as u8,
                    captures.len() as u8,
                    flags.into(),
                ],
            );
            Some(self.push_offset_placeholder())
        } else {
            None
        };

        let local_count = match u8::try_from(function.local_count) {
            Ok(x) => x,
            Err(_) => {
                return self.error(ErrorKind::FunctionPropertyLimit {
                    property: "locals".into(),
                    amount: args.len(),
                });
            }
        };

        let allow_implicit_return = !function.is_generator;
        let body_as_slice = [function.body];
        let function_body = match ctx.node(function.body) {
            Node::Block(expressions) => expressions.as_slice(),
            _ => &body_as_slice,
        };
        self.compile_frame(
            FrameParameters {
                local_count,
                expressions: function_body,
                args,
                captures: &captures,
                allow_implicit_return,
                output_type: *output_type,
                is_generator: function.is_generator,
            },
            ctx,
        )?;

        if let Some(ip) = function_size_ip {
            self.update_offset_placeholder(ip)?;
        }

        if let Some(result_register) = result.register {
            for (i, expression) in optional_args.iter().enumerate() {
                let capture_register = self.push_register()?;
                self.compile_node(*expression, ctx.with_fixed_register(capture_register))?;
                self.push_op(Capture, &[result_register, i as u8, capture_register]);
                self.pop_register()?;
            }

            let optional_arg_count = optional_args.len() as u8;
            for (i, capture) in captures.iter().enumerate() {
                let index = optional_arg_count + i as u8;
                match self
                    .frame()
                    .get_local_assigned_or_reserved_register(*capture)
                {
                    AssignedOrReserved::Assigned(assigned_register) => {
                        self.push_op(Capture, &[result_register, index, assigned_register]);
                    }
                    AssignedOrReserved::Reserved(reserved_register) => {
                        let capture_span = self.span();
                        self.frame_mut()
                            .defer_op_until_register_is_committed(
                                reserved_register,
                                vec![Capture as u8, result_register, index, reserved_register],
                                capture_span,
                            )
                            .map_err(|e| self.make_error(e))?;
                    }
                    AssignedOrReserved::Unassigned => {
                        let capture_register = self.push_register()?;
                        self.compile_load_non_local(capture_register, *capture);
                        self.push_op(Capture, &[result_register, index, capture_register]);
                        self.pop_register()?;
                    }
                }
            }
        } else {
            // The function is unused, but compile the optional arg values to check for errors
            for expression in optional_args.iter() {
                self.compile_node(*expression, ctx.with_any_register())?;
            }
        }

        Ok(result)
    }

    // Compiles a chained expression
    //
    // - `piped_arg_register`: Used when a value is being piped into the chain,
    //   e.g. `f x -> foo.bar 123`, should be equivalent to `foo.bar(f(x), 123)`.
    // - `rhs`: Used when assigning to the result of a chain,
    //   e.g. `foo.bar += 42`, or `foo[123] = bar`
    // - `rhs_op`: If present, then the op should be applied to the result of the chain.
    fn compile_chain(
        &mut self,
        &(ref root_node, mut next_node_index): &(ChainNode, Option<AstIndex>),
        piped_arg_register: Option<u8>,
        rhs: Option<u8>,
        rhs_op: Option<Op>,
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        use Op::*;

        if next_node_index.is_none() {
            return self.error(ErrorKind::MissingNextChainNode);
        }

        // If the result is going into a temporary register then assign it now as the first step.
        let result = self.assign_result_register(ctx)?;

        // Keep track of the registers containing the two previous nodes in the chain.
        // This keeps track of parent containers for evaluating the next node in the chain.
        // For function calls, the two previous nodes are needed (the function's parent container
        // which will be passed as `self` to the function, and the function itself).
        let mut chain_nodes = ChainRegisters::default();

        // Once we're at the end of the chain we'll pop the register stack to the current count.
        let stack_count = self.stack_count();
        let span_stack_count = self.span_stack.len();

        let mut current_node = root_node.clone();

        let mut null_check_jump_placeholders = SmallVec::<[usize; 4]>::new();
        let mut null_check_on_end_node = false;

        // Compile the chain's nodes, except for the last node, which will be compiled separately
        // following the loop.
        while let Some(next) = next_node_index {
            match &current_node {
                ChainNode::Root(root_node) => {
                    if !chain_nodes.is_empty() {
                        return self.error(ErrorKind::OutOfPositionRootNodeInChain);
                    }

                    let root = self.compile_node(*root_node, ctx.with_any_register())?;
                    chain_nodes.push(root.unwrap(self)?, root.is_temporary);
                }
                ChainNode::Id(id, ..) => {
                    // Access by id
                    // e.g. x.foo()
                    //    - x = Root
                    //    - foo = Id
                    //    - () = Call

                    let Some(instance_register) = chain_nodes.previous() else {
                        return self.error(ErrorKind::OutOfPositionChildNodeInChain);
                    };

                    let node_register = match chain_nodes.reuse_oldest() {
                        Some(register) => register,
                        None => self.push_register()?,
                    };
                    chain_nodes.push(node_register, true);

                    self.compile_access_id(node_register, instance_register, *id);
                }
                ChainNode::Str(access_string) => {
                    // Access by string
                    // e.g. x."123"()
                    //    - x = Root
                    //    - "123" = Str
                    //    - () = Call

                    let Some(instance_register) = chain_nodes.previous() else {
                        return self.error(ErrorKind::OutOfPositionChildNodeInChain);
                    };

                    let node_register = match chain_nodes.reuse_oldest() {
                        Some(register) => register,
                        None => self.push_register()?,
                    };
                    chain_nodes.push(node_register, true);

                    self.compile_access_string(
                        node_register,
                        instance_register,
                        &access_string.contents,
                        ctx,
                    )?;
                }
                ChainNode::Index(index_node) => {
                    // Indexing into a value
                    // e.g. foo.bar[123]
                    //    - foo = Root
                    //    - bar = Id
                    //    - [123] = Index, with 123 as index node

                    let Some(instance_register) = chain_nodes.previous() else {
                        return self.error(ErrorKind::OutOfPositionChildNodeInChain);
                    };

                    let node_register = match chain_nodes.reuse_oldest() {
                        Some(register) => register,
                        None => self.push_register()?,
                    };
                    chain_nodes.push(node_register, true);

                    // Compile the index, into the current node register
                    self.compile_node(*index_node, ctx.with_fixed_register(node_register))?;

                    // Run the index operation, place the result in the node register
                    self.push_op(Index, &[node_register, instance_register, node_register]);
                }
                ChainNode::Call { args, .. } => {
                    // Function call on a chain node
                    let (function_register, instance_register) = chain_nodes.previous_two();
                    let Some(function_register) = function_register else {
                        return self.error(ErrorKind::OutOfPositionChildNodeInChain);
                    };

                    let node_register = match chain_nodes.reuse_oldest() {
                        Some(register) => register,
                        None => self.push_register()?,
                    };
                    chain_nodes.push(node_register, true);

                    self.compile_call(
                        function_register,
                        args,
                        None,
                        instance_register,
                        ctx.with_fixed_register(node_register),
                    )?;
                }
                ChainNode::NullCheck => {
                    // `?` null check
                    let Some(check_register) = chain_nodes.previous() else {
                        return self.error(ErrorKind::OutOfPositionChildNodeInChain);
                    };

                    self.push_op(JumpIfNull, &[check_register]);
                    null_check_jump_placeholders.push(self.push_offset_placeholder());
                }
            }

            let next_chain_node = ctx.node_with_span(next);
            self.push_span(next_chain_node, ctx.ast);

            match &next_chain_node.node {
                Node::Chain((node, next)) => {
                    current_node = node.clone();
                    next_node_index = *next;

                    // If the last node in the chain is a null check then break out now,
                    // allowing the final node (before the null check) to be held in `current_node`
                    // for further processing below, after this loop.
                    if let Some(next) = *next {
                        if ctx.node(next) == &Node::Chain((ChainNode::NullCheck, None)) {
                            null_check_on_end_node = true;
                            break;
                        }
                    }
                }
                unexpected => {
                    return self.error(ErrorKind::UnexpectedNode {
                        expected: "a chain node".into(),
                        unexpected: unexpected.clone(),
                    });
                }
            }
        }

        // The chain is complete up until the last node, and now we need to handle:
        //   - accessing and assigning to map entries
        //   - accessing and assigning to list entries
        //   - calling functions
        let end_node = current_node;

        let Some(container_register) = chain_nodes.previous() else {
            return self.error(ErrorKind::MissingChainParentRegister);
        };

        // Where should the final value in the chain be placed?
        let (output_register, output_register_is_temporary) =
            match (result.register, piped_arg_register, rhs_op) {
                // If there's a result register and no piped call, then use the result register
                (Some(register), None, _) => (register, false),
                // If there's a piped call after the chain, or an assignment operation,
                // then place the result of the chain in a temporary register.
                _ => match chain_nodes.reuse_oldest() {
                    Some(register) => (register, false),
                    None => (self.push_register()?, true),
                },
            };

        let string_key = if let ChainNode::Str(access_string) = &end_node {
            self.compile_string(&access_string.contents, ctx.with_any_register())?
        } else {
            CompileNodeOutput::none()
        };

        let index = if let ChainNode::Index(index_node) = end_node {
            self.compile_node(index_node, ctx.with_any_register())?
        } else {
            CompileNodeOutput::none()
        };

        // If rhs_op is Some, then rhs should also be Some
        debug_assert!(rhs_op.is_none() || rhs_op.is_some() && rhs.is_some());
        let simple_assignment = rhs.is_some() && rhs_op.is_none();
        let compound_assignment = rhs.is_some() && rhs_op.is_some();
        let access_end_node = !simple_assignment || null_check_on_end_node;

        // Do we need to access the last node in the lookup chain?
        // - No if it's a simple assignment (without a null check) and the last node is going to be
        //   overwritten.
        // - Otherwise yes, either there's a compound assignment, or a null check,
        //   or the last node is the expression result.
        match &end_node {
            ChainNode::Id(id, ..) if access_end_node => {
                self.compile_access_id(output_register, container_register, *id);
                chain_nodes.push(output_register, false);
            }
            ChainNode::Str(_) if access_end_node => {
                self.push_op(
                    AccessString,
                    &[
                        output_register,
                        container_register,
                        string_key.unwrap(self)?,
                    ],
                );
                chain_nodes.push(output_register, false);
            }
            ChainNode::Index(_) if access_end_node => {
                self.push_op(
                    Index,
                    &[output_register, container_register, index.unwrap(self)?],
                );
                chain_nodes.push(output_register, false);
            }
            ChainNode::Call { args, with_parens } => {
                if simple_assignment {
                    return self.error(ErrorKind::AssigningToATemporaryValue);
                } else if compound_assignment // e.g. `f() += 1 # (valid depending on return type)`
                    // If there's a piped call on the chain result, defer it until below
                    || piped_arg_register.is_none()
                    // Parenthesized calls need to be made now, e.g. `42 -> foo.bar()`
                    || *with_parens
                {
                    let (function_register, instance_register) = chain_nodes.previous_two();

                    let Some(function_register) = function_register else {
                        return self.error(ErrorKind::OutOfPositionChildNodeInChain);
                    };

                    self.compile_call(
                        function_register,
                        args,
                        None,
                        instance_register,
                        ctx.with_fixed_register(output_register),
                    )?;
                    chain_nodes.push(output_register, false);
                }
            }
            _ => {}
        }

        // Is a null check needed on the last node?
        if null_check_on_end_node {
            self.push_op(JumpIfNull, &[output_register]);
            null_check_jump_placeholders.push(self.push_offset_placeholder());
        }

        // Do we need to modify the accessed value?
        if compound_assignment {
            // Compound_assignment can only be true when both rhs and rhs_op have values
            let rhs = rhs.unwrap();
            let rhs_op = rhs_op.unwrap();

            self.push_op(rhs_op, &[output_register, rhs]);
            chain_nodes.push(output_register, false);
        }

        // Do we need to assign a value to the last node in the chain?
        if compound_assignment || simple_assignment {
            let value_register = if simple_assignment {
                rhs.unwrap()
            } else {
                output_register
            };

            match &end_node {
                ChainNode::Id(id, ..) => {
                    self.compile_map_insert(
                        value_register,
                        &Node::Id(*id, None),
                        Some(container_register),
                        false,
                        ctx,
                    )?;
                }
                ChainNode::Str(_) => {
                    let string_key = string_key.unwrap(self)?;
                    self.push_op(MapInsert, &[container_register, string_key, value_register]);
                }
                ChainNode::Index(_) => {
                    let index = index.unwrap(self)?;
                    self.push_op(IndexMut, &[container_register, index, value_register]);
                }
                _ => {}
            }
        }

        // As a final step, do we need to make a piped call to the result of the chain?
        if piped_arg_register.is_some() {
            let piped_call_args = match end_node {
                ChainNode::Call { args, with_parens } if !with_parens => args,
                _ => AstVec::new(),
            };

            let (function_register, parent_register) = chain_nodes.previous_two();
            let function_register = match function_register {
                Some(register) => register,
                None => return self.error(ErrorKind::OutOfPositionChildNodeInChain),
            };

            let call_result = if let Some(result_register) = result.register {
                ResultRegister::Fixed(result_register)
            } else {
                ResultRegister::None
            };

            if output_register_is_temporary && function_register != output_register {
                // The output register is unused and no longer needed, so can be deallocated
                self.pop_register()?;
            }

            self.compile_call(
                function_register,
                &piped_call_args,
                piped_arg_register,
                parent_register,
                ctx.with_register(call_result),
            )?;
        }

        // Now that all chain operations are complete, if `?` null checks were used then the result
        // register needs to be set to null for checks that failed.

        if !null_check_jump_placeholders.is_empty() {
            // First, add a jump to the end of the chain for the success path (all null checks passed).
            self.push_op(Op::Jump, &[]);
            let success_jump_placeholder = self.push_offset_placeholder();

            // Next, update the null check jump offsets, and set the result register to null
            for placeholder in null_check_jump_placeholders {
                self.update_offset_placeholder(placeholder)?;
            }
            if let Some(result_register) = result.register {
                self.push_op(Op::SetNull, &[result_register]);
            }

            // Update the success jump offset, skipping the SetNull op
            self.update_offset_placeholder(success_jump_placeholder)?;
        }

        // Clean up the span and register stacks
        self.span_stack.truncate(span_stack_count);
        self.truncate_register_stack(stack_count)?;

        Ok(result)
    }

    fn compile_map_insert(
        &mut self,
        value_register: u8,
        key: &Node,
        map_register: Option<u8>,
        export_entry: bool,
        ctx: CompileNodeContext,
    ) -> Result<()> {
        use Op::*;

        match key {
            Node::Id(id, ..) => {
                let key_register = self.push_register()?;
                self.compile_load_string_constant(key_register, *id);

                if let Some(map_register) = map_register {
                    self.push_op_without_span(
                        MapInsert,
                        &[map_register, key_register, value_register],
                    );
                }

                if export_entry {
                    self.push_op_without_span(ExportValue, &[key_register, value_register]);
                }

                self.pop_register()?;
            }
            Node::Str(string) => {
                let key_register = self.push_register()?;
                self.compile_string(&string.contents, ctx.with_fixed_register(key_register))?;

                if let Some(map_register) = map_register {
                    self.push_op_without_span(
                        MapInsert,
                        &[map_register, key_register, value_register],
                    );
                }

                if export_entry {
                    self.push_op_without_span(ExportValue, &[key_register, value_register]);
                }

                self.pop_register()?;
            }
            Node::Meta(key, name) => {
                let key = *key as u8;
                if let Some(name) = name {
                    let name_register = self.push_register()?;
                    self.compile_load_string_constant(name_register, *name);

                    if let Some(map_register) = map_register {
                        self.push_op_without_span(
                            MetaInsertNamed,
                            &[map_register, key, name_register, value_register],
                        );
                    }

                    if export_entry {
                        self.push_op_without_span(
                            MetaExportNamed,
                            &[key, name_register, value_register],
                        );
                    }

                    self.pop_register()?;
                } else {
                    if let Some(map_register) = map_register {
                        self.push_op_without_span(MetaInsert, &[map_register, key, value_register]);
                    }

                    if export_entry {
                        self.push_op(MetaExport, &[key, value_register]);
                    }
                }
            }
            unexpected => {
                return self.error(ErrorKind::UnexpectedNode {
                    expected: "a map key".into(),
                    unexpected: unexpected.clone(),
                });
            }
        }

        Ok(())
    }

    fn compile_access_id(&mut self, result: u8, value: u8, key: ConstantIndex) {
        self.push_op(Op::Access, &[result, value]);
        self.push_var_u32(key.into());
    }

    fn compile_access_string(
        &mut self,
        result_register: u8,
        value_register: u8,
        key_string_contents: &StringContents,
        ctx: CompileNodeContext,
    ) -> Result<()> {
        let key_register = self.push_register()?;
        self.compile_string(key_string_contents, ctx.with_fixed_register(key_register))?;
        self.push_op(
            Op::AccessString,
            &[result_register, value_register, key_register],
        );
        self.pop_register()?;
        Ok(())
    }

    // Compiles a node like `f x -> g`, compiling the lhs as the first arg for a call on the rhs
    fn compile_piped_call(
        &mut self,
        lhs: AstIndex,
        rhs: AstIndex,
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        // First things first, if a temporary result register is to be used, assign it now.
        let result = self.assign_result_register(ctx)?;

        // The piped call should either go into the specified register, or it can be ignored
        let call_result_register = if let Some(result_register) = result.register {
            ResultRegister::Fixed(result_register)
        } else {
            ResultRegister::None
        };

        // Next, compile the LHS to produce the value that should be piped into the call
        let piped_value = self.compile_node(lhs, ctx.with_any_register())?;
        let pipe_register = Some(piped_value.unwrap(self)?);

        let rhs_node = ctx.node_with_span(rhs);
        let result = match &rhs_node.node {
            Node::Id(id, ..) => {
                // Compile a call with the piped arg, using the id to access the function
                if let Some(function_register) = self.frame().get_local_assigned_register(*id) {
                    self.compile_call(function_register, &[], pipe_register, None, ctx)
                } else {
                    let call_result_register = if let Some(result_register) = result.register {
                        ResultRegister::Fixed(result_register)
                    } else {
                        ResultRegister::None
                    };

                    let function_register = self.push_register()?;
                    self.compile_load_non_local(function_register, *id);

                    let call_context = ctx.with_register(call_result_register);
                    self.compile_call(function_register, &[], pipe_register, None, call_context)?;

                    self.pop_register()?; // function_register
                    Ok(result)
                }
            }
            Node::Chain(chain_node) => {
                // Compile the chain, passing in the piped call arg, which will either be appended
                // to call args at the end of a chain, or the last node will be turned into a call.
                let call_context = ctx.with_register(call_result_register);
                self.compile_chain(chain_node, pipe_register, None, None, call_context)
            }
            _ => {
                // If the RHS is none of the above, then compile it assuming that the result will
                // be a function.
                let function = self.compile_node(rhs, ctx.with_any_register())?;
                let function_register = function.unwrap(self)?;
                let call_context = ctx.with_register(call_result_register);
                let result =
                    self.compile_call(function_register, &[], pipe_register, None, call_context)?;
                if function.is_temporary {
                    self.pop_register()?;
                }
                Ok(result)
            }
        };

        if piped_value.is_temporary {
            self.pop_register()?;
        }

        result
    }

    fn compile_call(
        &mut self,
        function_register: u8,
        args: &[AstIndex],
        piped_arg: Option<u8>,
        instance: Option<u8>,
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        use Op::*;

        let result = self.assign_result_register(ctx)?;
        let stack_count = self.stack_count();

        let mut arg_count = args.len();

        let frame_base = if let Some(instance) = instance {
            if instance == self.frame().next_temporary_register() - 1 {
                // If the instance is already at the top of the register stack then it can be used
                // directly as the frame base.
                instance
            } else {
                self.push_register()?
            }
        } else {
            // No instance is provided, but the frame base still needs to be present as an empty
            // register.
            self.push_register()?
        };

        let arg_offset = if let Some(piped_arg) = piped_arg {
            arg_count += 1;
            let arg_register = self.push_register()?;
            self.push_op(Copy, &[arg_register, piped_arg]);
            1
        } else {
            0
        };

        let mut packed_arg_indices = AstVec::<u8>::new();
        for (i, arg) in args.iter().enumerate() {
            let arg = if let Node::PackedExpression(packed_arg) = ctx.node(*arg) {
                packed_arg_indices.push(arg_offset + i as u8);
                packed_arg
            } else {
                arg
            };
            let arg_register = self.push_register()?;
            self.compile_node(*arg, ctx.with_fixed_register(arg_register))?;
        }

        // Indices of args that need to be unpacked are placed in the registers following the args
        for index in packed_arg_indices.iter() {
            let register = self.push_register()?;
            self.push_op(SetNumberU8, &[register, *index]);
        }

        let call_result_register = if let Some(result_register) = result.register {
            result_register
        } else {
            // The result isn't needed, so it can be placed in the frame's base register
            // (which isn't needed post-call and will be discarded).
            frame_base
        };

        if let Some(instance_register) = instance {
            self.push_op(
                CallInstance,
                &[
                    call_result_register,
                    function_register,
                    instance_register,
                    frame_base,
                    arg_count as u8,
                    packed_arg_indices.len() as u8,
                ],
            );
        } else {
            self.push_op(
                Call,
                &[
                    call_result_register,
                    function_register,
                    frame_base,
                    arg_count as u8,
                    packed_arg_indices.len() as u8,
                ],
            );
        }

        self.truncate_register_stack(stack_count)?;

        Ok(result)
    }

    fn compile_if(&mut self, ast_if: &AstIf, ctx: CompileNodeContext) -> Result<CompileNodeOutput> {
        use Op::*;

        let AstIf {
            condition,
            then_node,
            else_if_blocks,
            else_node,
            ..
        } = ast_if;

        let result = self.assign_result_register(ctx)?;

        // If
        let condition_register = self.compile_node(*condition, ctx.with_any_register())?;

        self.push_op_without_span(JumpIfFalse, &[condition_register.unwrap(self)?]);
        let condition_jump_ip = self.push_offset_placeholder();

        if condition_register.is_temporary {
            self.pop_register()?;
        }

        let expression_context = ctx.with_register(
            result
                .register
                .map_or(ResultRegister::None, ResultRegister::Fixed),
        );
        self.compile_node(*then_node, expression_context)?;

        let if_jump_ip = {
            if !else_if_blocks.is_empty() || else_node.is_some() || result.register.is_some() {
                self.push_op_without_span(Jump, &[]);
                Some(self.push_offset_placeholder())
            } else {
                None
            }
        };

        // A failing condition for the if jumps to here, at the start of the else if / else blocks
        self.update_offset_placeholder(condition_jump_ip)?;

        // Iterate through the else if blocks and collect their end jump placeholders
        let else_if_jump_ips = else_if_blocks
            .iter()
            .map(|(else_if_condition, else_if_node)| -> Result<usize> {
                let condition = self.compile_node(*else_if_condition, ctx.with_any_register())?;

                self.push_op_without_span(JumpIfFalse, &[condition.unwrap(self)?]);
                let conditon_jump_ip = self.push_offset_placeholder();

                if condition.is_temporary {
                    self.pop_register()?;
                }

                self.compile_node(*else_if_node, expression_context)?;

                self.push_op_without_span(Jump, &[]);
                let else_if_jump_ip = self.push_offset_placeholder();

                self.update_offset_placeholder(conditon_jump_ip)?;

                Ok(else_if_jump_ip)
            })
            .collect::<Result<Vec<_>>>()?;

        // Else - either compile the else block, or set the result to empty
        if let Some(else_node) = else_node {
            self.compile_node(*else_node, expression_context)?;
        } else if let Some(result_register) = result.register {
            self.push_op_without_span(SetNull, &[result_register]);
        }

        // We're at the end, so update the if and else if jump placeholders
        if let Some(if_jump_ip) = if_jump_ip {
            self.update_offset_placeholder(if_jump_ip)?;
        }

        for else_if_jump_ip in else_if_jump_ips.iter() {
            self.update_offset_placeholder(*else_if_jump_ip)?;
        }

        Ok(result)
    }

    fn compile_switch(
        &mut self,
        arms: &[AstIndex],
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        let result = self.assign_result_register(ctx)?;

        let stack_count = self.stack_count();

        let mut result_jump_placeholders = Vec::new();

        let switch_arm_context = ctx.with_register(
            result
                .register
                .map_or(ResultRegister::None, ResultRegister::Fixed),
        );

        let mut last_arm_is_else = false;
        for arm in arms.iter() {
            let arm_node = ctx.node(*arm);
            let Node::SwitchArm {
                condition,
                expression,
            } = arm_node
            else {
                return self.error(ErrorKind::UnexpectedNode {
                    expected: "SwitchArm".into(),
                    unexpected: arm_node.clone(),
                });
            };

            let arm_end_jump_placeholder = if let Some(condition) = condition {
                let condition_register = self.compile_node(*condition, ctx.with_any_register())?;

                self.push_op_without_span(Op::JumpIfFalse, &[condition_register.unwrap(self)?]);

                if condition_register.is_temporary {
                    self.pop_register()?;
                }

                Some(self.push_offset_placeholder())
            } else {
                None
            };

            self.compile_node(*expression, switch_arm_context)?;

            // Add a jump instruction if this anything other than an `else` arm
            if condition.is_some() {
                self.push_op_without_span(Op::Jump, &[]);
                result_jump_placeholders.push(self.push_offset_placeholder())
            }

            if let Some(jump_placeholder) = arm_end_jump_placeholder {
                self.update_offset_placeholder(jump_placeholder)?;
            }

            last_arm_is_else = condition.is_none();
        }

        // Set the result register to null, in case no switch arm is executed
        if let Some(result_register) = result.register {
            // If the last arm is `else`, then setting to Null isn't necessary
            if !last_arm_is_else {
                self.push_op(Op::SetNull, &[result_register]);
            }
        }

        for jump_placeholder in result_jump_placeholders.iter() {
            self.update_offset_placeholder(*jump_placeholder)?;
        }

        self.truncate_register_stack(stack_count)?;

        Ok(result)
    }

    fn compile_match(
        &mut self,
        match_expression: AstIndex,
        arms: &[AstIndex],
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        let result = self.assign_result_register(ctx)?;

        let stack_count = self.stack_count();

        let match_register = self
            .compile_node(match_expression, ctx.with_any_register())?
            .unwrap(self)?;
        let match_len = match ctx.node(match_expression) {
            Node::TempTuple(expressions) => expressions.len(),
            _ => 1,
        };

        // Compile the match arms, collecting their jump offset placeholders
        let mut last_arm_is_else = false;
        let arm_jump_placeholders = arms
            .iter()
            .map(|arm| {
                self.compile_match_arm(
                    result,
                    match_register,
                    match_len,
                    *arm,
                    &mut last_arm_is_else,
                    ctx,
                )
            })
            .collect::<Result<Vec<_>>>()?;

        // Set the result to Null in case there was no matching arm
        if let Some(result_register) = result.register {
            // If the last arm was `else`, then setting to Null isn't necessary
            if !last_arm_is_else {
                self.push_op(Op::SetNull, &[result_register]);
            }
        }

        // Update the arm jump placeholders
        for placeholder in arm_jump_placeholders.iter().flatten() {
            self.update_offset_placeholder(*placeholder)?;
        }

        self.truncate_register_stack(stack_count)?;

        Ok(result)
    }

    fn compile_match_arm(
        &mut self,
        result: CompileNodeOutput,
        match_register: u8,
        match_len: usize,
        arm: AstIndex,
        last_arm_is_else: &mut bool,
        ctx: CompileNodeContext,
    ) -> Result<Option<usize>> {
        let arm_node = ctx.node(arm);
        let Node::MatchArm {
            patterns,
            condition,
            expression,
        } = arm_node
        else {
            return self.error(ErrorKind::UnexpectedNode {
                expected: "MatchArm".into(),
                unexpected: arm_node.clone(),
            });
        };
        let arm_is_else = patterns.is_empty();
        *last_arm_is_else = arm_is_else;

        let mut jumps = MatchJumpPlaceholders::default();

        for (alternative_index, arm_pattern) in patterns.iter().enumerate() {
            let is_last_alternative = alternative_index == patterns.len() - 1;

            jumps.alternative_end.clear();

            let arm_node = ctx.node_with_span(*arm_pattern);
            self.push_span(arm_node, ctx.ast);
            let patterns = match &arm_node.node {
                Node::TempTuple(patterns) => {
                    if patterns.len() != match_len {
                        return self.error(ErrorKind::UnexpectedMatchPatternCount {
                            expected: match_len,
                            unexpected: patterns.len(),
                        });
                    }

                    Some(patterns.clone())
                }
                Node::Tuple {
                    elements: patterns, ..
                } => {
                    if match_len != 1 {
                        return self.error(ErrorKind::UnexpectedMatchPatternCount {
                            expected: match_len,
                            unexpected: 1,
                        });
                    }

                    self.compile_nested_match_arm_patterns(
                        MatchArmParameters {
                            match_register,
                            is_last_alternative,
                            has_last_pattern: true,
                            jumps: &mut jumps,
                        },
                        None, // pattern index
                        patterns,
                        ctx,
                    )?;

                    None
                }
                Node::Ignored(..) => Some(smallvec![*arm_pattern]),
                _ => {
                    if match_len != 1 {
                        return self.error(ErrorKind::UnexpectedMatchPatternCount {
                            expected: match_len,
                            unexpected: 1,
                        });
                    }
                    Some(smallvec![*arm_pattern])
                }
            };

            if let Some(patterns) = patterns {
                // Check that the number of patterns is correct
                self.compile_match_arm_patterns(
                    MatchArmParameters {
                        match_register,
                        is_last_alternative,
                        has_last_pattern: true,
                        jumps: &mut jumps,
                    },
                    patterns.len() > 1, // match_is_container
                    &patterns,
                    ctx,
                )?;
            }

            for jump_placeholder in jumps.alternative_end.iter() {
                self.update_offset_placeholder(*jump_placeholder)?;
            }

            self.pop_span(); // arm node
        }

        // Update the match end jump placeholders before the condition
        for jump_placeholder in jumps.match_end.iter() {
            self.update_offset_placeholder(*jump_placeholder)?;
        }

        // Arm condition, e.g.
        // match foo
        //   x if x > 10 then 99
        if let Some(condition) = condition {
            let condition_register = self.compile_node(*condition, ctx.with_any_register())?;

            self.push_op_without_span(Op::JumpIfFalse, &[condition_register.unwrap(self)?]);
            jumps.arm_end.push(self.push_offset_placeholder());

            if condition_register.is_temporary {
                self.pop_register()?;
            }
        }

        let body_result_register = result
            .register
            .map_or(ResultRegister::None, ResultRegister::Fixed);
        self.compile_node(*expression, ctx.with_register(body_result_register))?;

        // Jump to the end of the match expression, unless this is an `else` arm
        let result_jump_placeholder = if !arm_is_else {
            self.push_op_without_span(Op::Jump, &[]);
            Some(self.push_offset_placeholder())
        } else {
            None
        };

        for jump_placeholder in jumps.arm_end.iter() {
            self.update_offset_placeholder(*jump_placeholder)?;
        }

        Ok(result_jump_placeholder)
    }

    fn compile_match_arm_patterns(
        &mut self,
        params: MatchArmParameters,
        match_is_container: bool,
        arm_patterns: &[AstIndex],
        ctx: CompileNodeContext,
    ) -> Result<()> {
        use Op::*;

        let mut index_from_end = false;

        for (pattern_index, pattern) in arm_patterns.iter().enumerate() {
            let is_first_pattern = pattern_index == 0;
            let is_last_pattern = pattern_index == arm_patterns.len() - 1;
            let pattern_index = if index_from_end {
                -((arm_patterns.len() - pattern_index) as i8)
            } else {
                pattern_index as i8
            };
            let pattern_node = ctx.node_with_span(*pattern);

            match &pattern_node.node {
                Node::Null
                | Node::BoolTrue
                | Node::BoolFalse
                | Node::SmallInt(_)
                | Node::Int(_)
                | Node::Float(_)
                | Node::Str(_)
                | Node::Chain(_) => {
                    let pattern_register = self.push_register()?;
                    self.compile_node(*pattern, ctx.with_fixed_register(pattern_register))?;
                    let comparison = self.push_register()?;

                    if match_is_container {
                        let element = self.push_register()?;
                        self.push_op(
                            TempIndex,
                            &[element, params.match_register, pattern_index as u8],
                        );
                        self.push_op(Equal, &[comparison, pattern_register, element]);
                        self.pop_register()?; // element
                    } else {
                        self.push_op(
                            Equal,
                            &[comparison, pattern_register, params.match_register],
                        );
                    }

                    if params.is_last_alternative {
                        // If there's no match on the last alternative,
                        // then jump to the end of the arm
                        self.push_op(JumpIfFalse, &[comparison]);
                        params.jumps.arm_end.push(self.push_offset_placeholder());
                    } else if params.has_last_pattern && is_last_pattern {
                        // If there's a match with remaining alternative matches,
                        // then jump to the end of the alternatives
                        self.push_op(JumpIfTrue, &[comparison]);
                        params.jumps.match_end.push(self.push_offset_placeholder());
                    } else {
                        // If there's no match but there remaining alternative matches,
                        // then jump to the next alternative
                        self.push_op(JumpIfFalse, &[comparison]);
                        params
                            .jumps
                            .alternative_end
                            .push(self.push_offset_placeholder());
                    }

                    self.pop_register()?; // comparison_register
                    self.pop_register()?; // pattern_register
                }
                Node::Id(id, maybe_type) => {
                    let id_register = self.assign_local_register(*id)?;
                    if match_is_container {
                        self.push_op(
                            TempIndex,
                            &[id_register, params.match_register, pattern_index as u8],
                        );
                    } else {
                        self.push_op(Copy, &[id_register, params.match_register]);
                    }

                    if let Some(type_hint) = maybe_type {
                        let jump_placeholder =
                            self.compile_check_type(id_register, *type_hint, ctx)?;
                        // Where should failed type checks jump to?
                        if params.is_last_alternative {
                            // No more `or` alternatives, so jump to the end of the arm
                            params.jumps.arm_end.push(jump_placeholder);
                        } else {
                            // Jump to the next `or` alternative pattern
                            params.jumps.alternative_end.push(jump_placeholder);
                        }
                    }

                    // The variable has received its value, is a jump needed?
                    if is_last_pattern && !params.is_last_alternative {
                        // e.g. x, 0, y or x, 1, y if x == y then
                        //            ^ ~~~~~~ We're here, jump to the if condition
                        self.push_op(Jump, &[]);
                        params.jumps.match_end.push(self.push_offset_placeholder());
                    }
                }
                Node::Ignored(_, maybe_type) => {
                    if let Some(type_hint) = maybe_type {
                        let temp_register = self.push_register()?;
                        if match_is_container {
                            self.push_op(
                                TempIndex,
                                &[temp_register, params.match_register, pattern_index as u8],
                            );
                        } else {
                            self.push_op(Copy, &[temp_register, params.match_register]);
                        }

                        let jump_placeholder =
                            self.compile_check_type(temp_register, *type_hint, ctx)?;
                        // Where should failed type checks jump to?
                        if params.is_last_alternative {
                            // No more `or` alternatives, so jump to the end of the arm
                            params.jumps.arm_end.push(jump_placeholder);
                        } else {
                            // Jump to the next `or` alternative pattern
                            params.jumps.alternative_end.push(jump_placeholder);
                        }
                    }

                    // The ignored id has been validated, is a jump needed?
                    if is_last_pattern && !params.is_last_alternative {
                        // e.g. x, 0, _ or x, 1, y if foo x then
                        //            ^~~~~~~ We're here, jump to the if condition
                        self.push_op(Jump, &[]);
                        params.jumps.match_end.push(self.push_offset_placeholder());
                    }
                }
                Node::Tuple {
                    elements: patterns, ..
                } => {
                    self.compile_nested_match_arm_patterns(
                        MatchArmParameters {
                            match_register: params.match_register,
                            is_last_alternative: params.is_last_alternative,
                            has_last_pattern: params.has_last_pattern,
                            jumps: params.jumps,
                        },
                        Some(pattern_index),
                        patterns,
                        ctx,
                    )?;
                }
                Node::PackedId(maybe_id) => {
                    if is_last_pattern {
                        if let Some(id) = maybe_id {
                            // e.g. [x, y, z, rest...]
                            // We want to assign the slice containing all but the first three items
                            // to the given id.
                            let id_register = self.assign_local_register(*id)?;
                            self.push_op(
                                SliceFrom,
                                &[id_register, params.match_register, pattern_index as u8],
                            );
                        }

                        if !params.is_last_alternative {
                            // Ellipses match unconditionally in last position,
                            // multi-expression pattern, skip over the remaining alternatives
                            // e.g. (x, 0, rest...) or (x, 1, y) if rest.size() > 0 then
                            //             ^~~~~~~ We're here, jump to the if condition
                            self.push_op(Jump, &[]);
                            params.jumps.match_end.push(self.push_offset_placeholder());
                        }
                    } else if is_first_pattern {
                        if let Some(id) = maybe_id {
                            // e.g. [first..., x, y]
                            // We want to assign the slice containing all but the last two items to
                            // the given id.
                            let id_register = self.assign_local_register(*id)?;
                            let to_index = -(arm_patterns.len() as i8 - 1) as u8;
                            self.push_op(SliceTo, &[id_register, params.match_register, to_index]);
                        }

                        index_from_end = true;
                    } else {
                        return self.error(ErrorKind::OutOfPositionMatchEllipsis);
                    }
                }
                unexpected => {
                    return self.error(ErrorKind::InvalidMatchPattern(unexpected.clone()));
                }
            }
        }

        Ok(())
    }

    fn compile_nested_match_arm_patterns(
        &mut self,
        params: MatchArmParameters,
        pattern_index: Option<i8>,
        nested_patterns: &[AstIndex],
        ctx: CompileNodeContext,
    ) -> Result<()> {
        use Op::*;

        let value_register = if let Some(pattern_index) = pattern_index {
            // Place the nested container into a register
            let value_register = self.push_register()?;
            self.push_op(
                TempIndex,
                &[value_register, params.match_register, pattern_index as u8],
            );
            value_register
        } else {
            params.match_register
        };

        let temp_register = self.push_register()?;

        let first_or_last_pattern_is_ellipsis = {
            let first_is_ellipsis = nested_patterns
                .first()
                .is_some_and(|first| matches!(ctx.node(*first), Node::PackedId(_)));
            let last_is_ellipsis = nested_patterns
                .last()
                .is_some_and(|last| matches!(ctx.node(*last), Node::PackedId(_)));
            if nested_patterns.len() > 1 && first_is_ellipsis && last_is_ellipsis {
                return self.error(ErrorKind::MultipleMatchEllipses);
            }
            first_is_ellipsis || last_is_ellipsis
        };

        // Check that the container has sufficient elements for the match patterns
        if !nested_patterns.is_empty() {
            let expected_register = self.push_register()?;
            self.push_op(Size, &[temp_register, value_register]);

            let patterns_len = nested_patterns.len() as u8;

            let comparison_op = if first_or_last_pattern_is_ellipsis {
                self.push_op(SetNumberU8, &[expected_register, patterns_len - 1]);
                GreaterOrEqual
            } else {
                self.push_op(SetNumberU8, &[expected_register, patterns_len]);
                Equal
            };
            self.push_op(
                comparison_op,
                &[temp_register, temp_register, expected_register],
            );
            self.push_op(JumpIfFalse, &[temp_register]);

            // If there aren't the expected number of elements, jump to the next match patterns
            if params.is_last_alternative {
                params.jumps.arm_end.push(self.push_offset_placeholder());
            } else {
                params
                    .jumps
                    .alternative_end
                    .push(self.push_offset_placeholder());
            }

            self.pop_register()?; // expected_register
        }

        self.pop_register()?; // temp_register

        // Call compile_match_arm_patterns with the nested matches
        self.compile_match_arm_patterns(
            MatchArmParameters {
                match_register: value_register,
                ..params
            },
            true, // match_is_container
            nested_patterns,
            ctx,
        )?;

        if pattern_index.is_some() {
            self.pop_register()?; // value_register
        }

        Ok(())
    }

    fn compile_for(
        &mut self,
        ast_for: &AstFor,
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        use Op::*;

        let AstFor {
            args,
            iterable,
            body,
        } = &ast_for;

        //   make iterator, iterator_register
        //   make local registers for args
        // loop_start:
        //   iterator_next_or_jump iterator_register arg_register jump -> end
        //   loop body
        //   jump -> loop_start
        // end:

        let result = self.assign_result_register(ctx)?;

        let body_result_register = if let Some(result_register) = result.register {
            self.push_op(SetNull, &[result_register]);
            Some(result_register)
        } else {
            None
        };

        let stack_count = self.stack_count();

        let iterator_register = {
            let iterator_register = self.push_register()?;
            let iterable_register = self.compile_node(*iterable, ctx.with_any_register())?;

            // Make the iterator, using the iterator's span in case of errors
            self.push_span(ctx.node_with_span(*iterable), ctx.ast);
            self.push_op(
                MakeIterator,
                &[iterator_register, iterable_register.unwrap(self)?],
            );
            self.pop_span();

            if iterable_register.is_temporary {
                self.pop_register()?;
            }

            iterator_register
        };

        let loop_start_ip = self.bytes.len();
        self.frame_mut()
            .push_loop(loop_start_ip, body_result_register);

        match args.as_slice() {
            [] => return self.error(ErrorKind::MissingArgumentInForLoop),
            [single_arg] => {
                match ctx.node(*single_arg) {
                    Node::Id(id, maybe_type) => {
                        // e.g. for i in 0..10
                        let arg_register = self.assign_local_register(*id)?;
                        self.push_op_without_span(IterNext, &[arg_register, iterator_register]);
                        self.push_loop_jump_placeholder()?;
                        if let Some(type_hint) = maybe_type {
                            self.compile_assert_type(
                                arg_register,
                                *type_hint,
                                Some(*single_arg),
                                ctx,
                            )?;
                        }
                    }
                    Node::Ignored(_, maybe_type) => {
                        if let Some(type_hint) = maybe_type {
                            // e.g. for _: Number in 0..10
                            let temp_register = self.push_register()?;
                            self.push_op_without_span(
                                IterNext,
                                &[temp_register, iterator_register],
                            );
                            self.push_loop_jump_placeholder()?;
                            self.compile_assert_type(
                                temp_register,
                                *type_hint,
                                Some(*single_arg),
                                ctx,
                            )?;
                            self.pop_register()?; // temp_register
                        } else {
                            // e.g. for _ in 0..10
                            self.push_op_without_span(IterNextQuiet, &[iterator_register]);
                            self.push_loop_jump_placeholder()?;
                        }
                    }
                    unexpected => {
                        return self.error(ErrorKind::UnexpectedNode {
                            expected: "ID in for loop args".into(),
                            unexpected: unexpected.clone(),
                        });
                    }
                }
            }
            args => {
                // e.g. for a, b, c in list_of_lists()
                // e.g. for key, value in map

                // A temporary register for the iterator's output.
                // Args are unpacked via iteration from the temp register
                let output_register = self.push_register()?;

                self.push_op_without_span(IterNextTemp, &[output_register, iterator_register]);
                self.push_loop_jump_placeholder()?;

                self.push_op_without_span(MakeIterator, &[output_register, output_register]);

                for arg in args.iter() {
                    match ctx.node(*arg) {
                        Node::Id(id, maybe_type) => {
                            let arg_register = self.assign_local_register(*id)?;
                            self.push_op_without_span(IterUnpack, &[arg_register, output_register]);
                            if let Some(type_hint) = maybe_type {
                                self.compile_assert_type(
                                    arg_register,
                                    *type_hint,
                                    Some(*arg),
                                    ctx,
                                )?;
                            }
                        }
                        Node::Ignored(_, maybe_type) => {
                            if let Some(type_hint) = maybe_type {
                                let arg_register = self.push_register()?;
                                self.push_op_without_span(
                                    IterUnpack,
                                    &[arg_register, output_register],
                                );
                                self.compile_assert_type(
                                    arg_register,
                                    *type_hint,
                                    Some(*arg),
                                    ctx,
                                )?;
                                self.pop_register()?; // arg_register
                            } else {
                                self.push_op_without_span(IterNextQuiet, &[output_register, 0, 0]);
                            }
                        }
                        unexpected => {
                            return self.error(ErrorKind::UnexpectedNode {
                                expected: "ID in for loop args".into(),
                                unexpected: unexpected.clone(),
                            });
                        }
                    }
                }

                self.pop_register()?; // output_register
            }
        }

        self.compile_node(
            *body,
            ctx.with_register(
                body_result_register.map_or(ResultRegister::None, ResultRegister::Fixed),
            ),
        )?;

        self.push_jump_back_op(JumpBack, &[], loop_start_ip);
        self.pop_loop_and_update_placeholders()?;

        self.truncate_register_stack(stack_count)?;

        if self.settings.export_top_level_ids && self.frame_stack.len() == 1 {
            for arg in args {
                if let Node::Id(id, ..) = ctx.node(*arg) {
                    let arg_register = match self.frame().get_local_assigned_register(*id) {
                        Some(register) => register,
                        None => return self.error(ErrorKind::MissingArgRegister),
                    };
                    self.compile_value_export(*id, arg_register)?;
                }
            }
        }

        Ok(result)
    }

    // Used to compile `loop`, `while`, and `until`
    fn compile_loop(
        &mut self,
        condition: Option<(AstIndex, bool)>, // condition, negate condition
        body: AstIndex,
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        use Op::*;

        let result = self.assign_result_register(ctx)?;
        let body_result_register = if let Some(result_register) = result.register {
            if condition.is_some() {
                // If there's a condition, then the result should be set to Null in case
                // there are no loop iterations
                self.push_op(SetNull, &[result_register]);
            }
            Some(result_register)
        } else {
            None
        };

        let loop_start_ip = self.bytes.len();
        self.frame_mut()
            .push_loop(loop_start_ip, body_result_register);

        if let Some((condition, negate_condition)) = condition {
            // Condition
            let condition_register = self.compile_node(condition, ctx.with_any_register())?;
            let op = if negate_condition {
                JumpIfTrue
            } else {
                JumpIfFalse
            };
            self.push_op_without_span(op, &[condition_register.unwrap(self)?]);
            self.push_loop_jump_placeholder()?;
            if condition_register.is_temporary {
                self.pop_register()?;
            }
        }

        let body_result = self.compile_node(
            body,
            ctx.with_register(
                body_result_register.map_or(ResultRegister::None, ResultRegister::Fixed),
            ),
        )?;

        self.push_jump_back_op(JumpBack, &[], loop_start_ip);

        if body_result.is_temporary {
            self.pop_register()?;
        }

        self.pop_loop_and_update_placeholders()?;

        Ok(result)
    }

    fn compile_node_with_jump_offset(
        &mut self,
        node_index: AstIndex,
        ctx: CompileNodeContext,
    ) -> Result<CompileNodeOutput> {
        let offset_ip = self.push_offset_placeholder();
        let result = self.compile_node(node_index, ctx)?;
        self.update_offset_placeholder(offset_ip)?;
        Ok(result)
    }

    fn push_jump_back_op(&mut self, op: Op, bytes: &[u8], target_ip: usize) {
        let offset = self.bytes.len() + 3 + bytes.len() - target_ip;
        self.push_op_without_span(op, bytes);
        self.push_bytes(&(offset as u16).to_le_bytes());
    }

    // For offset placeholders to work correctly,
    // ensure that they're the last value in the instruction.
    fn push_offset_placeholder(&mut self) -> usize {
        let offset_ip = self.bytes.len();
        self.push_bytes(&[0, 0]);
        offset_ip
    }

    fn push_loop_jump_placeholder(&mut self) -> Result<()> {
        let placeholder = self.push_offset_placeholder();
        self.frame_mut()
            .push_loop_jump_placeholder(placeholder)
            .map_err(|e| self.make_error(e))
    }

    fn pop_loop_and_update_placeholders(&mut self) -> Result<()> {
        let loop_info = self
            .frame_mut()
            .pop_loop()
            .map_err(|e| self.make_error(e))?;

        for placeholder in loop_info.jump_placeholders.iter() {
            self.update_offset_placeholder(*placeholder)?;
        }

        Ok(())
    }

    fn update_offset_placeholder(&mut self, offset_ip: usize) -> Result<()> {
        let offset = self.bytes.len() - offset_ip - 2; // -2 bytes for u16
        match u16::try_from(offset) {
            Ok(offset_u16) => {
                let offset_bytes = offset_u16.to_le_bytes();
                self.bytes[offset_ip] = offset_bytes[0];
                self.bytes[offset_ip + 1] = offset_bytes[1];
                Ok(())
            }
            Err(_) => self.error(ErrorKind::JumpOffsetIsTooLarge(offset)),
        }
    }

    fn push_op(&mut self, op: Op, bytes: &[u8]) {
        self.debug_info.push(self.bytes.len() as u32, self.span());
        self.push_op_without_span(op, bytes);
    }

    fn push_op_without_span(&mut self, op: Op, bytes: &[u8]) {
        self.bytes.push(op as u8);
        self.bytes.extend_from_slice(bytes);
    }

    fn push_bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    fn push_bytes_with_span(&mut self, bytes: &[u8], span: Span) {
        self.debug_info.push(self.bytes.len() as u32, span);
        self.bytes.extend_from_slice(bytes);
    }

    fn frame(&self) -> &Frame {
        self.frame_stack.last().expect("Frame stack is empty")
    }

    fn frame_mut(&mut self) -> &mut Frame {
        self.frame_stack.last_mut().expect("Frame stack is empty")
    }

    fn stack_count(&self) -> usize {
        self.frame().register_stack_size()
    }

    fn push_register(&mut self) -> Result<u8> {
        self.frame_mut()
            .push_register()
            .map_err(|e| self.make_error(e))
    }

    fn pop_register(&mut self) -> Result<u8> {
        self.frame_mut()
            .pop_register()
            .map_err(|e| self.make_error(e))
    }

    fn peek_register(&mut self, n: usize) -> Result<u8> {
        self.frame_mut()
            .peek_register(n)
            .map_err(|e| self.make_error(e))
    }

    fn truncate_register_stack(&mut self, stack_count: usize) -> Result<()> {
        self.frame_mut()
            .truncate_register_stack(stack_count)
            .map_err(|e| self.make_error(e))
    }

    // Used for values that can be assigned directly to a register
    fn assign_local_register(&mut self, local: ConstantIndex) -> Result<u8> {
        self.frame_mut()
            .assign_local_register(local)
            .map_err(|e| self.make_error(e))
    }

    // Used for expressions that are about to evaluated and assigned to a register
    //
    // After the RHS has been compiled it needs to be committed to be available in the local scope,
    // see commit_local_register.
    //
    // Reserving is necessary to avoid bringing the local's name into scope during the RHS's
    // evaluation before it's been assigned.
    fn reserve_local_register(&mut self, local: ConstantIndex) -> Result<u8> {
        self.frame_mut()
            .reserve_local_register(local)
            .map_err(|e| self.make_error(e))
    }

    // Commit a register now that the RHS expression for an assignment has been computed
    fn commit_local_register(&mut self, register: u8) -> Result<u8> {
        for deferred_op in self
            .frame_mut()
            .commit_local_register(register)
            .map_err(|e| self.make_error(e))?
        {
            self.push_bytes_with_span(&deferred_op.bytes, deferred_op.span);
        }

        Ok(register)
    }

    fn error<T>(&self, error: impl Into<ErrorKind>) -> Result<T> {
        Err(self.make_error(error))
    }

    fn make_error(&self, error: impl Into<ErrorKind>) -> CompilerError {
        CompilerError {
            error: error.into(),
            span: self.span(),
        }
    }

    fn push_span(&mut self, node: &AstNode, ast: &Ast) {
        self.span_stack.push(*ast.span(node.span));
    }

    fn pop_span(&mut self) {
        self.span_stack.pop();
    }

    fn span(&self) -> Span {
        *self.span_stack.last().expect("Empty span stack")
    }
}

fn args_size_op(args: &[AstIndex], ast: &Ast) -> (Op, usize) {
    if args
        .iter()
        .any(|arg| matches!(&ast.node(*arg).node, Node::PackedId(_)))
    {
        (Op::CheckSizeMin, args.len() - 1)
    } else {
        (Op::CheckSizeEqual, args.len())
    }
}

#[derive(Default)]
struct MatchJumpPlaceholders {
    // Jumps to the end of the arm
    arm_end: Vec<usize>,
    // Jumps to the end of the arm's match patterns,
    // used after a successful match to skip over remaining alternatives
    match_end: Vec<usize>,
    // Jumps to the end of the current arm alternative,
    // e.g.
    // match x
    //   0 or 1 or 2 then y
    //   ^~~~ a match failure here should attempt matching on the next alternative
    alternative_end: Vec<usize>,
}

struct MatchArmParameters<'a> {
    match_register: u8,
    is_last_alternative: bool,
    has_last_pattern: bool,
    jumps: &'a mut MatchJumpPlaceholders,
}

struct FrameParameters<'a> {
    local_count: u8,
    expressions: &'a [AstIndex],
    args: &'a [AstIndex],
    captures: &'a [ConstantIndex],
    allow_implicit_return: bool,
    output_type: Option<AstIndex>,
    is_generator: bool,
}

// Used by Compiler::compile_chain to keep track of incremental chain registers
#[derive(Default)]
struct ChainRegisters {
    // The previous 2 registers need to be kept around for instance calls
    //
    // 3 registers are kept in the buffer, the oldest can be reused for temporaries.
    registers: CircularBuffer<3, ChainRegister>,
}

impl ChainRegisters {
    fn push(&mut self, register: u8, is_temporary: bool) {
        self.registers.push_back(ChainRegister {
            register,
            is_temporary,
        });
    }

    fn is_empty(&self) -> bool {
        self.registers.is_empty()
    }

    fn reuse_oldest(&mut self) -> Option<u8> {
        if self.registers.is_full() {
            self.registers
                .pop_front()
                .filter(|register| register.is_temporary)
                .map(|register| register.register)
        } else {
            None
        }
    }

    fn previous(&self) -> Option<u8> {
        self.registers.back().map(|register| register.register)
    }

    fn previous_two(&self) -> (Option<u8>, Option<u8>) {
        let mut previous_iter = self
            .registers
            .iter()
            .map(|register| register.register)
            .rev();
        (previous_iter.next(), previous_iter.next())
    }
}

#[derive(Clone)]
struct ChainRegister {
    register: u8,
    is_temporary: bool,
}
