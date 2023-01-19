use {
    crate::{DebugInfo, FunctionFlags, Op, TypeId},
    koto_parser::{
        AssignTarget, Ast, AstBinaryOp, AstFor, AstIf, AstIndex, AstNode, AstTry, AstUnaryOp,
        ConstantIndex, Function, ImportItemNode, LookupNode, MapKey, MatchArm, MetaKeyId, Node,
        Scope, Span, StringNode, SwitchArm,
    },
    smallvec::SmallVec,
    std::{collections::HashSet, error, fmt},
};

/// The error type used to report errors during compilation
#[derive(Clone, Debug)]
pub struct CompilerError {
    /// The error's message
    pub message: String,
    /// The span in the source where the error occurred
    pub span: Span,
}

impl fmt::Display for CompilerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.message)
    }
}

impl error::Error for CompilerError {}

macro_rules! make_compiler_error {
    ($span:expr, $message:expr) => {{
        CompilerError {
            message: $message,
            span: $span,
        }
    }};
}

macro_rules! compiler_error {
    ($compiler:expr, $error:expr) => {
        Err(make_compiler_error!($compiler.span(), String::from($error)))
    };
    ($compiler:expr, $error:expr, $($args:expr),+ $(,)?) => {
        Err(make_compiler_error!($compiler.span(), format!($error, $($args),+)))
    };
}

#[derive(Clone, Debug)]
struct Loop {
    // The loop's result register,
    result_register: Option<u8>,
    // The ip of the start of the loop, used for continue statements
    start_ip: usize,
    // Placeholders for jumps to the end of the loop, updated when the loop compilation is complete
    jump_placeholders: Vec<usize>,
}

enum Arg {
    Local(ConstantIndex),
    Unpacked(ConstantIndex),
    Placeholder,
}

#[derive(Clone, Debug, PartialEq)]
enum LocalRegister {
    // The register is assigned to a specific id.
    Assigned(ConstantIndex),
    // The register is reserved at the start of an assignment expression,
    // and it will become assigned at the end of the assignment.
    // Instructions can be deferred until the register is committed,
    // e.g. for functions that need to capture themselves after they've been fully assigned.
    Reserved(ConstantIndex, Vec<DeferredOp>),
    // The register contains a value not associated with an id, e.g. a wildcard function arg
    Allocated,
}

#[derive(Clone, Debug, PartialEq)]
enum AssignedOrReserved {
    Assigned(u8),
    Reserved(u8),
    Unassigned,
}

#[derive(Clone, Debug, PartialEq)]
struct DeferredOp {
    bytes: Vec<u8>,
    span: Span,
}

#[derive(Clone, Debug, Default)]
struct Frame {
    loop_stack: Vec<Loop>,
    register_stack: Vec<u8>,
    local_registers: Vec<LocalRegister>,
    exported_ids: HashSet<ConstantIndex>,
    temporary_base: u8,
    temporary_count: u8,
    last_op: Option<Op>, // used to decide if an additional return instruction is needed
}

impl Frame {
    fn new(local_count: u8, args: &[Arg], captures: &[ConstantIndex]) -> Self {
        let temporary_base =
            // Includes all named args (including unpacked args),
            // and any locally assigned values.
            local_count
            // Captures get copied to local registers when the function is called.
            + captures.len() as u8
            // To get the first temporary register, we also need to include 'unnamed' args, which
            // are represented in the args list as Placeholders.
            + args
                .iter()
                .filter(|arg| matches!(arg, Arg::Placeholder))
                .count() as u8;

        // First, assign registers to the 'top-level' args, including placeholder registers
        let mut local_registers = Vec::with_capacity(args.len() + captures.len());
        local_registers.extend(args.iter().filter_map(|arg| match arg {
            Arg::Local(id) => Some(LocalRegister::Assigned(*id)),
            Arg::Placeholder => Some(LocalRegister::Allocated),
            _ => None,
        }));

        // Next, assign registers for the function's captures
        local_registers.extend(captures.iter().map(|id| LocalRegister::Assigned(*id)));

        // Finally, assign registers for args that will be unpacked when the function is called
        local_registers.extend(args.iter().filter_map(|arg| match arg {
            Arg::Unpacked(id) => Some(LocalRegister::Assigned(*id)),
            _ => None,
        }));

        Self {
            register_stack: Vec::with_capacity(temporary_base as usize),
            local_registers,
            temporary_base,
            ..Default::default()
        }
    }

    fn push_register(&mut self) -> Result<u8, String> {
        let new_register = self.temporary_base + self.temporary_count;
        self.temporary_count += 1;

        if new_register == u8::MAX {
            Err("Reached maximum number of registers".to_string())
        } else {
            self.register_stack.push(new_register);
            Ok(new_register)
        }
    }

    fn get_local_assigned_register(&self, local_name: ConstantIndex) -> Option<u8> {
        self.local_registers
            .iter()
            .position(|local_register| {
                matches!(local_register,
                    LocalRegister::Assigned(assigned) if *assigned == local_name
                )
            })
            .map(|position| position as u8)
    }

    fn get_local_assigned_or_reserved_register(
        &self,
        local_name: ConstantIndex,
    ) -> AssignedOrReserved {
        for (i, local_register) in self.local_registers.iter().enumerate() {
            match local_register {
                LocalRegister::Assigned(assigned) if *assigned == local_name => {
                    return AssignedOrReserved::Assigned(i as u8);
                }
                LocalRegister::Reserved(reserved, _) if *reserved == local_name => {
                    return AssignedOrReserved::Reserved(i as u8);
                }
                _ => {}
            }
        }
        AssignedOrReserved::Unassigned
    }

    fn reserve_local_register(&mut self, local: ConstantIndex) -> Result<u8, String> {
        match self.get_local_assigned_or_reserved_register(local) {
            AssignedOrReserved::Assigned(assigned) => Ok(assigned),
            AssignedOrReserved::Reserved(reserved) => Ok(reserved),
            AssignedOrReserved::Unassigned => {
                self.local_registers
                    .push(LocalRegister::Reserved(local, vec![]));

                let new_local_register = self.local_registers.len() - 1;

                if new_local_register > self.temporary_base as usize {
                    return Err("reserve_local_register: Locals overflowed".to_string());
                }

                Ok(new_local_register as u8)
            }
        }
    }

    fn add_to_exported_ids(&mut self, id: ConstantIndex) {
        self.exported_ids.insert(id);
    }

    fn defer_op_until_register_is_committed(
        &mut self,
        reserved_register: u8,
        bytes: Vec<u8>,
        span: Span,
    ) -> Result<(), String> {
        match self.local_registers.get_mut(reserved_register as usize) {
            Some(LocalRegister::Reserved(_, deferred_ops)) => {
                deferred_ops.push(DeferredOp { bytes, span });
                Ok(())
            }
            _ => Err(format!("register {reserved_register} hasn't been reserved")),
        }
    }

    fn commit_local_register(&mut self, local_register: u8) -> Result<Vec<DeferredOp>, String> {
        let local_register = local_register as usize;
        let (index, deferred_ops) = match self.local_registers.get(local_register) {
            Some(LocalRegister::Assigned(_)) => {
                return Ok(vec![]);
            }
            Some(LocalRegister::Reserved(index, deferred_ops)) => (*index, deferred_ops.to_vec()),
            _ => {
                return Err(format!(
                    "commit_local_register: register {local_register} hasn't been reserved"
                ));
            }
        };

        self.local_registers[local_register] = LocalRegister::Assigned(index);
        Ok(deferred_ops)
    }

    fn assign_local_register(&mut self, local: ConstantIndex) -> Result<u8, String> {
        match self.get_local_assigned_or_reserved_register(local) {
            AssignedOrReserved::Assigned(assigned) => Ok(assigned),
            AssignedOrReserved::Reserved(reserved) => {
                let deferred_ops = self.commit_local_register(reserved)?;
                if !deferred_ops.is_empty() {
                    return Err(
                        "assign_local_register: committing register that has remaining ops"
                            .to_string(),
                    );
                }
                Ok(reserved)
            }
            AssignedOrReserved::Unassigned => {
                self.local_registers.push(LocalRegister::Assigned(local));
                let new_local_register = self.local_registers.len() - 1;
                if new_local_register > self.temporary_base as usize {
                    return Err("assign_local_register: Locals overflowed".to_string());
                }
                Ok(new_local_register as u8)
            }
        }
    }

    fn pop_register(&mut self) -> Result<u8, String> {
        let register = match self.register_stack.pop() {
            Some(register) => register,
            None => {
                return Err("pop_register: Empty register stack".to_string());
            }
        };

        if register >= self.temporary_base {
            if self.temporary_count == 0 {
                return Err("pop_register: Unexpected temporary register".to_string());
            }

            self.temporary_count -= 1;
        }

        Ok(register)
    }

    fn peek_register(&self, n: usize) -> Result<u8, String> {
        self.register_stack
            .get(self.register_stack.len() - n - 1)
            .cloned()
            .ok_or_else(|| "peek_register_n: Non enough registers in the stack".to_string())
    }

    fn truncate_register_stack(&mut self, stack_count: usize) -> Result<(), String> {
        while self.register_stack.len() > stack_count {
            self.pop_register()?;
        }

        Ok(())
    }

    fn next_temporary_register(&self) -> u8 {
        self.temporary_count + self.temporary_base
    }

    fn available_registers_count(&self) -> u8 {
        u8::MAX - self.next_temporary_register()
    }

    fn captures_for_nested_frame(
        &self,
        accessed_non_locals: &[ConstantIndex],
    ) -> Vec<ConstantIndex> {
        // The non-locals accessed in the nested frame should be captured if they're in the current
        // frame's local scope, or if they were exported prior to the nested frame being created.
        accessed_non_locals
            .iter()
            .filter(|&non_local| {
                self.local_registers.iter().any(|register| match register {
                    LocalRegister::Assigned(assigned) if assigned == non_local => true,
                    LocalRegister::Reserved(reserved, _) if reserved == non_local => true,
                    _ => false,
                }) || self.exported_ids.contains(non_local)
            })
            .cloned()
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
enum ResultRegister {
    // The result will be ignored, expressions without side-effects can be dropped.
    None,
    // The result can be any temporary register, or an assigned register.
    Any,
    // The result must be placed in the specified register.
    Fixed(u8),
}

// While compiling a node, ResultRegister::Any might cause a temporary register to be allocated,
// so the result register should be determined before other temporary registers are allocated.
#[derive(Clone, Copy, Debug, Default)]
struct CompileResult {
    register: u8,
    is_temporary: bool,
}

impl CompileResult {
    fn with_assigned(register: u8) -> Self {
        Self {
            register,
            is_temporary: false,
        }
    }

    fn with_temporary(register: u8) -> Self {
        Self {
            register,
            is_temporary: true,
        }
    }
}

type CompileNodeResult = Result<Option<CompileResult>, CompilerError>;

/// The settings used by the [Compiler]
#[derive(Default)]
pub struct CompilerSettings {
    /// Causes all top level identifiers to be exported
    pub repl_mode: bool,
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
    /// Compiles an [Ast]
    ///
    /// Returns compiled bytecode along with corresponding debug information
    pub fn compile(
        ast: &Ast,
        settings: CompilerSettings,
    ) -> Result<(Vec<u8>, DebugInfo), CompilerError> {
        let mut compiler = Compiler {
            settings,
            ..Default::default()
        };

        if let Some(entry_point) = ast.entry_point() {
            compiler.compile_node(ResultRegister::None, entry_point, ast)?;
        }

        Ok((compiler.bytes, compiler.debug_info))
    }

    fn compile_node(
        &mut self,
        result_register: ResultRegister,
        node: &AstNode,
        ast: &Ast,
    ) -> CompileNodeResult {
        use Op::*;

        self.span_stack.push(*ast.span(node.span));

        let result = match &node.node {
            Node::Null => {
                let result = self.get_result_register(result_register)?;
                if let Some(result) = result {
                    self.push_op(SetNull, &[result.register]);
                }
                result
            }
            Node::Nested(nested) => self.compile_node(result_register, ast.node(*nested), ast)?,
            Node::Id(index) => self.compile_load_id(result_register, *index)?,
            Node::Lookup(lookup) => {
                self.compile_lookup(result_register, lookup, None, None, None, ast)?
            }
            Node::BoolTrue => {
                let result = self.get_result_register(result_register)?;
                if let Some(result) = result {
                    self.push_op(SetTrue, &[result.register]);
                }
                result
            }
            Node::BoolFalse => {
                let result = self.get_result_register(result_register)?;
                if let Some(result) = result {
                    self.push_op(SetFalse, &[result.register]);
                }
                result
            }
            Node::SmallInt(n) => {
                let result = self.get_result_register(result_register)?;
                if let Some(result) = result {
                    match *n {
                        0 => self.push_op(Set0, &[result.register]),
                        1 => self.push_op(Set1, &[result.register]),
                        n if n >= 0 => self.push_op(SetNumberU8, &[result.register, n as u8]),
                        n => {
                            self.push_op(SetNumberNegU8, &[result.register, n.unsigned_abs() as u8])
                        }
                    }
                }
                result
            }
            Node::Float(constant) => {
                let result = self.get_result_register(result_register)?;
                if let Some(result) = result {
                    self.compile_constant_op(
                        result.register,
                        *constant,
                        LoadFloat,
                        LoadFloat16,
                        LoadFloat24,
                    );
                }
                result
            }
            Node::Int(constant) => {
                let result = self.get_result_register(result_register)?;
                if let Some(result) = result {
                    self.compile_constant_op(
                        result.register,
                        *constant,
                        LoadInt,
                        LoadInt16,
                        LoadInt24,
                    );
                }
                result
            }
            Node::Str(string) => self.compile_string(result_register, &string.nodes, ast)?,
            Node::List(elements) => {
                self.compile_make_sequence(result_register, elements, Op::SequenceToList, ast)?
            }
            Node::Map(entries) => self.compile_make_map(result_register, entries, ast)?,
            Node::Range {
                start,
                end,
                inclusive,
            } => match self.get_result_register(result_register)? {
                Some(result) => {
                    let start_register = self
                        .compile_node(ResultRegister::Any, ast.node(*start), ast)?
                        .unwrap();

                    let end_register = self
                        .compile_node(ResultRegister::Any, ast.node(*end), ast)?
                        .unwrap();

                    let op = if *inclusive { RangeInclusive } else { Range };
                    self.push_op(
                        op,
                        &[
                            result.register,
                            start_register.register,
                            end_register.register,
                        ],
                    );

                    if start_register.is_temporary {
                        self.pop_register()?;
                    }
                    if end_register.is_temporary {
                        self.pop_register()?;
                    }

                    Some(result)
                }
                None => {
                    self.compile_node(ResultRegister::None, ast.node(*start), ast)?;
                    self.compile_node(ResultRegister::None, ast.node(*end), ast)?
                }
            },
            Node::RangeFrom { start } => match self.get_result_register(result_register)? {
                Some(result) => {
                    let start_register = self
                        .compile_node(ResultRegister::Any, ast.node(*start), ast)?
                        .unwrap();

                    self.push_op(RangeFrom, &[result.register, start_register.register]);

                    if start_register.is_temporary {
                        self.pop_register()?;
                    }

                    Some(result)
                }
                None => self.compile_node(ResultRegister::None, ast.node(*start), ast)?,
            },
            Node::RangeTo { end, inclusive } => match self.get_result_register(result_register)? {
                Some(result) => {
                    let end_register = self
                        .compile_node(ResultRegister::Any, ast.node(*end), ast)?
                        .unwrap();

                    let op = if *inclusive {
                        RangeToInclusive
                    } else {
                        RangeTo
                    };
                    self.push_op(op, &[result.register, end_register.register]);

                    if end_register.is_temporary {
                        self.pop_register()?;
                    }

                    Some(result)
                }
                None => self.compile_node(ResultRegister::None, ast.node(*end), ast)?,
            },
            Node::RangeFull => {
                let result = self.get_result_register(result_register)?;
                if let Some(result) = result {
                    self.push_op(RangeFull, &[result.register]);
                }
                result
            }
            Node::MainBlock { body, local_count } => {
                self.compile_frame(*local_count as u8, body, &[], &[], ast, true)?;
                None
            }
            Node::Block(expressions) => self.compile_block(result_register, expressions, ast)?,
            Node::Tuple(elements) => {
                self.compile_make_sequence(result_register, elements, Op::SequenceToTuple, ast)?
            }
            Node::TempTuple(elements) => {
                self.compile_make_temp_tuple(result_register, elements, ast)?
            }
            Node::Function(f) => self.compile_function(result_register, f, ast)?,
            Node::NamedCall { id, args } => {
                self.compile_named_call(result_register, *id, args, None, ast)?
            }
            Node::Import { from, items } => {
                self.compile_import_expression(result_register, from, items, ast)?
            }
            Node::Assign { target, expression } => {
                self.compile_assign(result_register, target, *expression, ast)?
            }
            Node::MultiAssign {
                targets,
                expression,
            } => self.compile_multi_assign(result_register, targets, *expression, ast)?,
            Node::UnaryOp { op, value } => {
                self.compile_unary_op(result_register, *op, *value, ast)?
            }
            Node::BinaryOp { op, lhs, rhs } => {
                self.compile_binary_op(result_register, *op, *lhs, *rhs, ast)?
            }
            Node::If(ast_if) => self.compile_if(result_register, ast_if, ast)?,
            Node::Match { expression, arms } => {
                self.compile_match(result_register, *expression, arms, ast)?
            }
            Node::Switch(arms) => self.compile_switch(result_register, arms, ast)?,
            Node::Ellipsis(_) => {
                return compiler_error!(self, "Ellipsis found outside of match patterns")
            }
            Node::Wildcard(_) => {
                return compiler_error!(self, "Attempting to access an ignored value")
            }
            Node::For(ast_for) => self.compile_for(result_register, ast_for, ast)?,
            Node::While { condition, body } => {
                self.compile_loop(result_register, Some((*condition, false)), *body, ast)?
            }
            Node::Until { condition, body } => {
                self.compile_loop(result_register, Some((*condition, true)), *body, ast)?
            }
            Node::Loop { body } => self.compile_loop(result_register, None, *body, ast)?,
            Node::Break(expression) => match self.frame().loop_stack.last() {
                Some(loop_info) => {
                    let loop_result_register = loop_info.result_register;

                    match (loop_result_register, expression) {
                        (Some(loop_result_register), Some(expression)) => {
                            self.compile_node(
                                ResultRegister::Fixed(loop_result_register),
                                ast.node(*expression),
                                ast,
                            )?
                            .unwrap();
                        }
                        (Some(loop_result_register), None) => {
                            self.push_op(SetNull, &[loop_result_register]);
                        }
                        (None, Some(_)) => {
                            return compiler_error!(
                                self,
                                "The result of this `break` expression will be ignored,
                                consider assigning the result of the loop"
                            );
                        }
                        (None, None) => {}
                    }

                    self.push_op(Jump, &[]);
                    self.push_loop_jump_placeholder()?;

                    None
                }
                None => return compiler_error!(self, "`break` used outside of loop"),
            },
            Node::Continue => match self.frame().loop_stack.last() {
                Some(loop_info) => {
                    let loop_result_register = loop_info.result_register;
                    let loop_start_ip = loop_info.start_ip;

                    if let Some(result_register) = loop_result_register {
                        self.push_op(SetNull, &[result_register]);
                    }
                    self.push_jump_back_op(JumpBack, &[], loop_start_ip);

                    None
                }
                None => {
                    return compiler_error!(self, "`continue` used outside of loop");
                }
            },
            Node::Return(None) => match self.get_result_register(result_register)? {
                Some(result) => {
                    self.push_op(SetNull, &[result.register]);
                    self.push_op(Return, &[result.register]);
                    Some(result)
                }
                None => {
                    let register = self.push_register()?;
                    self.push_op(SetNull, &[register]);
                    self.push_op(Return, &[register]);
                    self.pop_register()?;
                    None
                }
            },
            Node::Return(Some(expression)) => {
                let expression_register = self
                    .compile_node(ResultRegister::Any, ast.node(*expression), ast)?
                    .unwrap();

                match result_register {
                    ResultRegister::Any => {
                        self.push_op(Return, &[expression_register.register]);
                        Some(expression_register)
                    }
                    ResultRegister::Fixed(result) => {
                        self.push_op(Copy, &[result, expression_register.register]);
                        self.push_op(Return, &[result]);
                        if expression_register.is_temporary {
                            self.pop_register()?;
                        }
                        Some(CompileResult::with_assigned(result))
                    }
                    ResultRegister::None => {
                        self.push_op(Return, &[expression_register.register]);
                        if expression_register.is_temporary {
                            self.pop_register()?;
                        }
                        None
                    }
                }
            }
            Node::Yield(expression) => {
                let result = self.get_result_register(result_register)?;

                let expression_register = self
                    .compile_node(ResultRegister::Any, ast.node(*expression), ast)?
                    .unwrap();

                self.push_op(Yield, &[expression_register.register]);

                if let Some(result) = result {
                    self.push_op(Copy, &[result.register, expression_register.register]);
                }

                if expression_register.is_temporary {
                    self.pop_register()?;
                }

                result
            }
            Node::Throw(expression) => {
                let expression_register = self
                    .compile_node(ResultRegister::Any, ast.node(*expression), ast)?
                    .unwrap();

                self.push_op(Throw, &[expression_register.register]);

                if expression_register.is_temporary {
                    self.pop_register()?;
                }

                None
            }
            Node::Try(try_expression) => {
                self.compile_try_expression(result_register, try_expression, ast)?
            }
            Node::Debug {
                expression_string,
                expression,
            } => {
                let result = self.get_result_register(result_register)?;

                let expression_register = self
                    .compile_node(ResultRegister::Any, ast.node(*expression), ast)?
                    .unwrap();

                self.push_op(Debug, &[expression_register.register]);
                self.push_bytes(&expression_string.bytes());

                if let Some(result) = result {
                    self.push_op(Copy, &[result.register, expression_register.register]);
                }

                if expression_register.is_temporary {
                    self.pop_register()?;
                }

                result
            }
            Node::Meta(_, _) => {
                // Meta nodes are currently only compiled in the context of an export assignment,
                // see compile_assign().
                unreachable!();
            }
        };

        self.span_stack.pop();

        Ok(result)
    }

    fn get_result_register(&mut self, result_register: ResultRegister) -> CompileNodeResult {
        let result = match result_register {
            ResultRegister::Fixed(register) => Some(CompileResult::with_assigned(register)),
            ResultRegister::Any => Some(CompileResult::with_temporary(self.push_register()?)),
            ResultRegister::None => None,
        };

        Ok(result)
    }

    fn compile_frame(
        &mut self,
        local_count: u8,
        expressions: &[AstIndex],
        args: &[AstIndex],
        captures: &[ConstantIndex],
        ast: &Ast,
        allow_implicit_return: bool,
    ) -> Result<(), CompilerError> {
        self.frame_stack.push(Frame::new(
            local_count,
            &self.collect_args(args, ast)?,
            captures,
        ));

        // unpack nested args
        for (arg_index, arg) in args.iter().enumerate() {
            let arg_node = ast.node(*arg);
            self.span_stack.push(*ast.span(arg_node.span));
            match &arg_node.node {
                Node::List(nested_args) => {
                    let list_register = arg_index as u8;
                    let size_op = args_size_op(nested_args, ast);
                    self.push_op(Op::CheckType, &[list_register, TypeId::List as u8]);
                    self.push_op(size_op, &[list_register, nested_args.len() as u8]);
                    self.compile_unpack_nested_args(list_register, nested_args, ast)?;
                }
                Node::Tuple(nested_args) => {
                    let tuple_register = arg_index as u8;
                    let size_op = args_size_op(nested_args, ast);
                    self.push_op(Op::CheckType, &[tuple_register, TypeId::Tuple as u8]);
                    self.push_op(size_op, &[tuple_register, nested_args.len() as u8]);
                    self.compile_unpack_nested_args(tuple_register, nested_args, ast)?;
                }
                _ => {}
            }
            self.span_stack.pop();
        }

        let result_register = if allow_implicit_return {
            ResultRegister::Any
        } else {
            ResultRegister::None
        };

        let block_result = self.compile_block(result_register, expressions, ast)?;

        if let Some(result) = block_result {
            if self.frame().last_op != Some(Op::Return) {
                self.push_op_without_span(Op::Return, &[result.register]);
            }
            if result.is_temporary {
                self.pop_register()?;
            }
        } else {
            let register = self.push_register()?;
            self.push_op(Op::SetNull, &[register]);
            self.push_op_without_span(Op::Return, &[register]);
            self.pop_register()?;
        }

        self.frame_stack.pop();

        Ok(())
    }

    fn collect_args(&self, args: &[AstIndex], ast: &Ast) -> Result<Vec<Arg>, CompilerError> {
        // Collect args for local assignment in the new frame
        // Top-level args need to match the arguments as they appear in the arg list, with
        // Placeholders for wildcards and containers that are being unpacked.
        // Unpacked IDs have registers assigned for them after the top-level IDs.
        // e.g. Given:
        // f = |a, (b, (c, d)), _, e|
        // Args should then appear as:
        // [Local(a), Placeholder, Placeholder, Local(e), Unpacked(b), Unpacked(c), Unpacked(d)]
        //
        // Note that the value stack at runtime will have the function's captures loaded in after
        // the top-level locals and placeholders, and before any unpacked args (e.g. in the example
        // above, captures will be placed after Local(e) and before Unpacked(b)).

        let mut result = Vec::new();
        let mut nested_args = Vec::new();

        for arg in args.iter() {
            match &ast.node(*arg).node {
                Node::Id(id_index) => result.push(Arg::Local(*id_index)),
                Node::Wildcard(_) => result.push(Arg::Placeholder),
                Node::List(nested) | Node::Tuple(nested) => {
                    result.push(Arg::Placeholder);
                    nested_args.extend(self.collect_nested_args(nested, ast)?);
                }
                unexpected => {
                    return compiler_error!(
                        self,
                        "Expected ID in function args, found {}",
                        unexpected
                    )
                }
            }
        }

        result.extend(nested_args);
        Ok(result)
    }

    fn collect_nested_args(&self, args: &[AstIndex], ast: &Ast) -> Result<Vec<Arg>, CompilerError> {
        let mut result = Vec::new();

        for arg in args.iter() {
            match &ast.node(*arg).node {
                Node::Id(id) => result.push(Arg::Unpacked(*id)),
                Node::Wildcard(_) => {}
                Node::List(nested_args) | Node::Tuple(nested_args) => {
                    result.extend(self.collect_nested_args(nested_args, ast)?);
                }
                Node::Ellipsis(Some(id)) => result.push(Arg::Unpacked(*id)),
                Node::Ellipsis(None) => {}
                unexpected => {
                    return compiler_error!(
                        self,
                        "Expected ID in function args, found {}",
                        unexpected
                    )
                }
            }
        }

        Ok(result)
    }

    fn compile_unpack_nested_args(
        &mut self,
        container_register: u8,
        args: &[AstIndex],
        ast: &Ast,
    ) -> Result<(), CompilerError> {
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

            match &ast.node(*arg).node {
                Node::Wildcard(_) => {}
                Node::Id(constant_index) => {
                    let local_register = self.assign_local_register(*constant_index)?;
                    self.push_op(TempIndex, &[local_register, container_register, arg_index]);
                }
                Node::List(nested_args) => {
                    let list_register = self.push_register()?;
                    let size_op = args_size_op(nested_args, ast);
                    self.push_op(TempIndex, &[list_register, container_register, arg_index]);
                    self.push_op(CheckType, &[list_register, TypeId::List as u8]);
                    self.push_op(size_op, &[list_register, nested_args.len() as u8]);
                    self.compile_unpack_nested_args(list_register, nested_args, ast)?;
                    self.pop_register()?; // list_register
                }
                Node::Tuple(nested_args) => {
                    let tuple_register = self.push_register()?;
                    let size_op = args_size_op(nested_args, ast);
                    self.push_op(TempIndex, &[tuple_register, container_register, arg_index]);
                    self.push_op(CheckType, &[tuple_register, TypeId::Tuple as u8]);
                    self.push_op(size_op, &[tuple_register, nested_args.len() as u8]);
                    self.compile_unpack_nested_args(tuple_register, nested_args, ast)?;
                    self.pop_register()?; // tuple_register
                }
                Node::Ellipsis(maybe_id) if is_first_arg => {
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
                Node::Ellipsis(Some(id)) if is_last_arg => {
                    // e.g. [x, y, z, rest...]
                    // We want to assign the slice containing all but the first three items
                    // to the given id.
                    let id_register = self.assign_local_register(*id)?;
                    self.push_op(SliceFrom, &[id_register, container_register, arg_index]);
                }
                Node::Ellipsis(None) if is_last_arg => {}
                Node::Ellipsis(_) => {
                    return compiler_error!(
                        self,
                        "Args with ellipses are only allowed in first or last position"
                    );
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn compile_block(
        &mut self,
        result_register: ResultRegister,
        expressions: &[AstIndex],
        ast: &Ast,
    ) -> CompileNodeResult {
        use Op::SetNull;

        let result = match expressions {
            [] => match self.get_result_register(result_register)? {
                Some(result) => {
                    self.push_op(SetNull, &[result.register]);
                    Some(result)
                }
                None => {
                    // TODO Under what conditions do we get into this branch?
                    let register = self.push_register()?;
                    self.push_op(SetNull, &[register]);
                    Some(CompileResult::with_temporary(register))
                }
            },
            [expression] => self.compile_node(result_register, ast.node(*expression), ast)?,
            [expressions @ .., last_expression] => {
                for expression in expressions.iter() {
                    self.compile_node(ResultRegister::None, ast.node(*expression), ast)?;
                }

                self.compile_node(result_register, ast.node(*last_expression), ast)?
            }
        };

        Ok(result)
    }

    fn scope_for_assign_target(&self, target: &AssignTarget) -> Scope {
        if self.settings.repl_mode && self.frame_stack.len() == 1 {
            Scope::Export
        } else {
            target.scope
        }
    }

    fn local_register_for_assign_target(
        &mut self,
        target: &AssignTarget,
        ast: &Ast,
    ) -> Result<Option<u8>, CompilerError> {
        let result = match self.scope_for_assign_target(target) {
            Scope::Local => match &ast.node(target.target_index).node {
                Node::Id(constant_index) => Some(self.reserve_local_register(*constant_index)?),
                Node::Lookup(_) | Node::Wildcard(_) => None,
                unexpected => {
                    return compiler_error!(self, "Expected Id in AST, found {}", unexpected)
                }
            },
            _ => None,
        };

        Ok(result)
    }

    fn compile_assign(
        &mut self,
        result_register: ResultRegister,
        target: &AssignTarget,
        expression: AstIndex,
        ast: &Ast,
    ) -> CompileNodeResult {
        use Op::*;

        let local_assign_register = self.local_register_for_assign_target(target, ast)?;
        let value_result_register = match local_assign_register {
            Some(local) => ResultRegister::Fixed(local),
            None => ResultRegister::Any,
        };

        let value_register = self
            .compile_node(value_result_register, ast.node(expression), ast)?
            .unwrap();

        let target_node = ast.node(target.target_index);
        self.span_stack.push(*ast.span(target_node.span));

        match &target_node.node {
            Node::Id(id_index) => {
                match self.scope_for_assign_target(target) {
                    Scope::Local => {
                        if !value_register.is_temporary {
                            // To ensure that exported rhs ids with the same name as a local that's
                            // currently being assigned can be loaded correctly, only commit the
                            // reserved local as assigned after the rhs has been compiled.
                            self.commit_local_register(value_register.register)?;
                        }
                    }
                    Scope::Export => {
                        self.compile_value_export(*id_index, value_register.register)?;
                    }
                }
            }
            Node::Lookup(lookup) => {
                self.compile_lookup(
                    ResultRegister::None,
                    lookup,
                    None,
                    Some(value_register.register),
                    None,
                    ast,
                )?;
            }
            Node::Meta(meta_id, name) => {
                self.compile_meta_export(*meta_id, *name, value_register.register)?;
            }
            Node::Wildcard(_) => {}
            unexpected => {
                return compiler_error!(self, "Expected Lookup or Id in AST, found {}", unexpected)
            }
        };

        let result = match result_register {
            ResultRegister::Fixed(register) => {
                if register != value_register.register {
                    self.push_op(Copy, &[register, value_register.register]);
                }
                Some(CompileResult::with_assigned(register))
            }
            ResultRegister::Any => Some(value_register),
            ResultRegister::None => None,
        };

        self.span_stack.pop();

        Ok(result)
    }

    fn compile_multi_assign(
        &mut self,
        result_register: ResultRegister,
        targets: &[AssignTarget],
        expression: AstIndex,
        ast: &Ast,
    ) -> CompileNodeResult {
        use Op::*;

        if targets.len() >= u8::MAX as usize {
            return compiler_error!(
                self,
                "Too many targets in multi-assignment, ({})",
                targets.len()
            );
        }

        let result = self.get_result_register(result_register)?;

        // Reserve any assignment registers for IDs on the LHS before compiling the RHS
        let target_registers = targets
            .iter()
            .map(|target| self.local_register_for_assign_target(target, ast))
            .collect::<Result<Vec<_>, _>>()?;

        let rhs_is_temp_tuple = matches!(ast.node(expression).node, Node::TempTuple(_));
        let rhs = self
            .compile_node(ResultRegister::Any, ast.node(expression), ast)?
            .unwrap();

        for (i, (target, target_register)) in
            targets.iter().zip(target_registers.iter()).enumerate()
        {
            match &ast.node(target.target_index).node {
                Node::Id(id_index) => {
                    match (target_register, self.scope_for_assign_target(target)) {
                        (Some(target_register), Scope::Local) => {
                            self.push_op(TempIndex, &[*target_register, rhs.register, i as u8]);
                            // The register was reserved before the RHS was compiled, and now it
                            // needs to be committed.
                            self.commit_local_register(*target_register)?;
                        }
                        (None, Scope::Export) => {
                            let index_register = self.push_register()?;
                            self.push_op(TempIndex, &[index_register, rhs.register, i as u8]);
                            self.compile_value_export(*id_index, index_register)?;
                            self.pop_register()?; // index_register
                        }
                        _ => {
                            // Either the scope is local, so there should be a reserved target
                            // register, or the scope is export, so there shouldn't be a
                            // reserved register.
                            unreachable!();
                        }
                    }
                }
                Node::Lookup(lookup) => {
                    let register = self.push_register()?;

                    self.push_op(TempIndex, &[register, rhs.register, i as u8]);
                    self.compile_lookup(
                        ResultRegister::None,
                        lookup,
                        None,
                        Some(register),
                        None,
                        ast,
                    )?;

                    self.pop_register()?;
                }
                Node::Wildcard(_) => {}
                unexpected => {
                    return compiler_error!(
                        self,
                        "Expected ID or lookup in AST, found {}",
                        unexpected
                    );
                }
            };
        }

        if let Some(result) = result {
            if rhs_is_temp_tuple {
                self.push_op(TempTupleToTuple, &[result.register, rhs.register]);
            } else {
                self.push_op(Copy, &[result.register, rhs.register]);
            }
        }

        if rhs.is_temporary {
            self.pop_register()?;
        }

        Ok(result)
    }

    fn compile_load_id(
        &mut self,
        result_register: ResultRegister,
        id: ConstantIndex,
    ) -> CompileNodeResult {
        let result = if let Some(local_register) = self.frame().get_local_assigned_register(id) {
            match result_register {
                ResultRegister::None => None,
                ResultRegister::Any => Some(CompileResult::with_assigned(local_register)),
                ResultRegister::Fixed(register) => {
                    self.push_op(Op::Copy, &[register, local_register]);
                    Some(CompileResult::with_assigned(register))
                }
            }
        } else {
            match self.get_result_register(result_register)? {
                Some(result) => {
                    self.compile_load_non_local(result.register, id);
                    Some(result)
                }
                None => {
                    let register = self.push_register()?;
                    self.compile_load_non_local(register, id);
                    Some(CompileResult::with_temporary(register))
                }
            }
        };

        Ok(result)
    }

    fn compile_value_export(
        &mut self,
        id: ConstantIndex,
        value_register: u8,
    ) -> Result<(), CompilerError> {
        let id_register = self.push_register()?;
        self.compile_load_string_constant(id_register, id);
        self.push_op(Op::ValueExport, &[id_register, value_register]);
        self.pop_register()?;
        self.frame_mut().add_to_exported_ids(id);
        Ok(())
    }

    fn compile_meta_export(
        &mut self,
        meta_id: MetaKeyId,
        name: Option<ConstantIndex>,
        value_register: u8,
    ) -> Result<(), CompilerError> {
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
        self.compile_constant_op(
            result_register,
            index,
            Op::LoadString,
            Op::LoadString16,
            Op::LoadString24,
        );
    }

    fn compile_load_non_local(&mut self, result_register: u8, id: ConstantIndex) {
        self.compile_constant_op(
            result_register,
            id,
            Op::LoadNonLocal,
            Op::LoadNonLocal16,
            Op::LoadNonLocal24,
        );
    }

    fn compile_constant_op(
        &mut self,
        result_register: u8,
        id: ConstantIndex,
        op8: Op,
        op16: Op,
        op24: Op,
    ) {
        match id.bytes() {
            [byte1, 0, 0] => self.push_op(op8, &[result_register, byte1]),
            [byte1, byte2, 0] => self.push_op(op16, &[result_register, byte1, byte2]),
            [byte1, byte2, byte3] => self.push_op(op24, &[result_register, byte1, byte2, byte3]),
        }
    }

    fn compile_import_expression(
        &mut self,
        result_register: ResultRegister,
        from: &[ImportItemNode],
        items: &[Vec<ImportItemNode>],
        ast: &Ast,
    ) -> CompileNodeResult {
        use Op::*;

        let result = self.get_result_register(result_register)?;

        let stack_count = self.frame().register_stack.len();

        let mut imported = vec![];

        if from.is_empty() {
            for item in items.iter() {
                match item.last() {
                    Some(ImportItemNode::Id(import_id)) => {
                        if result.is_some() {
                            // The result of the import expression is being assigned,
                            // so import the item into a temporary register.
                            let import_register = self.push_register()?;
                            self.compile_import_item(import_register, item, ast)?;
                            imported.push(import_register);
                        } else {
                            // Reserve a local for the imported item.
                            // The register must only be reserved for now otherwise it'll show up in
                            // the import search.
                            let import_register = self.reserve_local_register(*import_id)?;
                            self.compile_import_item(import_register, item, ast)?;

                            // Commit the register now that the import is complete
                            self.commit_local_register(import_register)?;

                            // If we're in repl mode then re-export the imported id
                            if self.settings.repl_mode && self.frame_stack.len() == 1 {
                                self.compile_value_export(*import_id, import_register)?;
                            }
                        }
                    }
                    Some(ImportItemNode::Str(_)) => {
                        let import_register = self.push_register()?;
                        self.compile_import_item(import_register, item, ast)?;
                        imported.push(import_register);
                    }
                    None => return compiler_error!(self, "Missing ID in import item"),
                };
            }
        } else {
            let from_register = self.push_register()?;

            self.compile_import_item(from_register, from, ast)?;

            for item in items.iter() {
                match item.last() {
                    Some(ImportItemNode::Id(import_id)) => {
                        let import_register = if result.is_some() {
                            // The result of the import expression is being assigned,
                            // so import the item into a temporary register.
                            self.push_register()?
                        } else {
                            // Assign the leaf item to a local with a matching name.
                            self.assign_local_register(*import_id)?
                        };

                        // Access the item from from_register, incrementally accessing nested items
                        let mut access_register = from_register;
                        for item_node in item.iter() {
                            match item_node {
                                ImportItemNode::Id(id) => {
                                    self.compile_access_id(import_register, access_register, *id)?;
                                }
                                ImportItemNode::Str(string) => {
                                    self.compile_access_string(
                                        import_register,
                                        access_register,
                                        &string.nodes,
                                        ast,
                                    )?;
                                }
                            }
                            access_register = import_register;
                        }

                        if result.is_some() {
                            imported.push(import_register);
                        } else {
                            // If we're in repl mode then re-export the imported id
                            if self.settings.repl_mode && self.frame_stack.len() == 1 {
                                self.compile_value_export(*import_id, import_register)?;
                            }
                        }
                    }
                    Some(ImportItemNode::Str(_)) => {
                        let import_register = self.push_register()?;

                        // Access the item from from_register, incrementally accessing nested items
                        let mut access_register = from_register;
                        for item_node in item.iter() {
                            match item_node {
                                ImportItemNode::Id(id) => {
                                    self.compile_access_id(import_register, access_register, *id)?;
                                }
                                ImportItemNode::Str(string) => {
                                    self.compile_access_string(
                                        import_register,
                                        access_register,
                                        &string.nodes,
                                        ast,
                                    )?;
                                }
                            }
                            access_register = import_register;
                        }

                        imported.push(import_register);
                    }
                    None => return compiler_error!(self, "Missing ID in import item"),
                };
            }
        }

        if let Some(result) = result {
            match imported.as_slice() {
                [] => return compiler_error!(self, "Missing item to import"),
                [single_item] => self.push_op(Copy, &[result.register, *single_item]),
                _ => {
                    self.push_op(SequenceStart, &[result.register, imported.len() as u8]);
                    for item in imported.iter() {
                        self.push_op(SequencePush, &[result.register, *item]);
                    }
                    self.push_op(SequenceToTuple, &[result.register]);
                }
            }
        }

        self.truncate_register_stack(stack_count)?;

        Ok(result)
    }

    fn compile_import_item(
        &mut self,
        result_register: u8,
        item: &[ImportItemNode],
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        match item {
            [] => return compiler_error!(self, "Missing item to import"),
            [root] => {
                self.compile_import_root(result_register, root, ast)?;
            }
            [root, nested @ ..] => {
                self.compile_import_root(result_register, root, ast)?;

                for nested_item in nested.iter() {
                    match nested_item {
                        ImportItemNode::Id(id) => {
                            self.compile_access_id(result_register, result_register, *id)?
                        }
                        ImportItemNode::Str(string) => self.compile_access_string(
                            result_register,
                            result_register,
                            &string.nodes,
                            ast,
                        )?,
                    }
                }
            }
        }

        Ok(())
    }

    fn compile_import_root(
        &mut self,
        import_register: u8,
        root: &ImportItemNode,
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        use Op::*;

        match root {
            ImportItemNode::Id(id) => {
                if let Some(local_register) = self.frame().get_local_assigned_register(*id) {
                    if local_register != import_register {
                        self.push_op(Copy, &[import_register, local_register]);
                    }
                } else {
                    // If the id isn't a local then it needs to be imported
                    self.compile_load_string_constant(import_register, *id);
                }
            }
            ImportItemNode::Str(string) => {
                self.compile_string(ResultRegister::Fixed(import_register), &string.nodes, ast)?;
            }
        }

        self.push_op(Import, &[import_register]);

        Ok(())
    }

    fn compile_try_expression(
        &mut self,
        result_register: ResultRegister,
        try_expression: &AstTry,
        ast: &Ast,
    ) -> CompileNodeResult {
        use Op::*;

        let AstTry {
            try_block,
            catch_arg,
            catch_block,
            finally_block,
        } = &try_expression;

        let try_node = ast.node(*try_block);

        let result = self.get_result_register(result_register)?;

        // The argument register for the catch block needs to be assigned now
        // so that it can be included in the TryStart op.
        let (catch_register, pop_catch_register) = match &ast.node(*catch_arg).node {
            Node::Id(id) => (self.assign_local_register(*id)?, false),
            Node::Wildcard(_) => {
                // The catch argument is being ignored, so just use a dummy register
                (self.push_register()?, true)
            }
            unexpected => {
                return compiler_error!(
                    self,
                    "Expected ID or wildcard as catch arg, found {}",
                    unexpected
                );
            }
        };

        self.push_op(TryStart, &[catch_register]);
        // The catch block start point is defined via an offset from the current byte
        let catch_offset = self.push_offset_placeholder();

        let try_result_register = match result {
            Some(result) if finally_block.is_none() => ResultRegister::Fixed(result.register),
            _ => ResultRegister::None,
        };

        self.compile_node(try_result_register, try_node, ast)?;

        // Clear the catch point at the end of the try block
        // - if the end of the try block has been reached then the catch block is no longer needed.
        self.push_op_without_span(TryEnd, &[]);
        // jump to the finally block
        self.push_op_without_span(Jump, &[]);

        let finally_offset = self.push_offset_placeholder();
        self.update_offset_placeholder(catch_offset)?;

        let catch_node = ast.node(*catch_block);
        self.span_stack.push(*ast.span(catch_node.span));

        // Clear the catch point at the start of the catch block
        // - if the catch block has been entered, then it needs to be de-registered in case there
        //   are errors thrown in the catch block.
        self.push_op(TryEnd, &[]);

        self.compile_node(try_result_register, catch_node, ast)?;
        self.span_stack.pop();

        if pop_catch_register {
            self.pop_register()?;
        }

        self.update_offset_placeholder(finally_offset)?;
        if let Some(finally_block) = finally_block {
            // If there's a finally block then the result of the expression is derived from there
            let finally_result_register = match result {
                Some(result) => ResultRegister::Fixed(result.register),
                _ => ResultRegister::None,
            };
            self.compile_node(finally_result_register, ast.node(*finally_block), ast)
        } else {
            Ok(result)
        }
    }

    fn compile_unary_op(
        &mut self,
        result_register: ResultRegister,
        op: AstUnaryOp,
        value: AstIndex,
        ast: &Ast,
    ) -> CompileNodeResult {
        let result = self.get_result_register(result_register)?;

        let value_register = self
            .compile_node(ResultRegister::Any, ast.node(value), ast)?
            .unwrap();

        if let Some(result) = result {
            let op_code = match op {
                AstUnaryOp::Negate => Op::Negate,
                AstUnaryOp::Not => Op::Not,
            };

            self.push_op(op_code, &[result.register, value_register.register]);
        }

        if value_register.is_temporary {
            self.pop_register()?;
        }

        Ok(result)
    }

    fn compile_binary_op(
        &mut self,
        result_register: ResultRegister,
        op: AstBinaryOp,
        lhs: AstIndex,
        rhs: AstIndex,
        ast: &Ast,
    ) -> CompileNodeResult {
        use AstBinaryOp::*;

        let lhs_node = ast.node(lhs);
        let rhs_node = ast.node(rhs);

        match op {
            Add | Subtract | Multiply | Divide | Remainder => {
                self.compile_arithmetic_op(result_register, op, lhs_node, rhs_node, ast)
            }
            AddAssign | SubtractAssign | MultiplyAssign | DivideAssign | RemainderAssign => {
                self.compile_arithmetic_assign_op(result_register, op, lhs_node, rhs_node, ast)
            }
            Less | LessOrEqual | Greater | GreaterOrEqual | Equal | NotEqual => {
                self.compile_comparison_op(result_register, op, lhs_node, rhs_node, ast)
            }
            And | Or => self.compile_logic_op(result_register, op, lhs, rhs, ast),
            Pipe => self.compile_piped_call(result_register, lhs, rhs, ast),
        }
    }

    fn compile_arithmetic_op(
        &mut self,
        result_register: ResultRegister,
        op: AstBinaryOp,
        lhs_node: &AstNode,
        rhs_node: &AstNode,
        ast: &Ast,
    ) -> CompileNodeResult {
        use AstBinaryOp::*;

        let op = match op {
            Add => Op::Add,
            Subtract => Op::Subtract,
            Multiply => Op::Multiply,
            Divide => Op::Divide,
            Remainder => Op::Remainder,
            _ => return compiler_error!(self, "Internal error: invalid op"),
        };

        let result = match self.get_result_register(result_register)? {
            Some(result) => {
                let lhs = self
                    .compile_node(ResultRegister::Any, lhs_node, ast)?
                    .ok_or_else(|| self.make_error("Missing lhs for binary op".into()))?;
                let rhs = self
                    .compile_node(ResultRegister::Any, rhs_node, ast)?
                    .ok_or_else(|| self.make_error("Missing rhs for binary op".into()))?;

                self.push_op(op, &[result.register, lhs.register, rhs.register]);

                if lhs.is_temporary {
                    self.pop_register()?;
                }
                if rhs.is_temporary {
                    self.pop_register()?;
                }

                Some(result)
            }
            None => {
                self.compile_node(ResultRegister::None, lhs_node, ast)?;
                self.compile_node(ResultRegister::None, rhs_node, ast)?;
                None
            }
        };

        Ok(result)
    }

    fn compile_arithmetic_assign_op(
        &mut self,
        result_register: ResultRegister,
        ast_op: AstBinaryOp,
        lhs_node: &AstNode,
        rhs_node: &AstNode,
        ast: &Ast,
    ) -> CompileNodeResult {
        use AstBinaryOp::*;

        let op = match ast_op {
            AddAssign => Op::AddAssign,
            SubtractAssign => Op::SubtractAssign,
            MultiplyAssign => Op::MultiplyAssign,
            DivideAssign => Op::DivideAssign,
            RemainderAssign => Op::RemainderAssign,
            _ => return compiler_error!(self, "Internal error: invalid op"),
        };

        let result = self.get_result_register(result_register)?;

        let rhs = self
            .compile_node(ResultRegister::Any, rhs_node, ast)?
            .ok_or_else(|| self.make_error("Missing rhs for binary op".into()))?;

        let result = if let Node::Lookup(lookup_node) = &lhs_node.node {
            self.compile_lookup(
                result_register,
                lookup_node,
                None,
                Some(rhs.register),
                Some(op),
                ast,
            )?
        } else {
            let lhs = self
                .compile_node(ResultRegister::Any, lhs_node, ast)?
                .ok_or_else(|| self.make_error("Missing lhs for binary op".into()))?;

            self.push_op(op, &[lhs.register, rhs.register]);

            let result = if let Some(result) = result {
                self.push_op(Op::Copy, &[result.register, lhs.register]);
                Some(result)
            } else {
                None
            };

            if lhs.is_temporary {
                self.pop_register()?;
            }

            result
        };

        if rhs.is_temporary {
            self.pop_register()?;
        }

        Ok(result)
    }

    fn compile_comparison_op(
        &mut self,
        result_register: ResultRegister,
        ast_op: AstBinaryOp,
        lhs: &AstNode,
        rhs: &AstNode,
        ast: &Ast,
    ) -> CompileNodeResult {
        use AstBinaryOp::*;

        let get_comparision_op = |ast_op| {
            Ok(match ast_op {
                Less => Op::Less,
                LessOrEqual => Op::LessOrEqual,
                Greater => Op::Greater,
                GreaterOrEqual => Op::GreaterOrEqual,
                Equal => Op::Equal,
                NotEqual => Op::NotEqual,
                _ => return Err("Internal error: invalid comparison op".to_string()),
            })
        };

        let result = self.get_result_register(result_register)?;

        let stack_count = self.frame().register_stack.len();

        let comparison_register = match result {
            Some(result) => result.register,
            None => self.push_register()?,
        };

        let mut jump_offsets = Vec::new();

        let mut lhs_register = self
            .compile_node(ResultRegister::Any, lhs, ast)?
            .unwrap()
            .register;
        let mut rhs = rhs;
        let mut ast_op = ast_op;

        while let Node::BinaryOp {
            op: rhs_ast_op,
            lhs: rhs_lhs,
            rhs: rhs_rhs,
        } = rhs.node
        {
            match rhs_ast_op {
                Less | LessOrEqual | Greater | GreaterOrEqual | Equal | NotEqual => {
                    // If the rhs is also a comparison, then chain the operations.
                    // e.g.
                    //   `a < (b < c)`
                    // needs to become equivalent to:
                    //   `(a < b) and (b < c)`
                    // To achieve this,
                    //   - use the lhs of the rhs as the rhs of the current operation
                    //   - use the temp value as the lhs for the current operation
                    //   - chain the two comparisons together with an And

                    let rhs_lhs_register = self
                        .compile_node(ResultRegister::Any, ast.node(rhs_lhs), ast)?
                        .unwrap()
                        .register;

                    // Place the lhs comparison result in the comparison_register
                    let op = get_comparision_op(ast_op).map_err(|e| self.make_error(e))?;
                    self.push_op(op, &[comparison_register, lhs_register, rhs_lhs_register]);

                    // Skip evaluating the rhs if the lhs result is false
                    self.push_op(Op::JumpIfFalse, &[comparison_register]);
                    jump_offsets.push(self.push_offset_placeholder());

                    lhs_register = rhs_lhs_register;
                    rhs = ast.node(rhs_rhs);
                    ast_op = rhs_ast_op;
                }
                _ => break,
            }
        }

        // Compile the rhs for the final rhs in the comparison chain
        let rhs_register = self
            .compile_node(ResultRegister::Any, rhs, ast)?
            .unwrap()
            .register;

        // We only need to perform the final comparison if there's a result register
        if let Some(result) = result {
            let op = get_comparision_op(ast_op).map_err(|e| self.make_error(e))?;
            self.push_op(op, &[result.register, lhs_register, rhs_register]);
        }

        for jump_offset in jump_offsets.iter() {
            self.update_offset_placeholder(*jump_offset)?;
        }

        self.truncate_register_stack(stack_count)?;

        Ok(result)
    }

    fn compile_logic_op(
        &mut self,
        result_register: ResultRegister,
        op: AstBinaryOp,
        lhs: AstIndex,
        rhs: AstIndex,
        ast: &Ast,
    ) -> CompileNodeResult {
        let result = self.get_result_register(result_register)?;

        // A register is needed to perform the jump,
        // so if there's no result register use a temporary register
        let register = match result {
            Some(result) => result.register,
            None => self.push_register()?,
        };

        self.compile_node(ResultRegister::Fixed(register), ast.node(lhs), ast)?;

        let jump_op = match op {
            AstBinaryOp::And => Op::JumpIfFalse,
            AstBinaryOp::Or => Op::JumpIfTrue,
            _ => unreachable!(),
        };

        self.push_op(jump_op, &[register]);

        // If the lhs caused a jump then that's the result, otherwise the result is the rhs
        self.compile_node_with_jump_offset(ResultRegister::Fixed(register), ast.node(rhs), ast)?;

        if result.is_none() {
            self.pop_register()?;
        }

        Ok(result)
    }

    fn compile_string(
        &mut self,
        result_register: ResultRegister,
        nodes: &[StringNode],
        ast: &Ast,
    ) -> CompileNodeResult {
        let result = self.get_result_register(result_register)?;

        let size_hint = nodes.iter().fold(0, |result, node| {
            match node {
                StringNode::Literal(constant_index) => {
                    result + ast.constants().get_str(*constant_index).len()
                }
                StringNode::Expr(_) => {
                    // Q. Why use '1' here?
                    // A. The expression can result in a displayed string of any length,
                    //    We can make an assumption that the expression will almost always produce
                    //    at least 1 character to display, but it's unhealthy to over-allocate so
                    //    let's leave it there for now until we have real-world practice that tells
                    //    us otherwise.
                    result + 1
                }
            }
        });

        match nodes {
            [] => return compiler_error!(self, "compile_string: Missing string nodes"),
            [StringNode::Literal(constant_index)] => {
                if let Some(result) = result {
                    self.compile_load_string_constant(result.register, *constant_index);
                }
            }
            _ => {
                if let Some(result) = result {
                    if size_hint <= u8::MAX as usize {
                        self.push_op(Op::StringStart, &[result.register, size_hint as u8]);
                    } else {
                        // Limit the size hint to u32::MAX, u64 size hinting can be added later if
                        // it would be useful in practice.
                        let size_hint = size_hint.min(u32::MAX as usize) as u32;
                        self.push_op(Op::StringStart32, &[result.register]);
                        self.push_bytes(&size_hint.to_le_bytes());
                    }
                }

                for node in nodes.iter() {
                    match node {
                        StringNode::Literal(constant_index) => {
                            if let Some(result) = result {
                                let node_register = self.push_register()?;

                                self.compile_load_string_constant(node_register, *constant_index);
                                self.push_op_without_span(
                                    Op::StringPush,
                                    &[result.register, node_register],
                                );

                                self.pop_register()?;
                            }
                        }
                        StringNode::Expr(expression_node) => {
                            if let Some(result) = result {
                                let expression_result = self
                                    .compile_node(
                                        ResultRegister::Any,
                                        ast.node(*expression_node),
                                        ast,
                                    )?
                                    .unwrap();

                                self.push_op_without_span(
                                    Op::StringPush,
                                    &[result.register, expression_result.register],
                                );

                                if expression_result.is_temporary {
                                    self.pop_register()?;
                                }
                            } else {
                                // Compile the expression even though we don't need the result,
                                // so that side-effects can take place.
                                self.compile_node(
                                    ResultRegister::None,
                                    ast.node(*expression_node),
                                    ast,
                                )?;
                            }
                        }
                    }
                }

                if let Some(result) = result {
                    self.push_op(Op::StringFinish, &[result.register]);
                }
            }
        }

        Ok(result)
    }

    fn compile_make_temp_tuple(
        &mut self,
        result_register: ResultRegister,
        elements: &[AstIndex],
        ast: &Ast,
    ) -> CompileNodeResult {
        let result = match self.get_result_register(result_register)? {
            Some(result) => {
                for element in elements.iter() {
                    let element_register = self.push_register()?;
                    self.compile_node(
                        ResultRegister::Fixed(element_register),
                        ast.node(*element),
                        ast,
                    )?;
                }

                let start_register = self.peek_register(elements.len() - 1)?;

                self.push_op(
                    Op::MakeTempTuple,
                    &[result.register, start_register, elements.len() as u8],
                );

                // If we're making a temp tuple then the registers need to be kept around,
                // and they should be removed by the caller.

                Some(result)
            }
            None => {
                // Compile the element nodes for side-effects
                for element in elements.iter() {
                    self.compile_node(ResultRegister::None, ast.node(*element), ast)?;
                }

                None
            }
        };

        Ok(result)
    }

    fn compile_make_sequence(
        &mut self,
        result_register: ResultRegister,
        elements: &[AstIndex],
        finish_op: Op,
        ast: &Ast,
    ) -> CompileNodeResult {
        use Op::*;

        let result = match self.get_result_register(result_register)? {
            Some(result) => {
                match elements.len() {
                    size_hint if size_hint <= u8::MAX as usize => {
                        self.push_op(SequenceStart, &[result.register, size_hint as u8]);
                    }
                    size_hint if size_hint <= u32::MAX as usize => {
                        self.push_op(SequenceStart32, &[result.register]);
                        self.push_bytes(&(size_hint as u32).to_le_bytes());
                    }
                    overflow => {
                        return compiler_error!(
                            self,
                            "Too many list elements, {} is greater than the maximum of {}",
                            overflow,
                            u32::MAX
                        );
                    }
                }

                match elements {
                    [] => {}
                    [single_element] => {
                        let element = self
                            .compile_node(ResultRegister::Any, ast.node(*single_element), ast)?
                            .unwrap();
                        self.push_op_without_span(
                            SequencePush,
                            &[result.register, element.register],
                        );
                        if element.is_temporary {
                            self.pop_register()?;
                        }
                    }
                    _ => {
                        let max_batch_size = self.frame().available_registers_count() as usize;
                        for elements_batch in elements.chunks(max_batch_size) {
                            let stack_count = self.frame().register_stack.len();
                            let start_register = self.frame().next_temporary_register();

                            for element_node in elements_batch {
                                let element_register = self.push_register()?;
                                self.compile_node(
                                    ResultRegister::Fixed(element_register),
                                    ast.node(*element_node),
                                    ast,
                                )?;
                            }

                            self.push_op_without_span(
                                SequencePushN,
                                &[result.register, start_register, elements_batch.len() as u8],
                            );

                            self.truncate_register_stack(stack_count)?;
                        }
                    }
                }

                // Now that the elements have been added to the sequence builder,
                // add the finishing op.
                self.push_op(finish_op, &[result.register]);

                Some(result)
            }
            None => {
                // Compile the element nodes for side-effects
                for element_node in elements.iter() {
                    self.compile_node(ResultRegister::None, ast.node(*element_node), ast)?;
                }
                None
            }
        };

        Ok(result)
    }

    fn compile_make_map(
        &mut self,
        result_register: ResultRegister,
        entries: &[(MapKey, Option<AstIndex>)],
        ast: &Ast,
    ) -> CompileNodeResult {
        use Op::*;

        let result = match self.get_result_register(result_register)? {
            Some(result) => {
                match entries.len() {
                    size_hint if size_hint <= u8::MAX as usize => {
                        self.push_op(MakeMap, &[result.register, size_hint as u8]);
                    }
                    size_hint if size_hint <= u32::MAX as usize => {
                        self.push_op(MakeMap32, &[result.register]);
                        self.push_bytes(&(size_hint as u32).to_le_bytes());
                    }
                    overflow => {
                        return compiler_error!(
                            self,
                            "Too many map entries, {} is greater than the maximum of {}",
                            overflow,
                            u32::MAX
                        );
                    }
                }

                for (key, maybe_value_node) in entries.iter() {
                    let value = match (key, maybe_value_node) {
                        (_, Some(value_node)) => {
                            let value_node = ast.node(*value_node);
                            self.compile_node(ResultRegister::Any, value_node, ast)?
                                .unwrap()
                        }
                        (MapKey::Id(id), None) => {
                            match self.frame().get_local_assigned_register(*id) {
                                Some(register) => CompileResult::with_assigned(register),
                                None => {
                                    let register = self.push_register()?;
                                    self.compile_load_non_local(register, *id);
                                    CompileResult::with_temporary(register)
                                }
                            }
                        }
                        _ => return compiler_error!(self, "Value missing for map key"),
                    };

                    self.compile_map_insert(result.register, value.register, key, ast)?;

                    if value.is_temporary {
                        self.pop_register()?;
                    }
                }

                Some(result)
            }
            None => {
                // Compile the value nodes for side-effects
                for (_key, value_node) in entries.iter() {
                    if let Some(value_node) = value_node {
                        self.compile_node(ResultRegister::None, ast.node(*value_node), ast)?;
                    }
                }

                None
            }
        };

        Ok(result)
    }

    fn compile_function(
        &mut self,
        result_register: ResultRegister,
        function: &Function,
        ast: &Ast,
    ) -> CompileNodeResult {
        use Op::*;

        if let Some(result) = self.get_result_register(result_register)? {
            let arg_count = match u8::try_from(function.args.len()) {
                Ok(x) => x,
                Err(_) => {
                    return compiler_error!(
                        self,
                        "Function has too many arguments: {}",
                        function.args.len()
                    );
                }
            };

            let captures = self
                .frame()
                .captures_for_nested_frame(&function.accessed_non_locals);
            if captures.len() > u8::MAX as usize {
                return compiler_error!(
                    self,
                    "Function captures too many values: {}",
                    captures.len(),
                );
            }
            let capture_count = captures.len() as u8;

            let arg_is_unpacked_tuple = matches!(
                function.args.as_slice(),
                &[single_arg] if matches!(ast.node(single_arg).node, Node::Tuple(_))
            );

            let flags_byte = FunctionFlags {
                instance_function: function.is_instance_function,
                variadic: function.is_variadic,
                generator: function.is_generator,
                arg_is_unpacked_tuple,
            }
            .as_byte();

            if flags_byte == 0 && capture_count == 0 {
                self.push_op(SimpleFunction, &[result.register, arg_count]);
            } else {
                self.push_op(
                    Function,
                    &[result.register, arg_count, capture_count, flags_byte],
                );
            }

            let function_size_ip = self.push_offset_placeholder();

            let local_count = match u8::try_from(function.local_count) {
                Ok(x) => x,
                Err(_) => {
                    return compiler_error!(
                        self,
                        "Function has too many locals: {}",
                        function.args.len()
                    );
                }
            };

            let allow_implicit_return = !function.is_generator;

            let body_as_slice = [function.body];

            let function_body = match &ast.node(function.body).node {
                Node::Block(expressions) => expressions.as_slice(),
                _ => &body_as_slice,
            };

            self.compile_frame(
                local_count,
                function_body,
                &function.args,
                &captures,
                ast,
                allow_implicit_return,
            )?;

            self.update_offset_placeholder(function_size_ip)?;

            for (i, capture) in captures.iter().enumerate() {
                match self
                    .frame()
                    .get_local_assigned_or_reserved_register(*capture)
                {
                    AssignedOrReserved::Assigned(assigned_register) => {
                        self.push_op(Capture, &[result.register, i as u8, assigned_register]);
                    }
                    AssignedOrReserved::Reserved(reserved_register) => {
                        let capture_span = self.span();
                        self.frame_mut()
                            .defer_op_until_register_is_committed(
                                reserved_register,
                                vec![Capture as u8, result.register, i as u8, reserved_register],
                                capture_span,
                            )
                            .map_err(|e| self.make_error(e))?;
                    }
                    AssignedOrReserved::Unassigned => {
                        let capture_register = self.push_register()?;
                        self.compile_load_non_local(capture_register, *capture);
                        self.push_op(Capture, &[result.register, i as u8, capture_register]);
                        self.pop_register()?;
                    }
                }
            }

            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    // Compiles a lookup chain
    //
    // The lookup chain is a linked list of LookupNodes stored as AST indices.
    //
    // The loop keeps track of the temporary values that are the result of each lookup step.
    //
    // piped_arg_register - used when a value is being piped into the lookup,
    //   e.g. `f x >> foo.bar 123`, should be equivalent to `foo.bar 123, (f x)`
    //
    // rhs - used when assigning to the result of a lookup,
    //   e.g. `foo.bar += 42`, or `foo[123] = bar`
    // rhs_op - If present, then the op should be applied to the result of the lookup.
    fn compile_lookup(
        &mut self,
        result_register: ResultRegister,
        (root_node, mut next_node_index): &(LookupNode, Option<AstIndex>),
        piped_arg_register: Option<u8>,
        rhs: Option<u8>,
        rhs_op: Option<Op>,
        ast: &Ast,
    ) -> CompileNodeResult {
        use Op::*;

        if next_node_index.is_none() {
            return compiler_error!(self, "compile_lookup: missing next node index");
        }

        // If the result is going into a temporary register then assign it now as the first step.
        let result = self.get_result_register(result_register)?;

        // Keep track of a register for each lookup node.
        // This produces a chain of temporary value registers, allowing lookup operations to access
        // parent containers when needed, e.g. calls to instance functions.
        let mut node_registers = SmallVec::<[u8; 4]>::new();

        // At the end of the lookup we'll pop the whole stack,
        // so we don't need to keep track of how many temporary registers we use.
        let stack_count = self.frame().register_stack.len();
        let span_stack_count = self.span_stack.len();

        // Where should the final value in the lookup chain be placed?
        let chain_result_register = match (result, piped_arg_register, rhs_op) {
            // No result register and no piped call or assignment operation,
            // so the result of the lookup chain isn't needed.
            (None, None, None) => None,
            // If there's a result register and no piped call, then use the result register
            (Some(result), None, _) => Some(result.register),
            // If there's a piped call after the lookup chain, or an assignment operation,
            // then place the result of the lookup chain in a temporary register.
            _ => Some(self.push_register()?),
        };

        let mut lookup_node = root_node.clone();

        while next_node_index.is_some() {
            match &lookup_node {
                LookupNode::Root(root_node) => {
                    if !node_registers.is_empty() {
                        return compiler_error!(self, "Root lookup node not in root position");
                    }

                    let root = self
                        .compile_node(ResultRegister::Any, ast.node(*root_node), ast)?
                        .unwrap();
                    node_registers.push(root.register);
                }
                LookupNode::Id(id) => {
                    // Access by id
                    // e.g. x.foo()
                    //    - x = Root
                    //    - foo = Id
                    //    - () = Call

                    let parent_register = match node_registers.last() {
                        Some(register) => *register,
                        None => return compiler_error!(self, "Child lookup node in root position"),
                    };

                    let node_register = self.push_register()?;
                    node_registers.push(node_register);
                    self.compile_access_id(node_register, parent_register, *id)?;
                }
                LookupNode::Str(ref lookup_string) => {
                    // Access by string
                    // e.g. x."123"()
                    //    - x = Root
                    //    - "123" = Str
                    //    - () = Call

                    let parent_register = match node_registers.last() {
                        Some(register) => *register,
                        None => return compiler_error!(self, "Child lookup node in root position"),
                    };

                    let node_register = self.push_register()?;
                    let key_register = self.push_register()?;
                    node_registers.push(node_register);
                    // TODO use compile_access_string
                    self.compile_string(
                        ResultRegister::Fixed(key_register),
                        &lookup_string.nodes,
                        ast,
                    )?;
                    self.push_op(
                        AccessString,
                        &[node_register, parent_register, key_register],
                    );
                    self.pop_register()?; // key_register
                }
                LookupNode::Index(index_node) => {
                    // Indexing into a value
                    // e.g. foo.bar[123]
                    //    - foo = Root
                    //    - bar = Id
                    //    - [123] = Index, with 123 as index node

                    let parent_register = match node_registers.last() {
                        Some(register) => *register,
                        None => return compiler_error!(self, "Child lookup node in root position"),
                    };

                    let index = self
                        .compile_node(ResultRegister::Any, ast.node(*index_node), ast)?
                        .unwrap();

                    let node_register = self.push_register()?;
                    node_registers.push(node_register);
                    self.push_op(Index, &[node_register, parent_register, index.register]);
                }
                LookupNode::Call { args, .. } => {
                    // Function call on a lookup result

                    let (parent_register, function_register) = match &node_registers.as_slice() {
                        [.., parent, function] => (Some(*parent), *function),
                        [function] => (None, *function),
                        [] => unreachable!(),
                    };

                    // Not in the last node, so for the lookup chain to continue,
                    // use a temporary register for the call result.
                    let call_result_register = self.push_register()?;
                    node_registers.push(call_result_register);

                    self.compile_call(
                        ResultRegister::Fixed(call_result_register),
                        function_register,
                        args,
                        None,
                        parent_register,
                        ast,
                    )?;
                }
            }

            // Is the lookup chain complete?
            if let Some(next) = next_node_index {
                let next_lookup_node = ast.node(next);

                match &next_lookup_node.node {
                    Node::Lookup((node, next)) => {
                        lookup_node = node.clone();
                        next_node_index = *next;
                    }
                    other => {
                        return compiler_error!(
                            self,
                            "compile_lookup: invalid node in lookup chain, found {}",
                            other
                        )
                    }
                };

                self.span_stack.push(*ast.span(next_lookup_node.span));
            } else {
                break;
            }
        }

        // The lookup chain is complete, now we need to handle:
        //   - accessing and assigning to lookup entries
        //   - calling functions
        let last_node = lookup_node;

        let access_register = chain_result_register.unwrap_or_default();
        let Some(&parent_register) = node_registers.last() else {
            return compiler_error!(self, "compile_lookup: Missing parent register");
        };

        // If rhs_op is Some, then rhs should also be Some
        debug_assert!(rhs_op.is_none() || rhs_op.is_some() && rhs.is_some());

        let simple_assignment = rhs.is_some() && rhs_op.is_none();
        let access_assignment = rhs.is_some() && rhs_op.is_some();

        let string_key = if let LookupNode::Str(lookup_string) = &last_node {
            let key_register = self.push_register()?;
            self.compile_string(
                ResultRegister::Fixed(key_register),
                &lookup_string.nodes,
                ast,
            )?
        } else {
            None
        };

        let index = if let LookupNode::Index(index_node) = last_node {
            self.compile_node(ResultRegister::Any, ast.node(index_node), ast)?
        } else {
            None
        };

        // Do we need to access the value?
        // Yes if the rhs_op is Some
        // If rhs_op is None, then Yes if rhs is also None (simple access)
        // If rhs is Some and rhs_op is None, then it's a simple assignment
        match &last_node {
            LookupNode::Id(id) if !simple_assignment => {
                self.compile_access_id(access_register, parent_register, *id)?;
                node_registers.push(access_register);
            }
            LookupNode::Str(_) if !simple_assignment => {
                self.push_op(
                    AccessString,
                    &[
                        access_register,
                        parent_register,
                        string_key.unwrap().register, // Guaranteed to be Some
                    ],
                );
                node_registers.push(access_register);
            }
            LookupNode::Index(_) if !simple_assignment => {
                self.push_op(
                    Index,
                    &[
                        access_register,
                        parent_register,
                        index.unwrap().register, // Guaranteed to be Some
                    ],
                );
                node_registers.push(access_register);
            }
            LookupNode::Call { args, with_parens } => {
                if simple_assignment {
                    return compiler_error!(self, "Assigning to temporary value");
                } else if access_assignment || piped_arg_register.is_none() || *with_parens {
                    let (parent_register, function_register) = match &node_registers.as_slice() {
                        [.., parent, function] => (Some(*parent), *function),
                        [function] => (None, *function),
                        [] => unreachable!(),
                    };

                    let call_result_register = match chain_result_register {
                        Some(result_register) => {
                            node_registers.push(result_register);
                            ResultRegister::Fixed(result_register)
                        }
                        None => ResultRegister::None,
                    };

                    self.compile_call(
                        call_result_register,
                        function_register,
                        args,
                        None,
                        parent_register,
                        ast,
                    )?;
                }
            }
            _ => {}
        }

        // Do we need to modify the accessed value?
        if access_assignment {
            let Some(rhs) = rhs else {
                return compiler_error!(self, "compile_lookup: Missing rhs")
            };
            let Some(rhs_op) = rhs_op else {
                return compiler_error!(self, "compile_lookup: Missing rhs_op")
            };

            self.push_op(rhs_op, &[access_register, rhs]);
            node_registers.push(access_register);
        }

        // Do we need to assign a value to the last node in the lookup?
        if access_assignment || simple_assignment {
            let value_register = if simple_assignment {
                rhs.unwrap()
            } else {
                access_register
            };

            match &last_node {
                LookupNode::Id(id) => {
                    self.compile_map_insert(
                        parent_register,
                        value_register,
                        &MapKey::Id(*id),
                        ast,
                    )?;
                }
                LookupNode::Str(_) => {
                    self.push_op(
                        MapInsert,
                        &[
                            parent_register,
                            string_key.unwrap().register, // Guaranteed to be Some
                            value_register,
                        ],
                    );
                }
                LookupNode::Index(_) => {
                    self.push_op(
                        SetIndex,
                        &[
                            parent_register,
                            index.unwrap().register, // Guaranteed to be Some
                            value_register,
                        ],
                    );
                }
                _ => {}
            }
        }

        // As a final step, do we need to make a piped call to the result of the lookup?
        if piped_arg_register.is_some() {
            let piped_call_args = match last_node {
                LookupNode::Call { args, with_parens } if !with_parens => args,
                _ => Vec::new(),
            };

            let (parent_register, function_register) = match &node_registers.as_slice() {
                [.., parent, function] => (Some(*parent), *function),
                [function] => (None, *function),
                [] => unreachable!(),
            };

            let call_result = if let Some(result) = result {
                ResultRegister::Fixed(result.register)
            } else {
                ResultRegister::None
            };

            self.compile_call(
                call_result,
                function_register,
                &piped_call_args,
                piped_arg_register,
                parent_register,
                ast,
            )?;
        }

        self.span_stack.truncate(span_stack_count);
        self.truncate_register_stack(stack_count)?;

        Ok(result)
    }

    fn compile_map_insert(
        &mut self,
        map_register: u8,
        value_register: u8,
        key: &MapKey,
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        use Op::*;

        match key {
            MapKey::Id(id) => {
                let key_register = self.push_register()?;
                self.compile_load_string_constant(key_register, *id);
                self.push_op_without_span(MapInsert, &[map_register, key_register, value_register]);
                self.pop_register()?;
            }
            MapKey::Str(string) => {
                let key_register = self.push_register()?;
                self.compile_string(ResultRegister::Fixed(key_register), &string.nodes, ast)?;
                self.push_op_without_span(MapInsert, &[map_register, key_register, value_register]);
                self.pop_register()?;
            }
            MapKey::Meta(key, name) => {
                let key = *key as u8;
                if let Some(name) = name {
                    let name_register = self.push_register()?;
                    self.compile_load_string_constant(name_register, *name);
                    self.push_op_without_span(
                        MetaInsertNamed,
                        &[map_register, key, name_register, value_register],
                    );
                    self.pop_register()?;
                } else {
                    self.push_op_without_span(MetaInsert, &[map_register, key, value_register]);
                }
            }
        }

        Ok(())
    }

    fn compile_access_id(
        &mut self,
        result: u8,
        value: u8,
        key: ConstantIndex,
    ) -> Result<(), CompilerError> {
        use Op::*;

        match key.bytes() {
            [byte1, 0, 0] => self.push_op(Access, &[result, value, byte1]),
            [byte1, byte2, 0] => self.push_op(Access16, &[result, value, byte1, byte2]),
            [byte1, byte2, byte3] => self.push_op(Access24, &[result, value, byte1, byte2, byte3]),
        }

        Ok(())
    }

    fn compile_access_string(
        &mut self,
        result_register: u8,
        value_register: u8,
        key_string_nodes: &[StringNode],
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        let key_register = self.push_register()?;
        self.compile_string(ResultRegister::Fixed(key_register), key_string_nodes, ast)?;
        self.push_op(
            Op::AccessString,
            &[result_register, value_register, key_register],
        );
        self.pop_register()?;
        Ok(())
    }

    // Compiles a node like `f x >> g`, compiling the lhs as the last arg for a call on the rhs
    fn compile_piped_call(
        &mut self,
        result_register: ResultRegister,
        lhs: AstIndex,
        rhs: AstIndex,
        ast: &Ast,
    ) -> CompileNodeResult {
        // First things first, if a temporary result register is to be used, assign it now.
        let result = self.get_result_register(result_register)?;

        // The piped call should either go into the specified register, or it can be ignored
        let call_result_register = if let Some(result) = result {
            ResultRegister::Fixed(result.register)
        } else {
            ResultRegister::None
        };

        // Next, compile the LHS to produce the value that should be piped into the call
        let piped_value = self
            .compile_node(ResultRegister::Any, ast.node(lhs), ast)?
            .unwrap();

        let rhs_node = ast.node(rhs);
        let result = match &rhs_node.node {
            Node::NamedCall { id, args } => self.compile_named_call(
                call_result_register,
                *id,
                args,
                Some(piped_value.register),
                ast,
            ),
            Node::Id(id) => {
                // Compile a call with the piped arg using the id to access the function
                self.compile_named_call(result_register, *id, &[], Some(piped_value.register), ast)
            }
            Node::Lookup(lookup_node) => {
                // Compile the lookup, passing in the piped call arg, which will either be appended
                // to call args at the end of a lookup, or the last node will be turned into a call.
                self.compile_lookup(
                    call_result_register,
                    lookup_node,
                    Some(piped_value.register),
                    None,
                    None,
                    ast,
                )
            }
            _ => {
                // If the RHS is none of the above, then compile it assuming that the result will
                // be a function.
                let function = self
                    .compile_node(ResultRegister::Any, rhs_node, ast)?
                    .unwrap();
                let result = self.compile_call(
                    call_result_register,
                    function.register,
                    &[],
                    Some(piped_value.register),
                    None,
                    ast,
                )?;
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

    fn compile_named_call(
        &mut self,
        result_register: ResultRegister,
        function_id: ConstantIndex,
        args: &[AstIndex],
        piped_arg: Option<u8>,
        ast: &Ast,
    ) -> CompileNodeResult {
        if let Some(function_register) = self.frame().get_local_assigned_register(function_id) {
            self.compile_call(
                result_register,
                function_register,
                args,
                piped_arg,
                None,
                ast,
            )
        } else {
            let result = self.get_result_register(result_register)?;
            let call_result_register = if let Some(result) = result {
                ResultRegister::Fixed(result.register)
            } else {
                ResultRegister::None
            };

            let function_register = self.push_register()?;
            self.compile_load_non_local(function_register, function_id);

            self.compile_call(
                call_result_register,
                function_register,
                args,
                piped_arg,
                None,
                ast,
            )?;

            self.pop_register()?; // function_register
            Ok(result)
        }
    }

    fn compile_call(
        &mut self,
        result_register: ResultRegister,
        function_register: u8,
        args: &[AstIndex],
        piped_arg: Option<u8>,
        instance: Option<u8>,
        ast: &Ast,
    ) -> CompileNodeResult {
        use Op::*;

        let result = self.get_result_register(result_register)?;
        let stack_count = self.frame().register_stack.len();

        // The frame base is an empty register that may be used for an instance value if needed
        // (it's decided at runtime if the instance value will be used or not).
        let frame_base = self.push_register()?;

        let mut arg_count = args.len();

        for arg in args.iter() {
            let arg_register = self.push_register()?;
            self.compile_node(ResultRegister::Fixed(arg_register), ast.node(*arg), ast)?;
        }

        if let Some(piped_arg) = piped_arg {
            arg_count += 1;
            let arg_register = self.push_register()?;
            self.push_op(Copy, &[arg_register, piped_arg]);
        }

        let call_result_register = if let Some(result) = result {
            result.register
        } else {
            // The result isn't needed, so it can be placed in the frame's base register
            // (which isn't needed post-call).
            // An alternative here could be to have CallNoResult ops, but this will do for now.
            frame_base
        };

        match instance {
            Some(instance_register) => {
                self.push_op(
                    CallInstance,
                    &[
                        call_result_register,
                        function_register,
                        frame_base,
                        arg_count as u8,
                        instance_register,
                    ],
                );
            }
            None => {
                self.push_op(
                    Call,
                    &[
                        call_result_register,
                        function_register,
                        frame_base,
                        arg_count as u8,
                    ],
                );
            }
        }

        self.truncate_register_stack(stack_count)?;

        Ok(result)
    }

    fn compile_if(
        &mut self,
        result_register: ResultRegister,
        ast_if: &AstIf,
        ast: &Ast,
    ) -> CompileNodeResult {
        use Op::*;

        let AstIf {
            condition,
            then_node,
            else_if_blocks,
            else_node,
        } = ast_if;

        let result = self.get_result_register(result_register)?;
        let expression_result_register = if let Some(result) = result {
            ResultRegister::Fixed(result.register)
        } else {
            ResultRegister::None
        };

        // If
        let condition_register = self
            .compile_node(ResultRegister::Any, ast.node(*condition), ast)?
            .unwrap();

        self.push_op_without_span(JumpIfFalse, &[condition_register.register]);
        let condition_jump_ip = self.push_offset_placeholder();

        if condition_register.is_temporary {
            self.pop_register()?;
        }

        self.compile_node(expression_result_register, ast.node(*then_node), ast)?;

        let if_jump_ip = {
            if !else_if_blocks.is_empty() || else_node.is_some() || result.is_some() {
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
            .map(
                |(else_if_condition, else_if_node)| -> Result<usize, CompilerError> {
                    let condition = self
                        .compile_node(ResultRegister::Any, ast.node(*else_if_condition), ast)?
                        .unwrap();

                    self.push_op_without_span(JumpIfFalse, &[condition.register]);
                    let conditon_jump_ip = self.push_offset_placeholder();

                    if condition.is_temporary {
                        self.pop_register()?;
                    }

                    self.compile_node(expression_result_register, ast.node(*else_if_node), ast)?;

                    self.push_op_without_span(Jump, &[]);
                    let else_if_jump_ip = self.push_offset_placeholder();

                    self.update_offset_placeholder(conditon_jump_ip)?;

                    Ok(else_if_jump_ip)
                },
            )
            .collect::<Result<Vec<_>, _>>()?;

        // Else - either compile the else block, or set the result to empty
        if let Some(else_node) = else_node {
            self.compile_node(expression_result_register, ast.node(*else_node), ast)?;
        } else if let Some(result) = result {
            self.push_op_without_span(SetNull, &[result.register]);
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
        result_register: ResultRegister,
        arms: &[SwitchArm],
        ast: &Ast,
    ) -> CompileNodeResult {
        let result = self.get_result_register(result_register)?;

        let stack_count = self.frame().register_stack.len();

        let mut result_jump_placeholders = Vec::new();

        for (arm_index, arm) in arms.iter().enumerate() {
            let is_last_arm = arm_index == arms.len() - 1;

            let arm_end_jump_placeholder = if let Some(condition) = arm.condition {
                let condition_register = self
                    .compile_node(ResultRegister::Any, ast.node(condition), ast)?
                    .unwrap();

                self.push_op_without_span(Op::JumpIfFalse, &[condition_register.register]);

                if condition_register.is_temporary {
                    self.pop_register()?;
                }

                Some(self.push_offset_placeholder())
            } else {
                None
            };

            let body_result_register = if let Some(result) = result {
                ResultRegister::Fixed(result.register)
            } else {
                ResultRegister::None
            };

            self.compile_node(body_result_register, ast.node(arm.expression), ast)?;

            if !is_last_arm {
                self.push_op_without_span(Op::Jump, &[]);
                result_jump_placeholders.push(self.push_offset_placeholder())
            }

            if let Some(jump_placeholder) = arm_end_jump_placeholder {
                self.update_offset_placeholder(jump_placeholder)?;
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
        result_register: ResultRegister,
        match_expression: AstIndex,
        arms: &[MatchArm],
        ast: &Ast,
    ) -> CompileNodeResult {
        let result = self.get_result_register(result_register)?;

        let stack_count = self.frame().register_stack.len();

        let match_node = ast.node(match_expression);
        let match_register = self
            .compile_node(ResultRegister::Any, match_node, ast)?
            .unwrap();
        let match_len = match &match_node.node {
            Node::TempTuple(expressions) => expressions.len(),
            _ => 1,
        };

        let mut result_jump_placeholders = Vec::new();

        for (arm_index, arm) in arms.iter().enumerate() {
            let is_last_arm = arm_index == arms.len() - 1;

            if let Some(placeholder) = self.compile_match_arm(
                result,
                match_register.register,
                match_len,
                arm,
                is_last_arm,
                ast,
            )? {
                result_jump_placeholders.push(placeholder);
            }
        }

        for jump_placeholder in result_jump_placeholders.iter() {
            self.update_offset_placeholder(*jump_placeholder)?;
        }

        self.truncate_register_stack(stack_count)?;

        Ok(result)
    }

    fn compile_match_arm(
        &mut self,
        result: Option<CompileResult>,
        match_register: u8,
        match_len: usize,
        arm: &MatchArm,
        is_last_arm: bool,
        ast: &Ast,
    ) -> Result<Option<usize>, CompilerError> {
        let mut jumps = MatchJumpPlaceholders::default();

        for (alternative_index, arm_pattern) in arm.patterns.iter().enumerate() {
            let is_last_alternative = alternative_index == arm.patterns.len() - 1;

            jumps.alternative_end.clear();

            let arm_node = ast.node(*arm_pattern);
            self.span_stack.push(*ast.span(arm_node.span));
            let patterns = match &arm_node.node {
                Node::TempTuple(patterns) => {
                    if patterns.len() != match_len {
                        return compiler_error!(
                            self,
                            "Expected {} patterns in match arm, found {}",
                            match_len,
                            patterns.len()
                        );
                    }

                    Some(patterns.clone())
                }
                Node::List(patterns) | Node::Tuple(patterns) => {
                    if match_len != 1 {
                        return compiler_error!(
                            self,
                            "Expected {} patterns in match arm, found 1",
                            match_len,
                        );
                    }

                    let type_check_op = if matches!(arm_node.node, Node::List(_)) {
                        Op::IsList
                    } else {
                        Op::IsTuple
                    };
                    self.compile_nested_match_arm_patterns(
                        MatchArmParameters {
                            match_register,
                            is_last_alternative,
                            has_last_pattern: true,
                            jumps: &mut jumps,
                        },
                        None, // pattern index
                        patterns,
                        type_check_op,
                        ast,
                    )?;

                    None
                }
                Node::Wildcard(_) => Some(vec![*arm_pattern]),
                _ => {
                    if match_len != 1 {
                        return compiler_error!(
                            self,
                            "Expected {} patterns in match arm, found 1",
                            match_len,
                        );
                    }
                    Some(vec![*arm_pattern])
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
                    ast,
                )?;
            }

            for jump_placeholder in jumps.alternative_end.iter() {
                self.update_offset_placeholder(*jump_placeholder)?;
            }

            self.span_stack.pop(); // arm node
        }

        // Update the match end jump placeholders before the condition
        for jump_placeholder in jumps.match_end.iter() {
            self.update_offset_placeholder(*jump_placeholder)?;
        }

        // Arm condition, e.g.
        // match foo
        //   x if x > 10 then 99
        if let Some(condition) = arm.condition {
            let condition_register = self
                .compile_node(ResultRegister::Any, ast.node(condition), ast)?
                .unwrap();

            self.push_op_without_span(Op::JumpIfFalse, &[condition_register.register]);
            jumps.arm_end.push(self.push_offset_placeholder());

            if condition_register.is_temporary {
                self.pop_register()?;
            }
        }

        let body_result_register = if let Some(result) = result {
            ResultRegister::Fixed(result.register)
        } else {
            ResultRegister::None
        };

        self.compile_node(body_result_register, ast.node(arm.expression), ast)?;

        let result_jump_placeholder = if !is_last_arm {
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
        ast: &Ast,
    ) -> Result<(), CompilerError> {
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
            let pattern_node = ast.node(*pattern);

            match &pattern_node.node {
                Node::Null
                | Node::BoolTrue
                | Node::BoolFalse
                | Node::SmallInt(_)
                | Node::Int(_)
                | Node::Float(_)
                | Node::Str(_)
                | Node::Lookup(_) => {
                    let pattern = self.push_register()?;
                    self.compile_node(ResultRegister::Fixed(pattern), pattern_node, ast)?;
                    let comparison = self.push_register()?;

                    if match_is_container {
                        let element = self.push_register()?;
                        self.push_op(
                            TempIndex,
                            &[element, params.match_register, pattern_index as u8],
                        );
                        self.push_op(Equal, &[comparison, pattern, element]);
                        self.pop_register()?; // element
                    } else {
                        self.push_op(Equal, &[comparison, pattern, params.match_register]);
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
                Node::Id(id) => {
                    let id_register = self.assign_local_register(*id)?;
                    if match_is_container {
                        self.push_op(
                            TempIndex,
                            &[id_register, params.match_register, pattern_index as u8],
                        );
                    } else {
                        self.push_op(Copy, &[id_register, params.match_register]);
                    }

                    if is_last_pattern && !params.is_last_alternative {
                        // Ids match unconditionally, so if we're at the end of a
                        // multi-expression pattern, skip over the remaining alternatives
                        self.push_op(Jump, &[]);
                        params.jumps.match_end.push(self.push_offset_placeholder());
                    }
                }
                Node::Wildcard(_) => {
                    if is_last_pattern && !params.is_last_alternative {
                        // Wildcards match unconditionally, so if we're at the end of a
                        // multi-expression pattern, skip over the remaining alternatives
                        // e.g. x, 0, _ or x, 1, y if foo x then
                        //            ^~~~~~~ We're here, jump to the if condition
                        self.push_op(Jump, &[]);
                        params.jumps.match_end.push(self.push_offset_placeholder());
                    }
                }
                Node::List(patterns) | Node::Tuple(patterns) => {
                    let type_check_op = if matches!(pattern_node.node, Node::List(_)) {
                        IsList
                    } else {
                        IsTuple
                    };
                    self.compile_nested_match_arm_patterns(
                        MatchArmParameters {
                            match_register: params.match_register,
                            is_last_alternative: params.is_last_alternative,
                            has_last_pattern: params.has_last_pattern,
                            jumps: params.jumps,
                        },
                        Some(pattern_index),
                        patterns,
                        type_check_op,
                        ast,
                    )?;
                }
                Node::Ellipsis(maybe_id) => {
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
                        return compiler_error!(
                            self,
                            "Matching with ellipsis is only allowed in first or last position"
                        );
                    }
                }
                _ => {
                    return compiler_error!(self, "Internal error: invalid match pattern");
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
        type_check_op: Op,
        ast: &Ast,
    ) -> Result<(), CompilerError> {
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

        // Check that the container has the correct type
        self.push_op(type_check_op, &[temp_register, value_register]);
        self.push_op(JumpIfFalse, &[temp_register]);
        // If the container has the wrong type, jump to the next match patterns
        if params.is_last_alternative {
            params.jumps.arm_end.push(self.push_offset_placeholder());
        } else {
            params
                .jumps
                .alternative_end
                .push(self.push_offset_placeholder());
        }

        let first_or_last_pattern_is_ellipsis = {
            let first_is_ellipsis = nested_patterns.first().map_or(false, |first| {
                matches!(ast.node(*first).node, Node::Ellipsis(_))
            });
            let last_is_ellipsis = nested_patterns.last().map_or(false, |last| {
                matches!(ast.node(*last).node, Node::Ellipsis(_))
            });
            if nested_patterns.len() > 1 && first_is_ellipsis && last_is_ellipsis {
                return compiler_error!(self, "Only one ellipsis is allowed in a match pattern");
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
            ast,
        )?;

        if pattern_index.is_some() {
            self.pop_register()?; // value_register
        }

        Ok(())
    }

    fn compile_for(
        &mut self,
        result_register: ResultRegister, // register that gets the last iteration's result
        ast_for: &AstFor,
        ast: &Ast,
    ) -> CompileNodeResult {
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

        let result = self.get_result_register(result_register)?;

        let body_result_register = if let Some(result) = result {
            self.push_op(SetNull, &[result.register]);
            Some(result.register)
        } else {
            None
        };

        let stack_count = self.frame().register_stack.len();

        let iterator_register = {
            let iterator_register = self.push_register()?;
            let iterable_register = self
                .compile_node(ResultRegister::Any, ast.node(*iterable), ast)?
                .unwrap();

            self.push_op_without_span(
                MakeIterator,
                &[iterator_register, iterable_register.register],
            );

            if iterable_register.is_temporary {
                self.pop_register()?;
            }

            iterator_register
        };

        let loop_start_ip = self.bytes.len();
        self.frame_mut().loop_stack.push(Loop {
            result_register: body_result_register,
            start_ip: loop_start_ip,
            jump_placeholders: vec![],
        });

        match args.as_slice() {
            [] => return compiler_error!(self, "Missing argument in for loop"),
            [single_arg] => {
                match &ast.node(*single_arg).node {
                    Node::Id(id) => {
                        // e.g. for i in 0..10
                        let arg_register = self.assign_local_register(*id)?;
                        self.push_op_without_span(IterNext, &[arg_register, iterator_register]);
                        self.push_loop_jump_placeholder()?;
                    }
                    Node::Wildcard(_) => {
                        // e.g. for _ in 0..10
                        self.push_op_without_span(IterNextQuiet, &[iterator_register]);
                        self.push_loop_jump_placeholder()?;
                    }
                    unexpected => {
                        return compiler_error!(
                            self,
                            "Expected ID or wildcard in for loop args, found {}",
                            unexpected
                        )
                    }
                }
            }
            [args @ ..] => {
                // e.g. for a, b, c in list_of_lists()
                // e.g. for key, value in map

                // A temporary register for the iterator output.
                // Args are unpacked from the temp register
                let temp_register = self.push_register()?;

                self.push_op_without_span(IterNextTemp, &[temp_register, iterator_register]);
                self.push_loop_jump_placeholder()?;

                for (i, arg) in args.iter().enumerate() {
                    match &ast.node(*arg).node {
                        Node::Id(id) => {
                            let arg_register = self.assign_local_register(*id)?;
                            self.push_op_without_span(
                                TempIndex,
                                &[arg_register, temp_register, i as u8],
                            );
                        }
                        Node::Wildcard(_) => {}
                        unexpected => {
                            return compiler_error!(
                                self,
                                "Expected ID or wildcard in for loop args, found {}",
                                unexpected
                            )
                        }
                    }
                }

                self.pop_register()?; // temp_register
            }
        }

        self.compile_node(
            body_result_register.map_or(ResultRegister::None, |register| {
                ResultRegister::Fixed(register)
            }),
            ast.node(*body),
            ast,
        )?;

        self.push_jump_back_op(JumpBack, &[], loop_start_ip);

        match self.frame_mut().loop_stack.pop() {
            Some(loop_info) => {
                for placeholder in loop_info.jump_placeholders.iter() {
                    self.update_offset_placeholder(*placeholder)?;
                }
            }
            None => return compiler_error!(self, "Empty loop info stack"),
        }

        self.truncate_register_stack(stack_count)?;

        if self.settings.repl_mode && self.frame_stack.len() == 1 {
            for arg in args {
                if let Node::Id(id) = &ast.node(*arg).node {
                    let arg_register = match self.frame().get_local_assigned_register(*id) {
                        Some(register) => register,
                        None => return compiler_error!(self, "Missing arg register"),
                    };
                    self.compile_value_export(*id, arg_register)?;
                }
            }
        }

        Ok(result)
    }

    fn compile_loop(
        &mut self,
        result_register: ResultRegister, // register that gets the last iteration's result
        condition: Option<(AstIndex, bool)>, // condition, negate condition
        body: AstIndex,
        ast: &Ast,
    ) -> CompileNodeResult {
        use Op::*;

        let result = self.get_result_register(result_register)?;
        let body_result_register = if let Some(result) = result {
            if condition.is_some() {
                // If there's a condition, then the result should be set to Null in case
                // there are no loop iterations
                self.push_op(SetNull, &[result.register]);
            }
            Some(result.register)
        } else {
            None
        };

        let loop_start_ip = self.bytes.len();

        self.frame_mut().loop_stack.push(Loop {
            start_ip: loop_start_ip,
            result_register: body_result_register,
            jump_placeholders: Vec::new(),
        });

        if let Some((condition, negate_condition)) = condition {
            // Condition
            let condition_register = self
                .compile_node(ResultRegister::Any, ast.node(condition), ast)?
                .unwrap();
            let op = if negate_condition {
                JumpIfTrue
            } else {
                JumpIfFalse
            };
            self.push_op_without_span(op, &[condition_register.register]);
            self.push_loop_jump_placeholder()?;
            if condition_register.is_temporary {
                self.pop_register()?;
            }
        }

        let body_result = self.compile_node(
            body_result_register.map_or(ResultRegister::None, |register| {
                ResultRegister::Fixed(register)
            }),
            ast.node(body),
            ast,
        )?;

        self.push_jump_back_op(JumpBack, &[], loop_start_ip);

        if let Some(body_result) = body_result {
            if body_result.is_temporary {
                self.pop_register()?;
            }
        }

        match self.frame_mut().loop_stack.pop() {
            Some(loop_info) => {
                for placeholder in loop_info.jump_placeholders.iter() {
                    self.update_offset_placeholder(*placeholder)?;
                }
            }
            None => return compiler_error!(self, "Empty loop info stack"),
        }

        Ok(result)
    }

    fn compile_node_with_jump_offset(
        &mut self,
        result_register: ResultRegister,
        node: &AstNode,
        ast: &Ast,
    ) -> CompileNodeResult {
        let offset_ip = self.push_offset_placeholder();
        let result = self.compile_node(result_register, node, ast)?;
        self.update_offset_placeholder(offset_ip)?;
        Ok(result)
    }

    fn push_jump_back_op(&mut self, op: Op, bytes: &[u8], target_ip: usize) {
        let offset = self.bytes.len() + 3 + bytes.len() - target_ip;
        self.push_op_without_span(op, bytes);
        self.push_bytes(&(offset as u16).to_le_bytes());
    }

    fn push_offset_placeholder(&mut self) -> usize {
        let offset_ip = self.bytes.len();
        self.push_bytes(&[0, 0]);
        offset_ip
    }

    fn push_loop_jump_placeholder(&mut self) -> Result<(), CompilerError> {
        let placeholder = self.push_offset_placeholder();
        match self.frame_mut().loop_stack.last_mut() {
            Some(loop_info) => {
                loop_info.jump_placeholders.push(placeholder);
                Ok(())
            }
            None => compiler_error!(self, "Missing loop info"),
        }
    }

    fn update_offset_placeholder(&mut self, offset_ip: usize) -> Result<(), CompilerError> {
        let offset = self.bytes.len() - offset_ip - 2; // -2 bytes for u16
        match u16::try_from(offset) {
            Ok(offset_u16) => {
                let offset_bytes = offset_u16.to_le_bytes();
                self.bytes[offset_ip] = offset_bytes[0];
                self.bytes[offset_ip + 1] = offset_bytes[1];
                Ok(())
            }
            Err(_) => compiler_error!(
                self,
                "Jump offset is too large, {} is larger than the maximum of {}.
                 Try breaking up this part of the program a bit."
            ),
        }
    }

    fn push_op(&mut self, op: Op, bytes: &[u8]) {
        self.debug_info.push(self.bytes.len(), self.span());
        self.push_op_without_span(op, bytes);
    }

    fn push_op_without_span(&mut self, op: Op, bytes: &[u8]) {
        self.bytes.push(op as u8);
        self.bytes.extend_from_slice(bytes);
        self.frame_mut().last_op = Some(op);
    }

    fn push_bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    fn push_bytes_with_span(&mut self, bytes: &[u8], span: Span) {
        self.debug_info.push(self.bytes.len(), span);
        self.bytes.extend_from_slice(bytes);
    }

    fn frame(&self) -> &Frame {
        self.frame_stack.last().expect("Frame stack is empty")
    }

    fn frame_mut(&mut self) -> &mut Frame {
        self.frame_stack.last_mut().expect("Frame stack is empty")
    }

    fn push_register(&mut self) -> Result<u8, CompilerError> {
        self.frame_mut()
            .push_register()
            .map_err(|e| self.make_error(e))
    }

    fn pop_register(&mut self) -> Result<u8, CompilerError> {
        self.frame_mut()
            .pop_register()
            .map_err(|e| self.make_error(e))
    }

    fn peek_register(&mut self, n: usize) -> Result<u8, CompilerError> {
        self.frame_mut()
            .peek_register(n)
            .map_err(|e| self.make_error(e))
    }

    fn truncate_register_stack(&mut self, stack_count: usize) -> Result<(), CompilerError> {
        self.frame_mut()
            .truncate_register_stack(stack_count)
            .map_err(|e| self.make_error(e))
    }

    fn assign_local_register(&mut self, local: ConstantIndex) -> Result<u8, CompilerError> {
        self.frame_mut()
            .assign_local_register(local)
            .map_err(|e| self.make_error(e))
    }

    fn reserve_local_register(&mut self, local: ConstantIndex) -> Result<u8, CompilerError> {
        self.frame_mut()
            .reserve_local_register(local)
            .map_err(|e| self.make_error(e))
    }

    fn commit_local_register(&mut self, register: u8) -> Result<u8, CompilerError> {
        for deferred_op in self
            .frame_mut()
            .commit_local_register(register)
            .map_err(|e| self.make_error(e))?
        {
            self.push_bytes_with_span(&deferred_op.bytes, deferred_op.span);
        }

        Ok(register)
    }

    fn make_error(&self, message: String) -> CompilerError {
        CompilerError {
            message,
            span: self.span(),
        }
    }

    fn span(&self) -> Span {
        *self.span_stack.last().expect("Empty span stack")
    }
}

fn args_size_op(args: &[AstIndex], ast: &Ast) -> Op {
    if args
        .iter()
        .any(|arg| matches!(&ast.node(*arg).node, Node::Ellipsis(_)))
    {
        Op::CheckSizeMin
    } else {
        Op::CheckSizeEqual
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
