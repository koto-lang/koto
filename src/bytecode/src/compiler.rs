use {
    crate::{DebugInfo, FunctionFlags, Op, TypeId},
    koto_parser::{
        AssignOp, AssignTarget, Ast, AstFor, AstIf, AstIndex, AstNode, AstOp, AstTry,
        ConstantIndex, Function, LookupNode, MapKey, MatchArm, MetaKeyId, Node, Scope, Span,
        StringNode, SwitchArm,
    },
    smallvec::SmallVec,
    std::{convert::TryFrom, error, fmt},
};

/// The error type used to report errors during compilation
#[derive(Clone, Debug)]
pub struct CompilerError {
    pub message: String,
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

#[derive(Clone, Debug, Default)]
struct Loop {
    start_ip: usize,
    jump_placeholders: Vec<usize>,
}

impl Loop {
    fn new(start_ip: usize) -> Self {
        Self {
            start_ip,
            ..Default::default()
        }
    }
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
    // The register is currently being assigned to,
    // it will become assigned at the end of the assignment expression.
    // Instructions can be deferred until the register is committed,
    // e.g. for functions that need to capture themselves after they've been fully assigned
    Reserved(ConstantIndex, Vec<u8>),
    // The register contains a value not associated with an id, e.g. a wildcard function arg
    Allocated,
}

#[derive(Clone, Debug, Default)]
struct Frame {
    loop_stack: Vec<Loop>,
    register_stack: Vec<u8>,
    local_registers: Vec<LocalRegister>,
    temporary_base: u8,
    temporary_count: u8,
    last_op: Option<Op>, // used to decide if an additional return instruction is needed
}

impl Frame {
    fn new(local_count: u8, args: &[Arg], captures: &[ConstantIndex]) -> Self {
        let temporary_base = local_count
            + captures.len() as u8
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

    fn get_local_register(&self, index: ConstantIndex) -> Option<u8> {
        self.local_registers
            .iter()
            .position(|local_register| {
                let register_index = match local_register {
                    LocalRegister::Assigned(register_index) => register_index,
                    LocalRegister::Reserved(register_index, _) => register_index,
                    LocalRegister::Allocated => return false,
                };
                *register_index == index
            })
            .map(|position| position as u8)
    }

    fn get_local_assigned_register(&self, index: ConstantIndex) -> Option<u8> {
        self.local_registers
            .iter()
            .position(|local_register| {
                matches!(local_register,
                    LocalRegister::Assigned(assigned_index) if *assigned_index == index
                )
            })
            .map(|position| position as u8)
    }

    fn get_local_reserved_register(&self, index: ConstantIndex) -> Option<u8> {
        self.local_registers
            .iter()
            .position(|local_register| {
                matches!(local_register,
                    LocalRegister::Reserved(assigned_index, _) if *assigned_index == index
                )
            })
            .map(|position| position as u8)
    }

    fn reserve_local_register(&mut self, local: ConstantIndex) -> Result<u8, String> {
        match self.get_local_assigned_register(local) {
            Some(assigned) => Ok(assigned),
            None => {
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

    fn defer_op_until_register_is_committed(
        &mut self,
        reserved_register: u8,
        bytes: Vec<u8>,
    ) -> Result<(), String> {
        match self.local_registers.get_mut(reserved_register as usize) {
            Some(LocalRegister::Reserved(_, deferred_ops)) => {
                deferred_ops.extend_from_slice(&bytes);
                Ok(())
            }
            _ => Err(format!(
                "defer_op_until_register_is_committed: register {} hasn't been reserved",
                reserved_register
            )),
        }
    }

    fn commit_local_register(&mut self, local_register: u8) -> Result<Vec<u8>, String> {
        let local_register = local_register as usize;
        let (index, deferred_ops) = match self.local_registers.get(local_register) {
            Some(LocalRegister::Assigned(_)) => {
                return Ok(vec![]);
            }
            Some(LocalRegister::Reserved(index, deferred_ops)) => (*index, deferred_ops.clone()),
            _ => {
                return Err(format!(
                    "commit_local_register: register {} hasn't been reserved",
                    local_register
                ));
            }
        };

        self.local_registers[local_register] = LocalRegister::Assigned(index);
        Ok(deferred_ops)
    }

    fn assign_local_register(&mut self, local: ConstantIndex) -> Result<u8, String> {
        let local_register = match self.get_local_assigned_register(local) {
            Some(assigned) => assigned,
            None => match self.get_local_reserved_register(local) {
                Some(reserved) => {
                    let deferred_ops = self.commit_local_register(reserved)?;
                    if !deferred_ops.is_empty() {
                        return Err(
                            "assign_local_register: committing register that has remaining ops"
                                .to_string(),
                        );
                    }
                    reserved
                }
                None => {
                    self.local_registers.push(LocalRegister::Assigned(local));

                    let new_local_register = self.local_registers.len() - 1;

                    if new_local_register > self.temporary_base as usize {
                        return Err("assign_local_register: Locals overflowed".to_string());
                    }

                    new_local_register as u8
                }
            },
        };

        Ok(local_register)
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
        // frame's local scope.
        accessed_non_locals
            .iter()
            .filter(|&non_local| {
                self.local_registers
                    .contains(&LocalRegister::Assigned(*non_local))
                    || self
                        .local_registers
                        .contains(&LocalRegister::Reserved(*non_local, vec![]))
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
#[derive(Clone, Copy, Debug)]
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
            Node::Empty => {
                let result = self.get_result_register(result_register)?;
                if let Some(result) = result {
                    self.push_op(SetEmpty, &[result.register]);
                }
                result
            }
            Node::Id(index) => self.compile_load_id(result_register, *index)?,
            Node::Lookup(lookup) => {
                self.compile_lookup(result_register, lookup, None, None, ast)?
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
            Node::Number0 => {
                let result = self.get_result_register(result_register)?;
                if let Some(result) = result {
                    self.push_op(Set0, &[result.register]);
                }
                result
            }
            Node::Number1 => {
                let result = self.get_result_register(result_register)?;
                if let Some(result) = result {
                    self.push_op(Set1, &[result.register]);
                }
                result
            }
            Node::Float(constant) => {
                let result = self.get_result_register(result_register)?;
                if let Some(result) = result {
                    self.load_constant(result.register, *constant, LoadFloat, LoadFloatLong);
                }
                result
            }
            Node::Int(constant) => {
                let result = self.get_result_register(result_register)?;
                if let Some(result) = result {
                    self.load_constant(result.register, *constant, LoadInt, LoadIntLong);
                }
                result
            }
            Node::Str(string) => self.compile_string(result_register, &string.nodes, ast)?,
            Node::Num2(elements) => self.compile_make_num2(result_register, elements, ast)?,
            Node::Num4(elements) => self.compile_make_num4(result_register, elements, ast)?,
            Node::List(elements) => self.compile_make_list(result_register, elements, ast)?,
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
                self.compile_make_tuple(result_register, elements, false, ast)?
            }
            Node::TempTuple(elements) => {
                self.compile_make_tuple(result_register, elements, true, ast)?
            }
            Node::Negate(expression) => self.compile_negate(result_register, *expression, ast)?,
            Node::Function(f) => self.compile_function(result_register, f, ast)?,
            Node::Call { function, args } => {
                match &ast.node(*function).node {
                    Node::Id(id) => {
                        if let Some(function_register) =
                            self.frame().get_local_assigned_register(*id)
                        {
                            self.compile_call(result_register, function_register, args, None, ast)?
                        } else {
                            let result = self.get_result_register(result_register)?;
                            let call_result_register = if let Some(result) = result {
                                ResultRegister::Fixed(result.register)
                            } else {
                                ResultRegister::None
                            };

                            let function_register = self.push_register()?;
                            self.compile_load_non_local(function_register, *id);

                            self.compile_call(
                                call_result_register,
                                function_register,
                                args,
                                None,
                                ast,
                            )?;

                            self.pop_register()?; // function_register
                            result
                        }
                    }
                    Node::Lookup(function_lookup) => self.compile_lookup(
                        result_register,
                        function_lookup,
                        Some(&LookupNode::Call(args.clone())),
                        None,
                        ast,
                    )?,
                    _ => {
                        return compiler_error!(self, "Call: unexpected node at index {}", function)
                    }
                }
            }
            Node::Import { from, items } => {
                self.compile_import_expression(result_register, from, items)?
            }
            Node::Assign {
                target,
                op,
                expression,
            } => self.compile_assign(result_register, target, *op, *expression, ast)?,
            Node::MultiAssign {
                targets,
                expression,
            } => self.compile_multi_assign(result_register, targets, *expression, ast)?,
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
            Node::Wildcard => None,
            Node::For(ast_for) => self.compile_for(result_register, ast_for, ast)?,
            Node::While { condition, body } => {
                self.compile_loop(result_register, Some((*condition, false)), *body, ast)?
            }
            Node::Until { condition, body } => {
                self.compile_loop(result_register, Some((*condition, true)), *body, ast)?
            }
            Node::Loop { body } => self.compile_loop(result_register, None, *body, ast)?,
            Node::Break => {
                self.push_op(Jump, &[]);
                self.push_loop_jump_placeholder()?;

                let result = self.get_result_register(result_register)?;
                if let Some(result) = result {
                    self.push_op(SetEmpty, &[result.register]);
                }
                result
            }
            Node::Continue => {
                self.push_jump_back_op(JumpBack, &[], self.current_loop()?.start_ip);

                let result = self.get_result_register(result_register)?;
                if let Some(result) = result {
                    self.push_op(SetEmpty, &[result.register]);
                }
                result
            }
            Node::Return => match self.get_result_register(result_register)? {
                Some(result) => {
                    self.push_op(SetEmpty, &[result.register]);
                    self.push_op(Return, &[result.register]);
                    Some(result)
                }
                None => {
                    let register = self.push_register()?;
                    self.push_op(SetEmpty, &[register]);
                    self.push_op(Return, &[register]);
                    self.pop_register()?;
                    None
                }
            },
            Node::ReturnExpression(expression) => {
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
                self.push_bytes(&expression_string.to_le_bytes());

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
            match &ast.node(*arg).node {
                Node::List(nested_args) => {
                    let list_register = arg_index as u8;
                    self.push_op(Op::CheckType, &[list_register, TypeId::List as u8]);
                    self.push_op(Op::CheckSize, &[list_register, nested_args.len() as u8]);
                    self.compile_unpack_nested_args(list_register, nested_args, ast)?;
                }
                Node::Tuple(nested_args) => {
                    let tuple_register = arg_index as u8;
                    self.push_op(Op::CheckType, &[tuple_register, TypeId::Tuple as u8]);
                    self.push_op(Op::CheckSize, &[tuple_register, nested_args.len() as u8]);
                    self.compile_unpack_nested_args(tuple_register, nested_args, ast)?;
                }
                _ => {}
            }
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
            self.push_op(Op::SetEmpty, &[register]);
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
        // Nested IDs that will be unpacked are assigned registers after the top-level IDs.
        // e.g. Given:
        // f = |a, (b, (c, d)), _, e|
        // Args should then appear as:
        // [Local(a), Placeholder, Placeholder, Local(e), Unpacked(b), Unpacked(c), Unpacked(d)]
        //
        // Note that the value stack at runtime will have the function's captures loaded in after
        // the top-level locals and placeholders, and before unpacked args.

        let mut result = Vec::new();
        let mut nested_args = Vec::new();

        for arg in args.iter() {
            match &ast.node(*arg).node {
                Node::Id(id_index) => result.push(Arg::Local(*id_index)),
                Node::Wildcard => result.push(Arg::Placeholder),
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
                Node::Id(id_index) => result.push(Arg::Unpacked(*id_index)),
                Node::Wildcard => {}
                Node::List(nested_args) | Node::Tuple(nested_args) => {
                    result.extend(self.collect_nested_args(nested_args, ast)?);
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

        Ok(result)
    }

    fn compile_unpack_nested_args(
        &mut self,
        container_register: u8,
        args: &[AstIndex],
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        for (arg_index, arg) in args.iter().enumerate() {
            match &ast.node(*arg).node {
                Node::Wildcard => {}
                Node::Id(constant_index) => {
                    let local_register = self.assign_local_register(*constant_index)?;
                    self.push_op(
                        Op::ValueIndex,
                        &[local_register, container_register, arg_index as u8],
                    );
                }
                Node::List(nested_args) => {
                    let list_register = self.push_register()?;
                    self.push_op(
                        Op::ValueIndex,
                        &[list_register, container_register, arg_index as u8],
                    );
                    self.push_op(Op::CheckType, &[list_register, TypeId::List as u8]);
                    self.push_op(Op::CheckSize, &[list_register, nested_args.len() as u8]);
                    self.compile_unpack_nested_args(list_register, nested_args, ast)?;
                    self.pop_register()?; // list_register
                }
                Node::Tuple(nested_args) => {
                    let tuple_register = self.push_register()?;
                    self.push_op(
                        Op::ValueIndex,
                        &[tuple_register, container_register, arg_index as u8],
                    );
                    self.push_op(Op::CheckType, &[tuple_register, TypeId::Tuple as u8]);
                    self.push_op(Op::CheckSize, &[tuple_register, nested_args.len() as u8]);
                    self.compile_unpack_nested_args(tuple_register, nested_args, ast)?;
                    self.pop_register()?; // tuple_register
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
        use Op::SetEmpty;

        let result = match expressions {
            [] => match self.get_result_register(result_register)? {
                Some(result) => {
                    self.push_op(SetEmpty, &[result.register]);
                    Some(result)
                }
                None => {
                    // TODO Under what conditions do we get into this branch?
                    let register = self.push_register()?;
                    self.push_op(SetEmpty, &[register]);
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
                Node::Lookup(_) | Node::Wildcard => None,
                unexpected => {
                    return compiler_error!(self, "Expected Id in AST, found {}", unexpected)
                }
            },
            Scope::Export => None,
        };

        Ok(result)
    }

    fn compile_assign(
        &mut self,
        result_register: ResultRegister,
        target: &AssignTarget,
        op: AssignOp,
        expression: AstIndex,
        ast: &Ast,
    ) -> CompileNodeResult {
        use Op::*;

        let local_assign_register = self.local_register_for_assign_target(target, ast)?;
        let value_result_register = match local_assign_register {
            Some(local) => ResultRegister::Fixed(local),
            None => ResultRegister::Any,
        };

        let value_register = match op {
            AssignOp::Equal => {
                self.compile_node(value_result_register, ast.node(expression), ast)?
            }
            AssignOp::Add => self.compile_binary_op(
                value_result_register,
                AstOp::Add,
                target.target_index,
                expression,
                ast,
            )?,
            AssignOp::Subtract => self.compile_binary_op(
                value_result_register,
                AstOp::Subtract,
                target.target_index,
                expression,
                ast,
            )?,
            AssignOp::Multiply => self.compile_binary_op(
                value_result_register,
                AstOp::Multiply,
                target.target_index,
                expression,
                ast,
            )?,
            AssignOp::Divide => self.compile_binary_op(
                value_result_register,
                AstOp::Divide,
                target.target_index,
                expression,
                ast,
            )?,
            AssignOp::Modulo => self.compile_binary_op(
                value_result_register,
                AstOp::Modulo,
                target.target_index,
                expression,
                ast,
            )?,
        }
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
                        self.compile_value_export(*id_index, value_register.register);
                    }
                }
            }
            Node::Lookup(lookup) => {
                self.compile_lookup(
                    ResultRegister::None,
                    lookup,
                    None,
                    Some(value_register.register),
                    ast,
                )?;
            }
            Node::Meta(meta_id, name) => {
                self.compile_meta_export(*meta_id, *name, value_register.register);
            }
            Node::Wildcard => {}
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

        assert!(targets.len() < u8::MAX as usize);

        let result = {
            // reserve ids on lhs before compiling rhs
            for target in targets.iter() {
                if let Node::Id(id_index) = &ast.node(target.target_index).node {
                    self.reserve_local_register(*id_index)?;
                }
            }

            let rhs = self
                .compile_node(ResultRegister::Any, ast.node(expression), ast)?
                .unwrap();

            for (i, target) in targets.iter().enumerate() {
                match &ast.node(target.target_index).node {
                    Node::Id(id_index) => {
                        let local_register = match self.frame().get_local_register(*id_index) {
                            Some(register) => register,
                            None => return compiler_error!(self, "Missing register for target"),
                        };
                        // Get the value for the target by index
                        self.push_op(ValueIndex, &[local_register, rhs.register, i as u8]);
                        // Commit the register now that it's assigned
                        self.commit_local_register(local_register)?;
                    }
                    Node::Lookup(lookup) => {
                        let register = self.push_register()?;

                        self.push_op(ValueIndex, &[register, rhs.register, i as u8]);
                        self.compile_lookup(
                            ResultRegister::None,
                            lookup,
                            None,
                            Some(register),
                            ast,
                        )?;

                        self.pop_register()?;
                    }
                    Node::Wildcard => {}
                    unexpected => {
                        return compiler_error!(
                            self,
                            "Expected ID or lookup in AST, found {}",
                            unexpected
                        );
                    }
                };
            }

            match result_register {
                ResultRegister::Fixed(register) => {
                    self.push_op(Copy, &[register, rhs.register]);

                    if rhs.is_temporary {
                        self.pop_register()?;
                    }

                    Some(CompileResult::with_assigned(register))
                }
                ResultRegister::Any => Some(rhs),
                ResultRegister::None => None,
            }
        };

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
                None => None,
            }
        };

        Ok(result)
    }

    fn compile_value_export(&mut self, id: ConstantIndex, register: u8) {
        if id <= u8::MAX as u32 {
            self.push_op(Op::ValueExport, &[id as u8, register]);
        } else {
            self.push_op(Op::ValueExportLong, &id.to_le_bytes());
            self.push_bytes(&[register]);
        }
    }

    fn compile_meta_export(
        &mut self,
        meta_id: MetaKeyId,
        name: Option<ConstantIndex>,
        value_register: u8,
    ) {
        if let Some(name) = name {
            if name <= u8::MAX as u32 {
                self.push_op(
                    Op::MetaExportNamed,
                    &[meta_id as u8, value_register, name as u8],
                );
            } else {
                self.push_op(Op::MetaExportNamedLong, &[meta_id as u8, value_register]);
                self.push_bytes(&name.to_le_bytes());
            }
        } else {
            self.push_op(Op::MetaExport, &[meta_id as u8, value_register]);
        }
    }

    fn compile_load_non_local(&mut self, result_register: u8, id: ConstantIndex) {
        use Op::*;

        if id <= u8::MAX as u32 {
            self.push_op(LoadNonLocal, &[result_register, id as u8]);
        } else {
            self.push_op(LoadNonLocalLong, &[result_register]);
            self.push_bytes(&id.to_le_bytes());
        }
    }

    fn compile_import_expression(
        &mut self,
        result_register: ResultRegister,
        from: &[ConstantIndex],
        items: &[Vec<ConstantIndex>],
    ) -> CompileNodeResult {
        use Op::*;

        let result = self.get_result_register(result_register)?;

        let mut imported = vec![];

        if from.is_empty() {
            for item in items.iter() {
                let import_id = match item.last() {
                    Some(id) => id,
                    None => return compiler_error!(self, "Missing ID in import item"),
                };

                // Reserve a local for the imported item
                // (only reserve the register otherwise it'll show up in the import search)
                let import_register = self.reserve_local_register(*import_id)?;

                self.compile_import_item(import_register, item)?;

                imported.push(import_register);
                self.commit_local_register(import_register)?;

                if self.settings.repl_mode && self.frame_stack.len() == 1 {
                    self.compile_value_export(*import_id, import_register);
                }
            }
        } else {
            let from_register = self.push_register()?;

            self.compile_import_item(from_register, from)?;

            for item in items.iter() {
                let mut access_register = from_register;
                let import_id = match item.last() {
                    Some(id) => id,
                    None => return compiler_error!(self, "Missing ID in import item"),
                };

                // assign the leaf item to a local with a matching name
                let import_register = self.assign_local_register(*import_id)?;

                for id in item.iter() {
                    self.compile_access(import_register, access_register, *id);
                    access_register = import_register;
                }

                imported.push(import_register);

                if self.settings.repl_mode && self.frame_stack.len() == 1 {
                    self.compile_value_export(*import_id, import_register);
                }
            }

            self.pop_register()?; // from_register
        }

        if let Some(result) = result {
            match imported.as_slice() {
                [] => return compiler_error!(self, "Missing item to import"),
                [single_item] => {
                    self.push_op(Copy, &[result.register, *single_item]);
                }
                _ => {
                    self.push_op(MakeList, &[result.register, imported.len() as u8]);
                    for item in imported.iter() {
                        self.push_op(ListPushValue, &[result.register, *item]);
                    }
                }
            }
        }

        Ok(result)
    }

    fn compile_import_item(
        &mut self,
        result_register: u8,
        item: &[ConstantIndex],
    ) -> Result<(), CompilerError> {
        match item {
            [] => return compiler_error!(self, "Missing item to import"),
            [import_id] => self.compile_import_id(result_register, *import_id),
            [import_id, nested @ ..] => {
                self.compile_import_id(result_register, *import_id);

                for nested_item in nested.iter() {
                    self.compile_access(result_register, result_register, *nested_item);
                }
            }
        }

        Ok(())
    }

    fn compile_import_id(&mut self, result_register: u8, id: ConstantIndex) {
        use Op::*;

        if let Some(local_register) = self.frame().get_local_assigned_register(id) {
            if local_register != result_register {
                self.push_op(Copy, &[result_register, local_register]);
            }
        } else {
            // If the id isn't a local then it needs to be imported
            if id <= u8::MAX as u32 {
                self.push_op(Import, &[result_register, id as u8]);
            } else {
                self.push_op(ImportLong, &[result_register]);
                self.push_bytes(&id.to_le_bytes());
            }
        }
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
        let catch_register = if let Some(catch_arg) = catch_arg {
            self.assign_local_register(*catch_arg)?
        } else {
            // The catch argument is ignored, just use a dummy register
            self.push_register()?
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
        self.update_offset_placeholder(catch_offset);

        let catch_node = ast.node(*catch_block);
        self.span_stack.push(*ast.span(catch_node.span));

        // Clear the catch point at the start of the catch block
        // - if the catch block has been entered, then it needs to be de-registered in case there
        //   are errors thrown in the catch block.
        self.push_op(TryEnd, &[]);

        self.compile_node(try_result_register, catch_node, ast)?;
        self.span_stack.pop();

        if catch_arg.is_none() {
            self.pop_register()?;
        }

        self.update_offset_placeholder(finally_offset);
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

    fn compile_binary_op(
        &mut self,
        result_register: ResultRegister,
        op: AstOp,
        lhs: AstIndex,
        rhs: AstIndex,
        ast: &Ast,
    ) -> CompileNodeResult {
        use AstOp::*;

        let lhs_node = ast.node(lhs);
        let rhs_node = ast.node(rhs);

        match op {
            Add | Subtract | Multiply | Divide | Modulo => {
                self.compile_op(result_register, op, lhs_node, rhs_node, ast)
            }
            Less | LessOrEqual | Greater | GreaterOrEqual | Equal | NotEqual => {
                self.compile_comparison_op(result_register, op, lhs_node, rhs_node, ast)
            }
            And | Or => self.compile_logic_op(result_register, op, lhs, rhs, ast),
        }
    }

    fn compile_op(
        &mut self,
        result_register: ResultRegister,
        op: AstOp,
        lhs_node: &AstNode,
        rhs_node: &AstNode,
        ast: &Ast,
    ) -> CompileNodeResult {
        use AstOp::*;

        let op = match op {
            Add => Op::Add,
            Subtract => Op::Subtract,
            Multiply => Op::Multiply,
            Divide => Op::Divide,
            Modulo => Op::Modulo,
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

    fn compile_comparison_op(
        &mut self,
        result_register: ResultRegister,
        ast_op: AstOp,
        lhs: &AstNode,
        rhs: &AstNode,
        ast: &Ast,
    ) -> CompileNodeResult {
        use AstOp::*;

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
                    self.push_op(Op::JumpFalse, &[comparison_register]);
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
            self.update_offset_placeholder(*jump_offset);
        }

        self.truncate_register_stack(stack_count)?;

        Ok(result)
    }

    fn compile_logic_op(
        &mut self,
        result_register: ResultRegister,
        op: AstOp,
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
            AstOp::And => Op::JumpFalse,
            AstOp::Or => Op::JumpTrue,
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

    fn compile_negate(
        &mut self,
        result_register: ResultRegister,
        expression: AstIndex,
        ast: &Ast,
    ) -> CompileNodeResult {
        let source = self
            .compile_node(ResultRegister::Any, ast.node(expression), ast)?
            .unwrap();

        let result = match self.get_result_register(result_register)? {
            Some(target) => {
                self.push_op(Op::Negate, &[target.register, source.register]);
                Some(target)
            }
            None => None,
        };

        if source.is_temporary {
            self.pop_register()?;
        }

        Ok(result)
    }

    fn compile_string(
        &mut self,
        result_register: ResultRegister,
        nodes: &[StringNode],
        _ast: &Ast,
    ) -> CompileNodeResult {
        let result = self.get_result_register(result_register)?;
        if let Some(result) = result {
            match nodes {
                [StringNode::Literal(string_literal)] => {
                    self.load_constant(
                        result.register,
                        *string_literal,
                        Op::LoadString,
                        Op::LoadStringLong,
                    );
                }
                _ => todo!(),
            }
        }
        Ok(result)
    }

    fn compile_make_num2(
        &mut self,
        result_register: ResultRegister,
        elements: &[AstIndex],
        ast: &Ast,
    ) -> CompileNodeResult {
        if elements.is_empty() || elements.len() > 2 {
            return compiler_error!(
                self,
                "compile_make_num2: unexpected number of elements: {}",
                elements.len()
            );
        }

        let result = match self.get_result_register(result_register)? {
            Some(result) => {
                let stack_count = self.frame().register_stack.len();

                for element_node in elements.iter() {
                    let element_register = self.push_register()?;
                    self.compile_node(
                        ResultRegister::Fixed(element_register),
                        ast.node(*element_node),
                        ast,
                    )?;
                }

                let first_element_register = self.peek_register(elements.len() - 1)?;
                self.push_op(
                    Op::MakeNum2,
                    &[
                        result.register,
                        elements.len() as u8,
                        first_element_register,
                    ],
                );

                self.truncate_register_stack(stack_count)?;
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

    fn compile_make_num4(
        &mut self,
        result_register: ResultRegister,
        elements: &[AstIndex],
        ast: &Ast,
    ) -> CompileNodeResult {
        if elements.is_empty() || elements.len() > 4 {
            return compiler_error!(
                self,
                "compile_make_num4: unexpected number of elements: {}",
                elements.len()
            );
        }

        let result = match self.get_result_register(result_register)? {
            Some(result) => {
                let stack_count = self.frame().register_stack.len();

                for element_node in elements.iter() {
                    let element_register = self.push_register()?;
                    self.compile_node(
                        ResultRegister::Fixed(element_register),
                        ast.node(*element_node),
                        ast,
                    )?;
                }

                let first_element_register = self.peek_register(elements.len() - 1)?;
                self.push_op(
                    Op::MakeNum4,
                    &[
                        result.register,
                        elements.len() as u8,
                        first_element_register,
                    ],
                );

                self.truncate_register_stack(stack_count)?;

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

    fn compile_make_tuple(
        &mut self,
        result_register: ResultRegister,
        elements: &[AstIndex],
        temp_tuple: bool,
        ast: &Ast,
    ) -> CompileNodeResult {
        let result = self.get_result_register(result_register)?;
        let stack_count = self.frame().register_stack.len();

        for element in elements.iter() {
            let element_register = self.push_register()?;
            self.compile_node(
                ResultRegister::Fixed(element_register),
                ast.node(*element),
                ast,
            )?;
        }

        let result = if let Some(result) = result {
            let start_register = self.peek_register(elements.len() - 1)?;

            if temp_tuple {
                self.push_op(
                    Op::MakeTempTuple,
                    &[result.register, start_register as u8, elements.len() as u8],
                );
            // If we're making a temp tuple then the registers need to be kept around
            } else {
                self.push_op(
                    Op::MakeTuple,
                    &[result.register, start_register as u8, elements.len() as u8],
                );
                self.truncate_register_stack(stack_count)?;
            }

            Some(result)
        } else {
            None
        };

        Ok(result)
    }

    fn compile_make_list(
        &mut self,
        result_register: ResultRegister,
        elements: &[AstIndex],
        ast: &Ast,
    ) -> CompileNodeResult {
        use Op::*;

        let result = match self.get_result_register(result_register)? {
            Some(result) => {
                // TODO take ranges into account when determining size hint
                let size_hint = elements.len();
                if size_hint <= u8::MAX as usize {
                    self.push_op(MakeList, &[result.register, size_hint as u8]);
                } else {
                    self.push_op(MakeListLong, &[result.register]);
                    self.push_bytes(&size_hint.to_le_bytes());
                }

                match elements {
                    [] => {}
                    [single_element] => {
                        let element = self
                            .compile_node(ResultRegister::Any, ast.node(*single_element), ast)?
                            .unwrap();
                        self.push_op_without_span(
                            ListPushValue,
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
                                ListPushValues,
                                &[result.register, start_register, elements_batch.len() as u8],
                            );

                            self.truncate_register_stack(stack_count)?;
                        }
                    }
                }

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
                let size_hint = entries.len();
                if size_hint <= u8::MAX as usize {
                    self.push_op(MakeMap, &[result.register, size_hint as u8]);
                } else {
                    self.push_op(MakeMapLong, &[result.register]);
                    self.push_bytes(&size_hint.to_le_bytes());
                }

                for (key, maybe_value_node) in entries.iter() {
                    let value = match (key, maybe_value_node) {
                        (_, Some(value_node)) => {
                            let value_node = ast.node(*value_node);
                            self.compile_node(ResultRegister::Any, value_node, ast)?
                                .unwrap()
                        }
                        (MapKey::Id(id), None) | (MapKey::Str(id, _), None) => {
                            match self.frame().get_local_assigned_register(*id) {
                                Some(register) => CompileResult::with_assigned(register),
                                None => {
                                    let register = self.push_register()?;
                                    self.compile_load_non_local(register, *id);
                                    CompileResult::with_temporary(register)
                                }
                            }
                        }
                        (MapKey::Meta(key, _), None) => {
                            return compiler_error!(
                                self,
                                "Value missing for meta map key: @{:?}",
                                key
                            );
                        }
                    };

                    self.compile_map_insert(result.register, value.register, key);

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

            let flags = FunctionFlags {
                instance_function: function.is_instance_function,
                variadic: function.is_variadic,
                generator: function.is_generator,
            };

            self.push_op(
                Op::Function,
                &[result.register, arg_count, capture_count, flags.as_byte()],
            );

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

            match &ast.node(function.body).node {
                Node::Block(expressions) => {
                    self.compile_frame(
                        local_count,
                        expressions,
                        &function.args,
                        &captures,
                        ast,
                        allow_implicit_return,
                    )?;
                }
                _ => {
                    self.compile_frame(
                        local_count,
                        &[function.body],
                        &function.args,
                        &captures,
                        ast,
                        allow_implicit_return,
                    )?;
                }
            };

            self.update_offset_placeholder(function_size_ip);

            for (i, capture) in captures.iter().enumerate() {
                if let Some(local_register) = self.frame().get_local_reserved_register(*capture) {
                    self.frame_mut()
                        .defer_op_until_register_is_committed(
                            local_register,
                            vec![Op::Capture as u8, result.register, i as u8, local_register],
                        )
                        .map_err(|e| self.make_error(e))?;
                } else if let Some(local_register) =
                    self.frame().get_local_assigned_register(*capture)
                {
                    self.push_op(Op::Capture, &[result.register, i as u8, local_register]);
                } else {
                    let capture_register = self.push_register()?;
                    self.compile_load_non_local(capture_register, *capture);
                    self.push_op(Op::Capture, &[result.register, i as u8, capture_register]);
                    self.pop_register()?;
                }
            }

            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    fn compile_lookup(
        &mut self,
        result_register: ResultRegister,
        (root_node, mut next_node_index): &(LookupNode, Option<AstIndex>),
        add_node_to_end_of_lookup: Option<&LookupNode>,
        set_value: Option<u8>,
        ast: &Ast,
    ) -> CompileNodeResult {
        use Op::*;

        if next_node_index.is_none() {
            return compiler_error!(self, "compile_lookup: missing next node index");
        }

        let result = self.get_result_register(result_register)?;

        // Keep track of a register for each lookup node.
        // This produces a lookup chain, allowing lookup operations to access parent containers.
        let mut node_registers = SmallVec::<[u8; 4]>::new();

        // At the end of the lookup we'll pop the whole stack,
        // so we don't need to keep track of how many temporary registers we use.
        let stack_count = self.frame().register_stack.len();
        let span_stack_count = self.span_stack.len();

        let mut i = 0;
        let mut lookup_node = root_node.clone();

        loop {
            let is_last_node = next_node_index.is_none();

            match lookup_node {
                LookupNode::Root(root_node) => {
                    assert!(i == 0, "Root node not in first position");

                    let root = self
                        .compile_node(ResultRegister::Any, ast.node(root_node), ast)?
                        .unwrap();
                    node_registers.push(root.register);
                }
                LookupNode::Id(id) => {
                    // Access by id
                    // e.g. x.foo()
                    //    - x = Root
                    //    - foo = Id
                    //    - () = Call
                    let map_register = *node_registers.last().expect("Empty node registers");

                    if is_last_node {
                        if let Some(set_value) = set_value {
                            self.compile_map_insert(map_register, set_value, &MapKey::Id(id));
                        } else if let Some(result) = result {
                            self.compile_access(result.register, map_register, id);
                        }
                    } else {
                        let node_register = self.push_register()?;
                        node_registers.push(node_register);
                        self.compile_access(node_register, map_register, id);
                    }
                }
                LookupNode::Index(index_node) => {
                    let index = self
                        .compile_node(ResultRegister::Any, ast.node(index_node), ast)?
                        .unwrap();
                    let list_register = *node_registers.last().expect("Empty node registers");

                    if is_last_node {
                        if let Some(set_value) = set_value {
                            self.push_op(SetIndex, &[list_register, index.register, set_value]);
                        } else if let Some(result) = result {
                            self.push_op(Index, &[result.register, list_register, index.register]);
                        }
                    } else {
                        let node_register = self.push_register()?;
                        node_registers.push(node_register);
                        self.push_op(Index, &[node_register, list_register, index.register]);
                    }
                }
                LookupNode::Call(args) => {
                    // Function call

                    if is_last_node && set_value.is_some() {
                        return compiler_error!(self, "Assigning to temporary value");
                    }

                    let parent_register = if i > 1 {
                        Some(node_registers[node_registers.len() - 2])
                    } else {
                        None
                    };

                    let function_register = *node_registers.last().expect("Empty node registers");

                    if is_last_node {
                        let call_result = if let Some(result) = result {
                            ResultRegister::Fixed(result.register)
                        } else {
                            ResultRegister::None
                        };

                        self.compile_call(
                            call_result,
                            function_register,
                            &args,
                            parent_register,
                            ast,
                        )?;
                    } else {
                        let node_register = self.push_register()?;
                        node_registers.push(node_register);
                        self.compile_call(
                            ResultRegister::Fixed(node_register),
                            function_register,
                            &args,
                            parent_register,
                            ast,
                        )?;
                    }
                }
            }

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
            } else if let Some(node) = add_node_to_end_of_lookup {
                lookup_node = node.clone();
            } else {
                break;
            }

            i += 1;
        }

        self.span_stack.truncate(span_stack_count);
        self.truncate_register_stack(stack_count)?;

        Ok(result)
    }

    fn compile_map_insert(&mut self, map_register: u8, value_register: u8, key: &MapKey) {
        match key {
            MapKey::Id(id) | MapKey::Str(id, _) => {
                if *id <= u8::MAX as u32 {
                    self.push_op_without_span(
                        Op::MapInsert,
                        &[map_register, value_register, *id as u8],
                    );
                } else {
                    self.push_op_without_span(Op::MapInsertLong, &[map_register, value_register]);
                    self.push_bytes(&id.to_le_bytes());
                }
            }
            MapKey::Meta(key, name) => {
                let key = *key as u8;
                if let Some(name) = name {
                    if *name <= u8::MAX as u32 {
                        self.push_op_without_span(
                            Op::MetaInsertNamed,
                            &[map_register, value_register, key, *name as u8],
                        );
                    } else {
                        self.push_op_without_span(
                            Op::MetaInsertNamedLong,
                            &[map_register, value_register, key],
                        );
                        self.push_bytes(&name.to_le_bytes());
                    }
                } else {
                    self.push_op_without_span(Op::MetaInsert, &[map_register, value_register, key]);
                }
            }
        }
    }

    fn compile_access(&mut self, result_register: u8, value_register: u8, key: ConstantIndex) {
        if key <= u8::MAX as u32 {
            self.push_op(Op::Access, &[result_register, value_register, key as u8]);
        } else {
            self.push_op(Op::AccessLong, &[result_register, value_register]);
            self.push_bytes(&key.to_le_bytes());
        }
    }

    fn compile_call(
        &mut self,
        result_register: ResultRegister,
        function_register: u8,
        args: &[AstIndex],
        parent: Option<u8>,
        ast: &Ast,
    ) -> CompileNodeResult {
        use Op::*;

        let result = self.get_result_register(result_register)?;
        let stack_count = self.frame().register_stack.len();

        // The frame base is an empty register that may be used for a parent value if needed
        // (it's decided at runtime if the parent value will be used or not).
        let frame_base = self.push_register()?;

        for arg in args.iter() {
            let arg_register = self.push_register()?;
            self.compile_node(ResultRegister::Fixed(arg_register), ast.node(*arg), ast)?;
        }

        let call_result_register = if let Some(result) = result {
            result.register
        } else {
            // The result isn't needed, so it can be placed in the frame's base register
            // (which isn't needed post-call).
            // An alternative here could be to have CallNoResult ops, but this will do for now.
            frame_base
        };

        match parent {
            Some(parent_register) => {
                self.push_op(
                    CallChild,
                    &[
                        call_result_register,
                        function_register,
                        frame_base,
                        args.len() as u8,
                        parent_register,
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
                        args.len() as u8,
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

        self.push_op_without_span(JumpFalse, &[condition_register.register]);
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
        self.update_offset_placeholder(condition_jump_ip);

        // Iterate through the else if blocks and collect their end jump placeholders
        let else_if_jump_ips = else_if_blocks
            .iter()
            .map(
                |(else_if_condition, else_if_node)| -> Result<usize, CompilerError> {
                    let condition = self
                        .compile_node(ResultRegister::Any, ast.node(*else_if_condition), ast)?
                        .unwrap();

                    self.push_op_without_span(JumpFalse, &[condition.register]);
                    let conditon_jump_ip = self.push_offset_placeholder();

                    if condition.is_temporary {
                        self.pop_register()?;
                    }

                    self.compile_node(expression_result_register, ast.node(*else_if_node), ast)?;

                    self.push_op_without_span(Jump, &[]);
                    let else_if_jump_ip = self.push_offset_placeholder();

                    self.update_offset_placeholder(conditon_jump_ip);

                    Ok(else_if_jump_ip)
                },
            )
            .collect::<Result<Vec<_>, _>>()?;

        // Else - either compile the else block, or set the result to empty
        if let Some(else_node) = else_node {
            self.compile_node(expression_result_register, ast.node(*else_node), ast)?;
        } else if let Some(result) = result {
            self.push_op_without_span(SetEmpty, &[result.register]);
        }

        // We're at the end, so update the if and else if jump placeholders
        if let Some(if_jump_ip) = if_jump_ip {
            self.update_offset_placeholder(if_jump_ip);
        }

        for else_if_jump_ip in else_if_jump_ips.iter() {
            self.update_offset_placeholder(*else_if_jump_ip);
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

                self.push_op_without_span(Op::JumpFalse, &[condition_register.register]);

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
                self.update_offset_placeholder(jump_placeholder);
            }
        }

        for jump_placeholder in result_jump_placeholders.iter() {
            self.update_offset_placeholder(*jump_placeholder);
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
            self.update_offset_placeholder(*jump_placeholder);
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
                Node::Wildcard => Some(vec![*arm_pattern]),
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
                self.update_offset_placeholder(*jump_placeholder);
            }

            self.span_stack.pop(); // arm node
        }

        // Update the match end jump placeholders before the condition
        for jump_placeholder in jumps.match_end.iter() {
            self.update_offset_placeholder(*jump_placeholder);
        }

        // Arm condition, e.g.
        // match foo
        //   x if x > 10 then 99
        if let Some(condition) = arm.condition {
            let condition_register = self
                .compile_node(ResultRegister::Any, ast.node(condition), ast)?
                .unwrap();

            self.push_op_without_span(Op::JumpFalse, &[condition_register.register]);
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
            self.update_offset_placeholder(*jump_placeholder);
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
                Node::Empty
                | Node::BoolTrue
                | Node::BoolFalse
                | Node::Number0
                | Node::Number1
                | Node::Float(_)
                | Node::Int(_)
                | Node::Str(_)
                | Node::Lookup(_) => {
                    let pattern = self.push_register()?;
                    self.compile_node(ResultRegister::Fixed(pattern), pattern_node, ast)?;
                    let comparison = self.push_register()?;

                    if match_is_container {
                        let element = self.push_register()?;
                        self.push_op(
                            ValueIndex,
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
                        self.push_op(JumpFalse, &[comparison]);
                        params.jumps.arm_end.push(self.push_offset_placeholder());
                    } else if params.has_last_pattern && is_last_pattern {
                        // If there's a match with remaining alternative matches,
                        // then jump to the end of the alternatives
                        self.push_op(JumpTrue, &[comparison]);
                        params.jumps.match_end.push(self.push_offset_placeholder());
                    } else {
                        // If there's no match but there remaining alternative matches,
                        // then jump to the next alternative
                        self.push_op(JumpFalse, &[comparison]);
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
                            ValueIndex,
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
                Node::Wildcard => {
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

    fn compile_nested_match_arm_patterns<'a>(
        &mut self,
        params: MatchArmParameters<'a>,
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
                ValueIndex,
                &[value_register, params.match_register, pattern_index as u8],
            );
            value_register
        } else {
            params.match_register
        };

        let temp_register = self.push_register()?;

        // Check that the container has the correct type
        self.push_op(type_check_op, &[temp_register, value_register]);
        self.push_op(JumpFalse, &[temp_register]);
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
            self.push_op(JumpFalse, &[temp_register]);

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

        let AstFor { args, range, body } = &ast_for;

        //   make iterator, iterator_register
        //   make local registers for args
        // loop_start:
        //   iterator_next_or_jump iterator_register arg_register jump -> end
        //   loop body
        //   jump -> loop_start
        // end:

        let result = self.get_result_register(result_register)?;
        if let Some(result) = result {
            self.push_op(SetEmpty, &[result.register]);
        }

        let stack_count = self.frame().register_stack.len();

        let iterator_register = {
            let iterator_register = self.push_register()?;
            let range_register = self
                .compile_node(ResultRegister::Any, ast.node(*range), ast)?
                .unwrap();

            self.push_op_without_span(MakeIterator, &[iterator_register, range_register.register]);

            if range_register.is_temporary {
                self.pop_register()?;
            }

            iterator_register
        };

        let loop_start_ip = self.bytes.len();
        self.frame_mut().loop_stack.push(Loop::new(loop_start_ip));

        match args.as_slice() {
            [] => return compiler_error!(self, "Missing argument in for loop"),
            [None] => {
                // e.g. for _ in 0..10
                self.push_op_without_span(IterNextQuiet, &[iterator_register as u8]);
                self.push_loop_jump_placeholder()?;
            }
            [Some(arg)] => {
                // e.g. for i in 0..10
                let arg_register = self.assign_local_register(*arg)?;
                self.push_op_without_span(IterNext, &[arg_register, iterator_register as u8]);
                self.push_loop_jump_placeholder()?;
            }
            [args @ ..] => {
                // e.g. for a, b, c in list_of_lists()
                // e.g. for key, value in map

                // A temporary register for the iterator output.
                // Args are unpacked from the temp register
                let temp_register = self.push_register()?;

                self.push_op_without_span(IterNextTemp, &[temp_register, iterator_register]);
                self.push_loop_jump_placeholder()?;

                for (i, maybe_arg) in args.iter().enumerate() {
                    if let Some(arg) = maybe_arg {
                        let arg_register = self.assign_local_register(*arg)?;
                        self.push_op_without_span(
                            ValueIndex,
                            &[arg_register, temp_register, i as u8],
                        );
                    }
                }

                self.pop_register()?; // temp_register
            }
        }

        let body_result_register = if let Some(result) = result {
            ResultRegister::Fixed(result.register)
        } else {
            ResultRegister::None
        };

        self.compile_node(body_result_register, ast.node(*body), ast)?;

        self.push_jump_back_op(JumpBack, &[], loop_start_ip);

        match self.frame_mut().loop_stack.pop() {
            Some(loop_info) => {
                for placeholder in loop_info.jump_placeholders.iter() {
                    self.update_offset_placeholder(*placeholder);
                }
            }
            None => return compiler_error!(self, "Empty loop info stack"),
        }

        self.truncate_register_stack(stack_count)?;

        if self.settings.repl_mode && self.frame_stack.len() == 1 {
            for arg in args.iter().flatten() {
                let arg_register = match self.frame().get_local_assigned_register(*arg) {
                    Some(register) => register,
                    None => return compiler_error!(self, "Missing arg register"),
                };
                self.compile_value_export(*arg, arg_register);
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

        let loop_start_ip = self.bytes.len();
        self.frame_mut().loop_stack.push(Loop::new(loop_start_ip));

        let result = self.get_result_register(result_register)?;

        if let Some((condition, negate_condition)) = condition {
            // Condition
            let condition_register = self
                .compile_node(ResultRegister::Any, ast.node(condition), ast)?
                .unwrap();
            let op = if negate_condition {
                JumpTrue
            } else {
                JumpFalse
            };
            self.push_op_without_span(op, &[condition_register.register]);
            self.push_loop_jump_placeholder()?;
            if condition_register.is_temporary {
                self.pop_register()?;
            }
        }

        let body_result_register = if let Some(result) = result {
            ResultRegister::Fixed(result.register)
        } else {
            ResultRegister::None
        };

        let body_result = self.compile_node(body_result_register, ast.node(body), ast)?;

        self.push_jump_back_op(JumpBack, &[], loop_start_ip);

        if let Some(body_result) = body_result {
            if body_result.is_temporary {
                self.pop_register()?;
            }
        }

        match self.frame_mut().loop_stack.pop() {
            Some(loop_info) => {
                for placeholder in loop_info.jump_placeholders.iter() {
                    self.update_offset_placeholder(*placeholder);
                }
            }
            None => return compiler_error!(self, "Empty loop info stack"),
        }

        Ok(result)
    }

    fn load_constant(
        &mut self,
        result_register: u8,
        index: ConstantIndex,
        short_op: Op,
        long_op: Op,
    ) {
        if index <= u8::MAX as u32 {
            self.push_op(short_op, &[result_register, index as u8]);
        } else {
            self.push_op(long_op, &[result_register]);
            self.push_bytes(&index.to_le_bytes());
        }
    }

    fn compile_node_with_jump_offset(
        &mut self,
        result_register: ResultRegister,
        node: &AstNode,
        ast: &Ast,
    ) -> CompileNodeResult {
        let offset_ip = self.push_offset_placeholder();
        let result = self.compile_node(result_register, node, ast)?;
        self.update_offset_placeholder(offset_ip);
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

    fn current_loop(&self) -> Result<&Loop, CompilerError> {
        self.frame()
            .loop_stack
            .last()
            .ok_or_else(|| self.make_error("Missing loop info".to_string()))
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

    fn update_offset_placeholder(&mut self, offset_ip: usize) {
        let offset = self.bytes.len() - offset_ip - 2;
        let offset_bytes = (offset as u16).to_le_bytes();
        self.bytes[offset_ip] = offset_bytes[0];
        self.bytes[offset_ip + 1] = offset_bytes[1];
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
        let deferred_ops = self
            .frame_mut()
            .commit_local_register(register)
            .map_err(|e| self.make_error(e))?;

        self.push_bytes(&deferred_ops);

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
