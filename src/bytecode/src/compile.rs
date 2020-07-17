use {
    crate::{DebugInfo, Op},
    koto_parser::{
        AssignOp, AssignTarget, Ast, AstFor, AstIf, AstIndex, AstNode, AstOp, AstTry,
        ConstantIndex, Function, LookupNode, MatchArm, Node, Scope, Span,
    },
    smallvec::SmallVec,
    std::convert::TryFrom,
};

pub struct CompilerError {
    pub message: String,
    pub span: Span,
}

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

#[derive(Clone, Debug, PartialEq)]
enum LocalRegister {
    Assigned(ConstantIndex),
    Reserved(ConstantIndex),
}

#[derive(Clone, Debug, Default)]
struct Frame {
    loop_stack: Vec<Loop>,
    register_stack: Vec<u8>,
    local_registers: Vec<LocalRegister>,
    captures: Vec<ConstantIndex>,
    temporary_base: u8,
    temporary_count: u8,
}

impl Frame {
    fn new(local_count: u8, args: &[ConstantIndex], captures: &[ConstantIndex]) -> Self {
        let mut local_registers = Vec::with_capacity(local_count as usize);
        local_registers.extend(args.iter().map(|arg| LocalRegister::Assigned(*arg)));

        Self {
            register_stack: Vec::with_capacity(local_count as usize),
            local_registers,
            captures: captures.to_vec(),
            temporary_base: local_count,
            ..Default::default()
        }
    }

    fn capture_slot(&self, index: ConstantIndex) -> Option<u8> {
        self.captures
            .iter()
            .position(|constant_index| index == *constant_index)
            .map(|position| position as u8)
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
                    LocalRegister::Reserved(register_index) => register_index,
                };
                *register_index == index
            })
            .map(|position| position as u8)
    }

    fn get_local_assigned_register(&self, index: ConstantIndex) -> Option<u8> {
        self.local_registers
            .iter()
            .position(|local_register| match local_register {
                LocalRegister::Assigned(assigned_index) if *assigned_index == index => true,
                _ => false,
            })
            .map(|position| position as u8)
    }

    fn reserve_local_register(&mut self, local: ConstantIndex) -> Result<u8, String> {
        match self.get_local_assigned_register(local) {
            Some(assigned) => Ok(assigned),
            None => {
                self.local_registers.push(LocalRegister::Reserved(local));

                let new_local_register = self.local_registers.len() - 1;

                if new_local_register > self.temporary_base as usize {
                    return Err("reserve_local_register: Locals overflowed".to_string());
                }

                Ok(new_local_register as u8)
            }
        }
    }

    fn commit_local_register(&mut self, local_register: u8) -> Result<(), String> {
        let local_register = local_register as usize;
        let index = match self.local_registers.get_mut(local_register) {
            Some(LocalRegister::Assigned(_)) => {
                return Ok(());
            }
            Some(LocalRegister::Reserved(index)) => index,
            None => {
                return Err(format!(
                    "commit_local_register: register {} hasn't been reserved",
                    local_register
                ));
            }
        };

        self.local_registers[local_register] = LocalRegister::Assigned(*index);
        Ok(())
    }

    fn assign_local_register(&mut self, local: ConstantIndex) -> Result<u8, String> {
        let local_register = match self.get_local_assigned_register(local) {
            Some(assigned) => assigned,
            None => {
                self.local_registers.push(LocalRegister::Assigned(local));

                let new_local_register = self.local_registers.len() - 1;

                if new_local_register > self.temporary_base as usize {
                    return Err("declare_local_register: Locals overflowed".to_string());
                }

                new_local_register as u8
            }
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

    fn captures_for_nested_frame(
        &self,
        accessed_non_locals: &[ConstantIndex],
    ) -> Vec<ConstantIndex> {
        accessed_non_locals
            .iter()
            .filter(|&non_local| {
                self.captures.contains(non_local)
                    || self
                        .local_registers
                        .contains(&LocalRegister::Assigned(*non_local))
                    || self
                        .local_registers
                        .contains(&LocalRegister::Reserved(*non_local))
            })
            .cloned()
            .collect()
    }
}

#[derive(Default)]
pub struct Options {
    /// Causes all top level identifiers to be exported to global
    pub repl_mode: bool,
}

#[derive(Default)]
pub struct Compiler {
    bytes: Vec<u8>,
    debug_info: DebugInfo,
    frame_stack: Vec<Frame>,
    span_stack: Vec<Span>,
    options: Options,
}

impl Compiler {
    pub fn compile(ast: &Ast, options: Options) -> Result<(Vec<u8>, DebugInfo), CompilerError> {
        let mut compiler = Compiler {
            options,
            ..Default::default()
        };

        if let Some(entry_point) = ast.entry_point() {
            compiler.compile_node(None, entry_point, ast)?;
        }

        Ok((compiler.bytes, compiler.debug_info))
    }

    fn compile_node(
        &mut self,
        result_register: Option<u8>,
        node: &AstNode,
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        use Op::*;

        self.span_stack.push(*ast.span(node.span));

        match &node.node {
            Node::Empty => {
                if let Some(result_register) = result_register {
                    self.push_op(SetEmpty, &[result_register]);
                }
            }
            Node::Id(index) => {
                if let Some(result_register) = result_register {
                    self.compile_load_id(result_register, *index)?;
                }
            }
            Node::Lookup(lookup) => self.compile_lookup(result_register, lookup, None, ast)?,
            Node::BoolTrue => {
                if let Some(result_register) = result_register {
                    self.push_op(SetTrue, &[result_register]);
                }
            }
            Node::BoolFalse => {
                if let Some(result_register) = result_register {
                    self.push_op(SetFalse, &[result_register]);
                }
            }
            Node::Number0 => {
                if let Some(result_register) = result_register {
                    self.push_op(Set0, &[result_register]);
                }
            }
            Node::Number1 => {
                if let Some(result_register) = result_register {
                    self.push_op(Set1, &[result_register]);
                }
            }
            Node::Number(constant) => {
                if let Some(result_register) = result_register {
                    let constant = *constant;
                    if constant <= u8::MAX as u32 {
                        self.push_op(LoadNumber, &[result_register, constant as u8]);
                    } else {
                        self.push_op(LoadNumberLong, &[result_register]);
                        self.push_bytes(&constant.to_le_bytes());
                    }
                }
            }
            Node::Str(constant) => {
                if let Some(result_register) = result_register {
                    self.load_string(result_register, *constant);
                }
            }
            Node::Num2(elements) => {
                self.compile_make_num2(result_register, &elements, ast)?;
            }
            Node::Num4(elements) => {
                self.compile_make_num4(result_register, &elements, ast)?;
            }
            Node::List(elements) => {
                self.compile_make_list(result_register, &elements, ast)?;
            }
            Node::Map(entries) => {
                self.compile_make_map(result_register, &entries, ast)?;
            }
            Node::Range {
                start,
                end,
                inclusive,
            } => {
                if let Some(result_register) = result_register {
                    let (start_register, pop_start) = self.compile_node_or_get_local(
                        Some(result_register),
                        ast.node(*start),
                        ast,
                    )?;

                    let end_available_register = if start_register != result_register {
                        Some(result_register)
                    } else {
                        None
                    };

                    let (end_register, pop_end) = self.compile_node_or_get_local(
                        end_available_register,
                        ast.node(*end),
                        ast,
                    )?;

                    let op = if *inclusive { RangeInclusive } else { Range };
                    self.push_op(op, &[result_register, start_register, end_register]);

                    if pop_end {
                        self.pop_register()?;
                    }
                    if pop_start {
                        self.pop_register()?;
                    }
                } else {
                    self.compile_node(None, ast.node(*start), ast)?;
                    self.compile_node(None, ast.node(*end), ast)?;
                }
            }
            Node::RangeFrom { start } => {
                if let Some(result_register) = result_register {
                    let start_register = self.push_register()?;

                    self.compile_node(Some(start_register), ast.node(*start), ast)?;
                    self.push_op(RangeFrom, &[result_register, start_register]);

                    self.pop_register()?;
                } else {
                    self.compile_node(None, ast.node(*start), ast)?;
                }
            }
            Node::RangeTo { end, inclusive } => {
                if let Some(result_register) = result_register {
                    let end_register = self.push_register()?;
                    self.compile_node(Some(end_register), ast.node(*end), ast)?;

                    let op = if *inclusive {
                        RangeToInclusive
                    } else {
                        RangeTo
                    };
                    self.push_op(op, &[result_register, end_register]);

                    self.pop_register()?;
                } else {
                    self.compile_node(None, ast.node(*end), ast)?;
                }
            }
            Node::RangeFull => {
                if let Some(result_register) = result_register {
                    self.push_op(RangeFull, &[result_register]);
                }
            }
            Node::MainBlock { body, local_count } => {
                self.compile_frame(*local_count as u8, body, &[], &[], ast)?;
            }
            Node::Block(expressions) => {
                self.compile_block(result_register, expressions, ast)?;
            }
            Node::Expressions(expressions) => {
                let stack_count = self.frame().register_stack.len();

                for expression in expressions.iter() {
                    let register = self.push_register()?;
                    self.compile_node(Some(register), ast.node(*expression), ast)?;
                }

                if let Some(result_register) = result_register {
                    let start_register = self.peek_register(expressions.len() - 1)?;

                    self.push_op(
                        RegisterList,
                        &[
                            result_register,
                            start_register as u8,
                            expressions.len() as u8,
                        ],
                    );
                } else {
                    self.truncate_register_stack(stack_count)?;
                }
            }
            Node::CopyExpression(expression) => {
                if let Some(result_register) = result_register {
                    let source_register = self.push_register()?;
                    self.compile_node(Some(source_register), ast.node(*expression), ast)?;

                    self.push_op(DeepCopy, &[result_register, source_register]);

                    self.pop_register()?;
                } else {
                    self.compile_node(None, ast.node(*expression), ast)?;
                }
            }
            Node::Negate(expression) => {
                if let Some(result_register) = result_register {
                    let source_register = self.push_register()?;
                    self.compile_node(Some(source_register), ast.node(*expression), ast)?;

                    self.push_op(Negate, &[result_register, source_register]);

                    self.pop_register()?;
                } else {
                    self.compile_node(None, ast.node(*expression), ast)?;
                }
            }
            Node::Function(f) => {
                self.compile_function(result_register, f, ast)?;
            }
            Node::Call { function, args } => {
                match &ast.node(*function).node {
                    Node::Id(id) => {
                        if let Some(function_register) =
                            self.frame().get_local_assigned_register(*id)
                        {
                            self.compile_call(result_register, function_register, args, None, ast)?;
                        } else {
                            let function_register = self.push_register()?;
                            self.compile_load_non_local_id(function_register, *id)?;
                            self.compile_call(result_register, function_register, args, None, ast)?;
                            self.pop_register()?;
                        }
                    }
                    Node::Lookup(function_lookup) => {
                        // TODO find a way to avoid the lookup cloning here
                        let mut call_lookup = function_lookup.clone();
                        call_lookup.push(LookupNode::Call(args.clone()));
                        self.compile_lookup(result_register, &call_lookup, None, ast)?
                    }
                    _ => {
                        return compiler_error!(self, "Call: unexpected node at index {}", function)
                    }
                };
            }
            Node::Import { from, items } => {
                self.compile_import_expression(result_register, from, items)?
            }
            Node::Assign {
                target,
                op,
                expression,
            } => {
                self.compile_assign(result_register, target, *op, *expression, ast)?;
            }
            Node::MultiAssign {
                targets,
                expressions,
            } => match &ast.node(*expressions).node {
                Node::Expressions(expressions) => {
                    self.compile_multi_assign(result_register, targets, &expressions, ast)?;
                }
                _ => {
                    self.compile_multi_assign(result_register, targets, &[*expressions], ast)?;
                }
            },
            Node::BinaryOp { op, lhs, rhs } => {
                self.compile_binary_op(result_register, *op, *lhs, *rhs, ast)?;
            }
            Node::If(ast_if) => self.compile_if(result_register, ast_if, ast)?,
            Node::Match { expression, arms } => {
                self.compile_match(result_register, *expression, arms, ast)?
            }
            Node::Wildcard => {}
            Node::For(ast_for) => self.compile_for(result_register, None, ast_for, ast)?,
            Node::While { condition, body } => {
                self.compile_while(result_register, None, *condition, *body, false, ast)?
            }
            Node::Until { condition, body } => {
                self.compile_while(result_register, None, *condition, *body, true, ast)?
            }
            Node::Break => {
                self.push_op(Jump, &[]);
                self.push_loop_jump_placeholder()?;
            }
            Node::Continue => {
                self.push_jump_back_op(JumpBack, &[], self.current_loop()?.start_ip);
            }
            Node::Return => {
                if let Some(result_register) = result_register {
                    self.push_op(SetEmpty, &[result_register]);
                    self.push_op(Return, &[result_register]);
                } else {
                    let register = self.push_register()?;
                    self.push_op(SetEmpty, &[register]);
                    self.push_op(Return, &[register]);
                    self.pop_register()?;
                }
            }
            Node::ReturnExpression(expression) => {
                self.compile_single_register_op(Return, result_register, *expression, ast)?
            }
            Node::Size(expression) => {
                self.compile_single_register_op(Size, result_register, *expression, ast)?
            }
            Node::Type(expression) => {
                self.compile_single_register_op(Type, result_register, *expression, ast)?
            }
            Node::Try(try_expression) => {
                self.compile_try_expression(result_register, try_expression, ast)?
            }
            Node::Debug {
                expression_string,
                expression,
            } => {
                let temp_register = self.push_register()?;
                self.compile_node(Some(temp_register), ast.node(*expression), ast)?;
                self.push_op(Debug, &[temp_register]);
                self.push_bytes(&expression_string.to_le_bytes());
                self.pop_register()?; // temp_register
            }
        }

        self.span_stack.pop();

        Ok(())
    }

    fn compile_node_or_get_local(
        &mut self,
        result_register: Option<u8>,
        node: &AstNode,
        ast: &Ast,
    ) -> Result<(u8, bool), CompilerError> {
        if let Node::Id(id) = node.node {
            if let Some(local_register) = self.frame().get_local_assigned_register(id) {
                Ok((local_register, false))
            } else if let Some(register) = result_register {
                self.compile_load_non_local_id(register, id)?;
                Ok((register, false))
            } else {
                let register = self.push_register()?;
                self.compile_load_non_local_id(register, id)?;
                Ok((register, true))
            }
        } else if let Some(register) = result_register {
            self.compile_node(Some(register), node, ast)?;
            Ok((register, false))
        } else {
            let register = self.push_register()?;
            self.compile_node(Some(register), node, ast)?;
            Ok((register, true))
        }
    }

    fn compile_frame(
        &mut self,
        local_count: u8,
        expressions: &[AstIndex],
        args: &[ConstantIndex],
        captures: &[ConstantIndex],
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        self.frame_stack
            .push(Frame::new(local_count, args, captures));

        let result_register = self.push_register()?;
        self.compile_block(Some(result_register), expressions, ast)?;
        self.push_op_without_span(Op::Return, &[result_register]);

        self.frame_stack.pop();

        Ok(())
    }

    fn compile_block(
        &mut self,
        result_register: Option<u8>,
        expressions: &[AstIndex],
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        use Op::*;

        match expressions {
            [] => {
                if let Some(result_register) = result_register {
                    self.push_op(SetEmpty, &[result_register]);
                }
            }
            [expression] => {
                self.compile_node(result_register, ast.node(*expression), ast)?;
            }
            [expressions @ .., last_expression] => {
                for expression in expressions.iter() {
                    self.compile_node(None, ast.node(*expression), ast)?;
                }

                self.compile_node(result_register, ast.node(*last_expression), ast)?;
            }
        }

        Ok(())
    }

    fn scope_for_assign_target(&self, target: &AssignTarget) -> Scope {
        if self.options.repl_mode && self.frame_stack.len() == 1 {
            Scope::Global
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
                Node::Id(constant_index) => {
                    if self.frame().capture_slot(*constant_index).is_some() {
                        None
                    } else {
                        Some(self.reserve_local_register(*constant_index)?)
                    }
                }
                Node::Lookup(_) => None,
                unexpected => {
                    return compiler_error!(self, "Expected Id in AST, found {}", unexpected)
                }
            },
            Scope::Global => None,
        };

        Ok(result)
    }

    fn compile_assign(
        &mut self,
        result_register: Option<u8>,
        target: &AssignTarget,
        op: AssignOp,
        expression: AstIndex,
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        use Op::*;

        let local_assign_register = self.local_register_for_assign_target(target, ast)?;
        let assign_register = match local_assign_register {
            Some(local) => local,
            None => self.push_register()?,
        };

        match op {
            AssignOp::Equal => {
                self.compile_node(Some(assign_register), ast.node(expression), ast)?
            }
            AssignOp::Add => self.compile_binary_op(
                Some(assign_register),
                AstOp::Add,
                target.target_index,
                expression,
                ast,
            )?,
            AssignOp::Subtract => self.compile_binary_op(
                Some(assign_register),
                AstOp::Subtract,
                target.target_index,
                expression,
                ast,
            )?,
            AssignOp::Multiply => self.compile_binary_op(
                Some(assign_register),
                AstOp::Multiply,
                target.target_index,
                expression,
                ast,
            )?,
            AssignOp::Divide => self.compile_binary_op(
                Some(assign_register),
                AstOp::Divide,
                target.target_index,
                expression,
                ast,
            )?,
            AssignOp::Modulo => self.compile_binary_op(
                Some(assign_register),
                AstOp::Modulo,
                target.target_index,
                expression,
                ast,
            )?,
        }

        match &ast.node(target.target_index).node {
            Node::Id(id_index) => {
                match self.scope_for_assign_target(target) {
                    Scope::Local => {
                        if local_assign_register.is_some() {
                            // To ensure that global rhs ids with the same name as a local that's
                            // currently being assigned can be loaded correctly, only commit the
                            // reserved local as assigned after the rhs has been compiled.
                            self.commit_local_register(assign_register)?;
                        }

                        if let Some(capture) = self.frame().capture_slot(*id_index) {
                            self.push_op(SetCapture, &[capture, assign_register]);
                        }
                    }
                    Scope::Global => {
                        self.compile_set_global(*id_index, assign_register);
                    }
                }

                if let Some(result_register) = result_register {
                    if result_register != assign_register {
                        self.push_op(Copy, &[result_register, assign_register]);
                    }
                }
            }
            Node::Lookup(lookup) => {
                self.compile_lookup(result_register, &lookup, Some(assign_register), ast)?;
            }
            unexpected => {
                return compiler_error!(self, "Expected Lookup or Id in AST, found {}", unexpected)
            }
        };

        if local_assign_register.is_none() {
            self.pop_register()?;
        }

        Ok(())
    }

    fn compile_multi_assign(
        &mut self,
        result_register: Option<u8>,
        targets: &[AssignTarget],
        expressions: &[AstIndex],
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        use Op::*;

        assert!(targets.len() < u8::MAX as usize);
        assert!(expressions.len() < u8::MAX as usize);

        match expressions {
            [] => {
                return compiler_error!(self, "compile_multi_assign: Missing expression");
            }
            [expression] => {
                let rhs_register = self.push_register()?;
                self.compile_node(Some(rhs_register), ast.node(*expression), ast)?;

                for (i, target) in targets.iter().enumerate() {
                    match &ast.node(target.target_index).node {
                        Node::Id(id_index) => {
                            if let Some(capture_slot) = self.frame().capture_slot(*id_index) {
                                let capture_register = self.push_register()?;

                                self.push_op(
                                    ValueIndex,
                                    &[capture_register, rhs_register, i as u8],
                                );
                                self.push_op(SetCapture, &[capture_slot, capture_register]);

                                self.pop_register()?;
                            } else {
                                let local_register = self.assign_local_register(*id_index)?;

                                self.push_op(ValueIndex, &[local_register, rhs_register, i as u8]);
                            }
                        }
                        Node::Lookup(lookup) => {
                            let register = self.push_register()?;

                            self.push_op(ValueIndex, &[register, rhs_register, i as u8]);
                            self.compile_lookup(None, &lookup, Some(register), ast)?;

                            self.pop_register()?;
                        }
                        unexpected => {
                            return compiler_error!(
                                self,
                                "Expected ID or lookup in AST, found {}",
                                unexpected
                            );
                        }
                    };
                }

                if let Some(result_register) = result_register {
                    self.push_op(Copy, &[result_register, rhs_register]);
                }

                self.pop_register()?; // rhs_register
            }
            _ => {
                let mut i = 0;

                let temp_register = self.push_register()?;

                // If we have multiple expressions and a result register,
                // capture the expression results in a list
                if let Some(result_register) = result_register {
                    self.push_op(MakeList, &[result_register, targets.len() as u8]);
                }

                for target in targets.iter() {
                    match expressions.get(i) {
                        Some(expression) => {
                            if let Some(result_register) = result_register {
                                self.compile_assign(
                                    Some(temp_register),
                                    target,
                                    AssignOp::Equal,
                                    *expression,
                                    ast,
                                )?;
                                self.push_op(ListPush, &[result_register, temp_register]);
                            } else {
                                self.compile_assign(
                                    None,
                                    target,
                                    AssignOp::Equal,
                                    *expression,
                                    ast,
                                )?;
                            }
                        }
                        None => {
                            match &ast.node(target.target_index).node {
                                Node::Id(id_index) => {
                                    if let Some(capture) = self.frame().capture_slot(*id_index) {
                                        self.push_op(SetEmpty, &[temp_register]);
                                        self.push_op(SetCapture, &[capture, temp_register]);
                                    } else {
                                        let local_register =
                                            self.assign_local_register(*id_index)?;
                                        self.push_op(SetEmpty, &[local_register]);
                                    }
                                }
                                Node::Lookup(lookup) => {
                                    self.push_op(SetEmpty, &[temp_register]);
                                    self.compile_lookup(None, &lookup, Some(temp_register), ast)?;
                                    self.pop_register()?;
                                }
                                unexpected => {
                                    return compiler_error!(
                                        self,
                                        "Expected ID or lookup in AST, found {}",
                                        unexpected
                                    );
                                }
                            }

                            if let Some(result_register) = result_register {
                                self.push_op(SetEmpty, &[temp_register]);
                                self.push_op(ListPush, &[result_register, temp_register]);
                            }
                        }
                    }

                    i += 1;
                }

                while i < expressions.len() {
                    self.compile_node(None, ast.node(expressions[i]), ast)?;
                    i += 1;
                }

                self.pop_register()?; // temp_register
            }
        }

        Ok(())
    }

    fn compile_load_id(
        &mut self,
        result_register: u8,
        id: ConstantIndex,
    ) -> Result<(), CompilerError> {
        if let Some(local_register) = self.frame().get_local_assigned_register(id) {
            if local_register != result_register {
                self.push_op(Op::Copy, &[result_register, local_register]);
            }
            Ok(())
        } else {
            self.compile_load_non_local_id(result_register, id)
        }
    }

    fn compile_set_global(&mut self, id: ConstantIndex, register: u8) {
        if id <= u8::MAX as u32 {
            self.push_op(Op::SetGlobal, &[id as u8, register]);
        } else {
            self.push_op(Op::SetGlobalLong, &id.to_le_bytes());
            self.push_bytes(&[register]);
        }
    }

    fn compile_load_non_local_id(
        &mut self,
        result_register: u8,
        id: ConstantIndex,
    ) -> Result<(), CompilerError> {
        use Op::*;

        if let Some(capture_slot) = self.frame().capture_slot(id) {
            self.push_op(LoadCapture, &[result_register, capture_slot]);
        } else {
            // global
            if id <= u8::MAX as u32 {
                self.push_op(LoadGlobal, &[result_register, id as u8]);
            } else {
                self.push_op(LoadGlobalLong, &[result_register]);
                self.push_bytes(&id.to_le_bytes());
            }
        }

        Ok(())
    }

    fn compile_import_expression(
        &mut self,
        result_register: Option<u8>,
        from: &[ConstantIndex],
        items: &[Vec<ConstantIndex>],
    ) -> Result<(), CompilerError> {
        use Op::*;

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

                if self.options.repl_mode && self.frame_stack.len() == 1 {
                    self.compile_set_global(*import_id, import_register);
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
                    self.compile_map_access(import_register, access_register, *id);
                    access_register = import_register;
                }

                imported.push(import_register);

                if self.options.repl_mode && self.frame_stack.len() == 1 {
                    self.compile_set_global(*import_id, import_register);
                }
            }

            self.pop_register()?; // from_register
        }

        if let Some(result_register) = result_register {
            match imported.as_slice() {
                [] => return compiler_error!(self, "Missing item to import"),
                [single_item] => {
                    self.push_op(Copy, &[result_register, *single_item]);
                }
                _ => {
                    self.push_op(MakeList, &[result_register, imported.len() as u8]);
                    for item in imported.iter() {
                        self.push_op(ListPush, &[result_register, *item]);
                    }
                }
            }
        }

        Ok(())
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
                    self.compile_map_access(result_register, result_register, *nested_item);
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
        } else if let Some(capture_slot) = self.frame().capture_slot(id) {
            self.push_op(LoadCapture, &[result_register, capture_slot]);
        } else {
            // If the id isn't a local or capture, then it needs to be imported
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
        result_register: Option<u8>,
        try_expression: &AstTry,
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        use Op::*;

        let AstTry {
            try_block,
            catch_arg,
            catch_block,
            finally_block,
        } = &try_expression;

        let try_node = ast.node(*try_block);

        // The argument register for the catch block needs to be assigned now
        // so that it can be included in the TryStart op.
        let catch_register = self.assign_local_register(*catch_arg)?;
        self.push_op(TryStart, &[catch_register]);
        // The catch block start point is defined via an offset from the current byte
        let catch_offset = self.push_offset_placeholder();

        // If there's a finally block then the result of the expression is derived from there
        let try_result_register = if finally_block.is_none() {
            result_register
        } else {
            None
        };
        self.compile_node(try_result_register, try_node, ast)?;

        // Clear the catch point at the end of the try block
        // - if the end of the try block has been reached then the catch block is no longer needed.
        self.push_op(TryEnd, &[]);
        // jump to the finally block
        self.push_op(Jump, &[]);

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

        self.update_offset_placeholder(finally_offset);
        if let Some(finally_block) = finally_block {
            self.compile_node(result_register, ast.node(*finally_block), ast)?;
        }

        Ok(())
    }

    fn compile_binary_op(
        &mut self,
        result_register: Option<u8>,
        op: AstOp,
        lhs: AstIndex,
        rhs: AstIndex,
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        use AstOp::*;

        let lhs_node = ast.node(lhs);
        let rhs_node = ast.node(rhs);

        match op {
            Add | Subtract | Multiply | Divide | Modulo | In => {
                self.compile_op(result_register, op, lhs_node, rhs_node, ast)
            }
            Less | LessOrEqual | Greater | GreaterOrEqual | Equal | NotEqual => {
                self.compile_comparison_op(result_register, op, &lhs_node, &rhs_node, ast)
            }
            And | Or => self.compile_logic_op(result_register, op, lhs, rhs, ast),
        }
    }

    fn compile_op(
        &mut self,
        result_register: Option<u8>,
        op: AstOp,
        lhs: &AstNode,
        rhs: &AstNode,
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        use AstOp::*;

        let op = match op {
            Add => Op::Add,
            Subtract => Op::Subtract,
            Multiply => Op::Multiply,
            Divide => Op::Divide,
            Modulo => Op::Modulo,
            In => Op::In,
            _ => return compiler_error!(self, "Internal error: invalid op"),
        };

        let (lhs_register, pop_lhs) = self.compile_node_or_get_local(result_register, lhs, ast)?;

        // If the result register wasn't used for the lhs, then it's available for the rhs
        let rhs_result_register = match result_register {
            Some(register) if lhs_register != register => Some(register),
            _ => None,
        };

        let (rhs_register, pop_rhs) =
            self.compile_node_or_get_local(rhs_result_register, rhs, ast)?;

        // We only need to do the actual op if there's a result register
        if let Some(result_register) = result_register {
            self.push_op(op, &[result_register, lhs_register, rhs_register]);
        }

        if pop_rhs {
            self.pop_register()?;
        }
        if pop_lhs {
            self.pop_register()?;
        }

        Ok(())
    }

    fn compile_comparison_op(
        &mut self,
        result_register: Option<u8>,
        ast_op: AstOp,
        lhs: &AstNode,
        rhs: &AstNode,
        ast: &Ast,
    ) -> Result<(), CompilerError> {
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

        let stack_count = self.frame().register_stack.len();

        let (result_register, _) = match result_register {
            Some(register) => (register, false),
            None => (self.push_register()?, true),
        };

        let mut jump_offsets = Vec::new();

        let (mut lhs_register, _) = self.compile_node_or_get_local(None, lhs, ast)?;
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

                    let (rhs_lhs_register, _) =
                        self.compile_node_or_get_local(None, ast.node(rhs_lhs), ast)?;

                    // Place the lhs comparison result in the result_register
                    let op = get_comparision_op(ast_op).map_err(|e| self.make_error(e))?;
                    self.push_op(op, &[result_register, lhs_register, rhs_lhs_register]);

                    // Skip evaluating the rhs if the lhs result is false
                    self.push_op(Op::JumpFalse, &[result_register]);
                    jump_offsets.push(self.push_offset_placeholder());

                    lhs_register = rhs_lhs_register;
                    rhs = ast.node(rhs_rhs);
                    ast_op = rhs_ast_op;
                }
                _ => break,
            }
        }

        // Compile the rhs and op for the final rhs in the comparison chain
        let (rhs_register, _) = self.compile_node_or_get_local(None, rhs, ast)?;
        let op = get_comparision_op(ast_op).map_err(|e| self.make_error(e))?;
        self.push_op(op, &[result_register, lhs_register, rhs_register]);

        for jump_offset in jump_offsets.iter() {
            self.update_offset_placeholder(*jump_offset);
        }

        self.truncate_register_stack(stack_count)?;

        Ok(())
    }

    fn compile_logic_op(
        &mut self,
        result_register: Option<u8>,
        op: AstOp,
        lhs: AstIndex,
        rhs: AstIndex,
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        let register = if let Some(result_register) = result_register {
            result_register
        } else {
            self.push_register()?
        };
        self.compile_node(Some(register), ast.node(lhs), ast)?;

        let jump_op = match op {
            AstOp::And => Op::JumpFalse,
            AstOp::Or => Op::JumpTrue,
            _ => unreachable!(),
        };

        self.push_op(jump_op, &[register]);

        // If the lhs causes a jump then that's the result,
        // otherwise the rhs is the result
        self.compile_node_with_jump_offset(Some(register), ast.node(rhs), ast)?;

        if result_register.is_none() {
            self.pop_register()?;
        }

        Ok(())
    }

    fn compile_single_register_op(
        &mut self,
        op: Op,
        result_register: Option<u8>,
        expression: AstIndex,
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        if let Some(result_register) = result_register {
            self.compile_node(Some(result_register), ast.node(expression), ast)?;
            self.push_op(op, &[result_register]);
        } else {
            let register = self.push_register()?;
            self.compile_node(Some(register), ast.node(expression), ast)?;
            self.push_op(op, &[register]);
            self.pop_register()?;
        }

        Ok(())
    }

    fn compile_make_num2(
        &mut self,
        result_register: Option<u8>,
        elements: &[AstIndex],
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        if elements.is_empty() || elements.len() > 2 {
            return compiler_error!(
                self,
                "compile_make_num2: unexpected number of elements: {}",
                elements.len()
            );
        }

        if let Some(result_register) = result_register {
            let stack_count = self.frame().register_stack.len();

            for element_node in elements.iter() {
                let element_register = self.push_register()?;
                self.compile_node(Some(element_register), ast.node(*element_node), ast)?;
            }

            let first_element_register = self.peek_register(elements.len() - 1)?;
            self.push_op(
                Op::MakeNum2,
                &[
                    result_register,
                    elements.len() as u8,
                    first_element_register,
                ],
            );

            self.truncate_register_stack(stack_count)?;
        } else {
            for element_node in elements.iter() {
                self.compile_node(None, ast.node(*element_node), ast)?;
            }
        }

        Ok(())
    }

    fn compile_make_num4(
        &mut self,
        result_register: Option<u8>,
        elements: &[AstIndex],
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        if elements.is_empty() || elements.len() > 4 {
            return compiler_error!(
                self,
                "compile_make_num4: unexpected number of elements: {}",
                elements.len()
            );
        }

        if let Some(result_register) = result_register {
            let stack_count = self.frame().register_stack.len();

            for element_node in elements.iter() {
                let element_register = self.push_register()?;
                self.compile_node(Some(element_register), ast.node(*element_node), ast)?;
            }

            let first_element_register = self.peek_register(elements.len() - 1)?;
            self.push_op(
                Op::MakeNum4,
                &[
                    result_register,
                    elements.len() as u8,
                    first_element_register,
                ],
            );

            self.truncate_register_stack(stack_count)?;
        } else {
            for element_node in elements.iter() {
                self.compile_node(None, ast.node(*element_node), ast)?;
            }
        }

        Ok(())
    }

    fn compile_make_list(
        &mut self,
        result_register: Option<u8>,
        elements: &[AstIndex],
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        use Op::*;

        if let Some(result_register) = result_register {
            // TODO take ranges into account when determining size hint
            let size_hint = elements.len();
            if size_hint <= u8::MAX as usize {
                self.push_op(MakeList, &[result_register, size_hint as u8]);
            } else {
                self.push_op(MakeListLong, &[result_register]);
                self.push_bytes(&size_hint.to_le_bytes());
            }

            let element_register = self.push_register()?;
            for element_node in elements.iter() {
                match &ast.node(*element_node).node {
                    Node::For(for_loop) => {
                        self.compile_for(
                            Some(element_register),
                            Some(result_register),
                            &for_loop,
                            ast,
                        )?;
                    }
                    Node::While { condition, body } => {
                        self.compile_while(
                            Some(element_register),
                            Some(result_register),
                            *condition,
                            *body,
                            false,
                            ast,
                        )?;
                    }
                    Node::Until { condition, body } => {
                        self.compile_while(
                            Some(element_register),
                            Some(result_register),
                            *condition,
                            *body,
                            true,
                            ast,
                        )?;
                    }
                    _ => {
                        self.compile_node(Some(element_register), ast.node(*element_node), ast)?;
                        self.push_op_without_span(ListPush, &[result_register, element_register]);
                    }
                }
            }
            self.pop_register()?; // element_register
        } else {
            for element_node in elements.iter() {
                self.compile_node(None, ast.node(*element_node), ast)?;
            }
        }

        Ok(())
    }

    fn compile_make_map(
        &mut self,
        result_register: Option<u8>,
        entries: &[(ConstantIndex, Option<AstIndex>)],
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        use Op::*;

        if let Some(result_register) = result_register {
            let size_hint = entries.len();
            if size_hint <= u8::MAX as usize {
                self.push_op(MakeMap, &[result_register, size_hint as u8]);
            } else {
                self.push_op(MakeMapLong, &[result_register]);
                self.push_bytes(&size_hint.to_le_bytes());
            }

            for (key, maybe_value_node) in entries.iter() {
                let (value_register, pop_value_register) = match maybe_value_node {
                    Some(value_node) => {
                        let value_node = ast.node(*value_node);
                        let register = self.push_register()?;
                        self.compile_node(Some(register), value_node, ast)?;
                        (register, true)
                    }
                    None => {
                        if let Some(register) = self.frame().get_local_assigned_register(*key) {
                            (register, false)
                        } else {
                            let register = self.push_register()?;
                            self.compile_load_non_local_id(register, *key)?;
                            (register, true)
                        }
                    }
                };

                self.compile_map_insert(result_register, value_register, *key);

                if pop_value_register {
                    self.pop_register()?;
                }
            }
        } else {
            // Evaluate value nodes to ensure functions are called as expected
            for (_key, value_node) in entries.iter() {
                let value_node = value_node.unwrap(); // TODO
                self.compile_node(None, ast.node(value_node), ast)?;
            }
        }

        Ok(())
    }

    fn compile_function(
        &mut self,
        result_register: Option<u8>,
        function: &Function,
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        use Op::*;

        if let Some(result_register) = result_register {
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

            if function.is_instance_function {
                self.push_op(
                    InstanceFunction,
                    &[result_register, arg_count - 1, capture_count],
                );
            } else {
                self.push_op(Function, &[result_register, arg_count, capture_count]);
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

            match &ast.node(function.body).node {
                Node::Block(expressions) => {
                    self.compile_frame(local_count, &expressions, &function.args, &captures, ast)?;
                }
                _ => {
                    self.compile_frame(
                        local_count,
                        &[function.body],
                        &function.args,
                        &captures,
                        ast,
                    )?;
                }
            };

            self.update_offset_placeholder(function_size_ip);

            for (i, capture) in captures.iter().enumerate() {
                if let Some(local_register) = self.frame().get_local_register(*capture) {
                    self.push_op(Capture, &[result_register, i as u8, local_register]);
                } else {
                    let capture_register = self.push_register()?;
                    self.compile_load_non_local_id(capture_register, *capture)?;

                    self.push_op(Capture, &[result_register, i as u8, capture_register]);

                    self.pop_register()?;
                }
            }
        }

        Ok(())
    }

    fn compile_lookup(
        &mut self,
        result_register: Option<u8>,
        lookup: &[LookupNode],
        set_value: Option<u8>,
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        use Op::*;

        let lookup_len = lookup.len();
        if lookup_len < 2 {
            return compiler_error!(
                self,
                "compile_lookup: lookup requires at least 2 elements, found {}",
                lookup_len
            );
        }

        // Keep track of a register for each lookup node.
        // This produces a lookup chain, allowing lookup operations to access parent containers.
        let mut node_registers = SmallVec::<[u8; 4]>::new();

        // At the end of the lookup we'll pop the whole stack,
        // so we don't need to keep track of how many temporary registers we use.
        let stack_count = self.frame().register_stack.len();

        for (i, lookup_node) in lookup.iter().enumerate() {
            let is_last_node = i == lookup.len() - 1;

            match lookup_node {
                LookupNode::Id(id) => {
                    if i == 0 {
                        // Root node

                        if let Some(local_register) = self.frame().get_local_assigned_register(*id)
                        {
                            node_registers.push(local_register);
                        } else {
                            let node_register = self.push_register()?;
                            node_registers.push(node_register);
                            self.compile_load_non_local_id(node_register, *id)?;
                        }
                    } else {
                        // Map access
                        let map_register = *node_registers.last().expect("Empty node registers");

                        if is_last_node {
                            if let Some(set_value) = set_value {
                                self.compile_map_insert(map_register, set_value, *id);
                            } else if let Some(result_register) = result_register {
                                self.compile_map_access(result_register, map_register, *id);
                            }
                        } else {
                            let node_register = self.push_register()?;
                            node_registers.push(node_register);
                            self.compile_map_access(node_register, map_register, *id);
                        }
                    }
                }
                LookupNode::Index(index_node) => {
                    // List index

                    let (index_register, _) =
                        self.compile_node_or_get_local(None, ast.node(*index_node), ast)?;
                    let list_register = *node_registers.last().expect("Empty node registers");

                    if is_last_node {
                        if let Some(set_value) = set_value {
                            self.push_op(ListUpdate, &[list_register, index_register, set_value]);
                        } else if let Some(result_register) = result_register {
                            self.push_op(
                                ListIndex,
                                &[result_register, list_register, index_register],
                            );
                        }
                    } else {
                        let node_register = self.push_register()?;
                        node_registers.push(node_register);
                        self.push_op(ListIndex, &[node_register, list_register, index_register]);
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
                        self.compile_call(
                            result_register,
                            function_register,
                            &args,
                            parent_register,
                            ast,
                        )?;
                    } else {
                        let node_register = self.push_register()?;
                        node_registers.push(node_register);
                        self.compile_call(
                            Some(node_register),
                            function_register,
                            &args,
                            parent_register,
                            ast,
                        )?;
                    }
                }
            }
        }

        self.truncate_register_stack(stack_count)?;

        Ok(())
    }

    fn compile_map_insert(&mut self, map_register: u8, value_register: u8, key: ConstantIndex) {
        if key <= u8::MAX as u32 {
            self.push_op(Op::MapInsert, &[map_register, value_register, key as u8]);
        } else {
            self.push_op(Op::MapInsertLong, &[map_register, value_register]);
            self.push_bytes(&key.to_le_bytes());
        }
    }

    fn compile_map_access(&mut self, result_register: u8, map_register: u8, key: ConstantIndex) {
        if key <= u8::MAX as u32 {
            self.push_op(Op::MapAccess, &[result_register, map_register, key as u8]);
        } else {
            self.push_op(Op::MapAccessLong, &[result_register, map_register]);
            self.push_bytes(&key.to_le_bytes());
        }
    }

    fn compile_call(
        &mut self,
        result_register: Option<u8>,
        function_register: u8,
        args: &[AstIndex],
        parent: Option<u8>,
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        use Op::*;

        let stack_count = self.frame().register_stack.len();

        let frame_base = if args.is_empty() {
            self.push_register()?
        } else {
            self.frame().next_temporary_register()
        };

        for arg in args.iter() {
            let arg_register = self.push_register()?;
            self.compile_node(Some(arg_register), ast.node(*arg), ast)?;
        }

        let call_result_register = result_register.unwrap_or(frame_base);

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

        Ok(())
    }

    fn compile_if(
        &mut self,
        result_register: Option<u8>,
        ast_if: &AstIf,
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        use Op::*;

        let AstIf {
            condition,
            then_node,
            else_if_blocks,
            else_node,
        } = ast_if;

        let condition_register = self.push_register()?;
        self.compile_node(Some(condition_register), ast.node(*condition), ast)?;
        self.push_op_without_span(JumpFalse, &[condition_register]);
        let if_jump_ip = self.push_offset_placeholder();
        self.pop_register()?;

        self.compile_node(result_register, ast.node(*then_node), ast)?;

        let then_jump_ip = {
            if !else_if_blocks.is_empty() || else_node.is_some() {
                self.push_op_without_span(Jump, &[]);
                Some(self.push_offset_placeholder())
            } else {
                None
            }
        };

        self.update_offset_placeholder(if_jump_ip);

        let else_if_jump_ips = else_if_blocks
            .iter()
            .map(
                |(else_if_condition, else_if_node)| -> Result<Option<usize>, CompilerError> {
                    let condition_register = self.push_register()?;
                    self.compile_node(Some(condition_register), ast.node(*else_if_condition), ast)?;

                    self.push_op_without_span(JumpFalse, &[condition_register]);
                    let then_jump_ip = self.push_offset_placeholder();

                    self.pop_register()?; // condition register

                    self.compile_node(result_register, ast.node(*else_if_node), ast)?;

                    let else_if_jump_ip = if else_node.is_some() {
                        self.push_op_without_span(Jump, &[]);
                        Some(self.push_offset_placeholder())
                    } else {
                        None
                    };
                    self.update_offset_placeholder(then_jump_ip);

                    Ok(else_if_jump_ip)
                },
            )
            .collect::<Result<Vec<_>, _>>()?;

        if let Some(else_node) = else_node {
            self.compile_node(result_register, ast.node(*else_node), ast)?;
        } else if let Some(result_register) = result_register {
            self.push_op_without_span(SetEmpty, &[result_register]);
        }

        if let Some(then_jump_ip) = then_jump_ip {
            self.update_offset_placeholder(then_jump_ip);
        }

        for else_if_jump_ip in else_if_jump_ips.iter() {
            // When there's no else block, the final else_if doesn't have a jump
            if let Some(else_if_jump_ip) = else_if_jump_ip {
                self.update_offset_placeholder(*else_if_jump_ip);
            }
        }

        Ok(())
    }

    fn compile_match(
        &mut self,
        result_register: Option<u8>,
        expression: AstIndex,
        arms: &[MatchArm],
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        use Op::*;

        let stack_count = self.frame().register_stack.len();

        let match_register = self.push_register()?;
        self.compile_node(Some(match_register), ast.node(expression), ast)?;

        let mut result_jump_placeholders = Vec::new();

        for (arm_index, arm) in arms.iter().enumerate() {
            let mut arm_jump_placeholders = Vec::new();

            for (pattern_index, pattern) in arm.patterns.iter().enumerate() {
                let pattern_node = ast.node(*pattern);

                match pattern_node.node {
                    Node::Empty
                    | Node::BoolTrue
                    | Node::BoolFalse
                    | Node::Number0
                    | Node::Number1
                    | Node::Number(_)
                    | Node::Str(_) => {
                        let pattern_register = self.push_register()?;
                        self.compile_node(Some(pattern_register), pattern_node, ast)?;
                        let comparison_register = self.push_register()?;

                        if arm.patterns.len() == 1 {
                            self.push_op_without_span(
                                Equal,
                                &[comparison_register, pattern_register, match_register],
                            );
                        } else {
                            let element_register = self.push_register()?;

                            self.push_op_without_span(
                                ValueIndex,
                                &[element_register, match_register, pattern_index as u8],
                            );

                            self.push_op_without_span(
                                Equal,
                                &[comparison_register, pattern_register, element_register],
                            );

                            self.pop_register()?; // element_register
                        }

                        self.push_op_without_span(Op::JumpFalse, &[comparison_register]);
                        arm_jump_placeholders.push(self.push_offset_placeholder());

                        self.pop_register()?; // comparison_register
                        self.pop_register()?; // pattern_register
                    }
                    Node::Id(id) => {
                        self.span_stack.push(*ast.span(pattern_node.span));

                        let id_register = self.assign_local_register(id)?;
                        if arm.patterns.len() == 1 {
                            self.push_op(Copy, &[id_register, match_register]);
                        } else {
                            self.push_op(
                                ValueIndex,
                                &[id_register, match_register, pattern_index as u8],
                            );
                        }

                        self.span_stack.pop();
                    }
                    Node::Wildcard => {}
                    _ => {
                        return compiler_error!(self, "Internal error: invalid match pattern");
                    }
                }
            }

            if let Some(condition) = arm.condition {
                let condition_register = self.push_register()?;
                self.compile_node(Some(condition_register), ast.node(condition), ast)?;

                self.push_op(Op::JumpFalse, &[condition_register]);
                arm_jump_placeholders.push(self.push_offset_placeholder());

                self.pop_register()?; // condition_register
            }

            self.compile_node(result_register, ast.node(arm.expression), ast)?;

            if arm_index < arms.len() - 1 {
                self.push_op_without_span(Op::Jump, &[]);
                result_jump_placeholders.push(self.push_offset_placeholder());
            }

            for jump_placeholder in arm_jump_placeholders.iter() {
                self.update_offset_placeholder(*jump_placeholder);
            }
        }

        for jump_placeholder in result_jump_placeholders.iter() {
            self.update_offset_placeholder(*jump_placeholder);
        }

        self.truncate_register_stack(stack_count)?;

        Ok(())
    }

    fn compile_for(
        &mut self,
        result_register: Option<u8>, // register that gets the last iteration's result
        list_register: Option<u8>,   // list that receives each iteration's result
        ast_for: &AstFor,
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        use Op::*;

        let AstFor {
            args,
            ranges,
            condition,
            body,
        } = &ast_for;

        //   make iterator, iterator_register
        //   make local registers for args
        // loop_start:
        //   iterator_next_or_jump iterator_register arg_register jump -> end
        //   if condition
        //     condition_body
        //     if body result false jump -> loop_start
        //   loop body
        //   jump -> loop_start
        // end:

        if ranges.len() > 1 && args.len() != ranges.len() {
            return compiler_error!(
                self,
                "compile_for: argument and range count mismatch: {} vs {}",
                args.len(),
                ranges.len()
            );
        }

        let stack_count = self.frame().register_stack.len();

        let iterator_register = match ranges.as_slice() {
            [] => {
                return compiler_error!(self, "compile_for: Missing range");
            }
            [range_node] => {
                let iterator_register = self.push_register()?;
                let range_register = self.push_register()?;
                self.compile_node(Some(range_register), ast.node(*range_node), ast)?;

                self.push_op_without_span(MakeIterator, &[iterator_register, range_register]);
                self.pop_register()?; // range register

                iterator_register
            }
            _ => {
                let mut first_iterator_register = None;
                for range_node in ranges.iter() {
                    let iterator_register = self.push_register()?;
                    let range_register = self.push_register()?;
                    self.compile_node(Some(range_register), ast.node(*range_node), ast)?;

                    self.push_op_without_span(MakeIterator, &[iterator_register, range_register]);
                    self.pop_register()?; // range register

                    if first_iterator_register.is_none() {
                        first_iterator_register = Some(iterator_register);
                    }
                }
                first_iterator_register.unwrap()
            }
        };

        let loop_start_ip = self.bytes.len();
        self.frame_mut().loop_stack.push(Loop::new(loop_start_ip));

        if args.len() > 1 && ranges.len() == 1 {
            // e.g. for a, b, c in list_of_lists()
            // e.g. for key, value in map
            let temp_register = self.push_register()?;

            self.push_op_without_span(IteratorNext, &[temp_register, iterator_register]);
            self.push_loop_jump_placeholder()?;

            for (i, arg) in args.iter().enumerate() {
                let arg_register = self.assign_local_register(*arg)?;
                self.push_op_without_span(ValueIndex, &[arg_register, temp_register, i as u8]);
            }

            self.pop_register()?; // temp_register
        } else {
            for (i, arg) in args.iter().enumerate() {
                let arg_register = self.assign_local_register(*arg)?;
                self.push_op_without_span(
                    IteratorNext,
                    &[arg_register, iterator_register + i as u8],
                );
                self.push_loop_jump_placeholder()?;
            }
        }

        if let Some(condition) = condition {
            let condition_register = self.push_register()?;
            self.compile_node(Some(condition_register), ast.node(*condition), ast)?;
            self.push_jump_back_op(JumpBackFalse, &[condition_register], loop_start_ip);
            self.pop_register()?;
        }

        self.compile_node(result_register, ast.node(*body), ast)?;

        if let Some(list_register) = list_register {
            match result_register {
                Some(result_register) => {
                    self.push_op_without_span(ListPush, &[list_register, result_register])
                }
                None => {
                    return compiler_error!(
                        self,
                        "compile_for: Missing result register for list expansion"
                    )
                }
            }
        }

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

        Ok(())
    }

    fn compile_while(
        &mut self,
        result_register: Option<u8>, // register that gets the last iteration's result
        list_register: Option<u8>,   // list that receives each iteration's result
        condition: AstIndex,
        body: AstIndex,
        negate_condition: bool,
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        use Op::*;

        let loop_start_ip = self.bytes.len();
        self.frame_mut().loop_stack.push(Loop::new(loop_start_ip));

        let condition_register = self.push_register()?;
        self.compile_node(Some(condition_register), ast.node(condition), ast)?;
        let op = if negate_condition {
            JumpTrue
        } else {
            JumpFalse
        };
        self.push_op_without_span(op, &[condition_register]);
        self.push_loop_jump_placeholder()?;
        self.pop_register()?; // condition register

        self.compile_node(result_register, ast.node(body), ast)?;

        if let Some(list_register) = list_register {
            match result_register {
                Some(result_register) => {
                    self.push_op_without_span(ListPush, &[list_register, result_register]);
                }
                None => {
                    if result_register.is_none() {
                        return compiler_error!(
                            self,
                            "compile_while: Missing result register for list expansion"
                        );
                    }
                }
            }
        }

        self.push_jump_back_op(JumpBack, &[], loop_start_ip);

        match self.frame_mut().loop_stack.pop() {
            Some(loop_info) => {
                for placeholder in loop_info.jump_placeholders.iter() {
                    self.update_offset_placeholder(*placeholder);
                }
            }
            None => return compiler_error!(self, "Empty loop info stack"),
        }

        Ok(())
    }

    fn load_string(&mut self, result_register: u8, index: ConstantIndex) {
        use Op::*;

        if index <= u8::MAX as u32 {
            self.push_op(LoadString, &[result_register, index as u8]);
        } else {
            self.push_op(LoadStringLong, &[result_register]);
            self.push_bytes(&index.to_le_bytes());
        }
    }

    fn compile_node_with_jump_offset(
        &mut self,
        result_register: Option<u8>,
        node: &AstNode,
        ast: &Ast,
    ) -> Result<(), CompilerError> {
        let offset_ip = self.push_offset_placeholder();
        self.compile_node(result_register, node, ast)?;
        self.update_offset_placeholder(offset_ip);
        Ok(())
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

    fn commit_local_register(&mut self, local_register: u8) -> Result<(), CompilerError> {
        self.frame_mut()
            .commit_local_register(local_register)
            .map_err(|e| self.make_error(e))
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
