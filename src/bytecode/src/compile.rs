use crate::{Bytecode, Op};

use koto_parser::{AssignTarget, AstIf, AstNode, AstOp, AstWhile, ConstantIndex, LookupOrId, Node};
use std::convert::TryFrom;

#[derive(Clone, Debug)]
struct Frame {
    register_stack: Vec<u8>,
    local_registers: Vec<ConstantIndex>,
    temporary_base: u8,
    temporary_count: u8,
}

impl Frame {
    fn new(local_count: u8, args: &[ConstantIndex]) -> Self {
        let mut local_registers = Vec::with_capacity(local_count as usize);
        local_registers.extend_from_slice(args);

        Self {
            register_stack: Vec::with_capacity(local_count as usize),
            local_registers,
            temporary_base: local_count,
            temporary_count: 0,
        }
    }

    fn get_register(&mut self) -> Result<u8, String> {
        let new_register = self.temporary_base + self.temporary_count;
        self.temporary_count += 1;

        if new_register > u8::MAX {
            Err("Reached maximum number of registers".to_string())
        } else {
            self.register_stack.push(new_register);
            Ok(new_register)
        }
    }

    fn is_local(&self, index: ConstantIndex) -> bool {
        self.local_registers
            .iter()
            .any(|constant_index| index == *constant_index)
    }

    fn get_local_register(&mut self, local: ConstantIndex) -> Result<u8, String> {
        let local_register = match self
            .local_registers
            .iter()
            .position(|constant_index| local == *constant_index)
        {
            Some(assigned) => assigned,
            None => {
                self.local_registers.push(local);
                let new_local_register = self.local_registers.len() - 1;

                if new_local_register > self.temporary_base as usize {
                    return Err("get_local_register: Locals overflowed".to_string());
                }

                new_local_register
            }
        } as u8;

        self.register_stack.push(local_register);

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

    fn peek_register(&self) -> Option<&u8> {
        self.register_stack.last()
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
}

#[derive(Default)]
pub struct Compiler {
    bytes: Bytecode,
    frame_stack: Vec<Frame>,
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub fn compile_ast(&mut self, ast: &AstNode) -> Result<&Bytecode, String> {
        // dbg!(ast);

        self.compile_node(ast)?;

        Ok(&self.bytes)
    }

    fn compile_frame(
        &mut self,
        local_count: u8,
        expressions: &[AstNode],
        args: &[ConstantIndex],
    ) -> Result<(), String> {
        self.frame_stack.push(Frame::new(local_count, args));

        self.compile_expressions(expressions)?;

        let result_register = self.frame_mut().pop_register()?;
        self.push_bytes(&[Op::Return.into(), result_register]);

        self.frame_stack.pop();

        Ok(())
    }

    fn compile_expressions(&mut self, expressions: &[AstNode]) -> Result<(), String> {
        use Op::*;

        let mut result_register = None;

        for expression in expressions.iter() {
            self.compile_node(expression)?;
            result_register = Some(self.frame_mut().pop_register()?);
        }

        if let Some(result_register) = result_register {
            let register = self.frame_mut().get_register()?;
            if register != result_register {
                self.push_bytes(&[Copy.into(), register, result_register]);
            }
        } else {
            let register = self.frame_mut().get_register()?;
            self.push_bytes(&[SetEmpty.into(), register]);
        }

        Ok(())
    }

    fn frame(&self) -> &Frame {
        self.frame_stack.last().expect("Frame stack is empty")
    }

    fn frame_mut(&mut self) -> &mut Frame {
        self.frame_stack.last_mut().expect("Frame stack is empty")
    }

    fn compile_node(&mut self, node: &AstNode) -> Result<(), String> {
        use Op::*;

        match &node.node {
            Node::Empty => {
                self.push_empty()?;
            }
            Node::Id(index) => {
                if self.frame().is_local(*index) {
                    self.frame_mut().get_local_register(*index)?;
                } else {
                    self.load_global(*index)?;
                }
            }
            Node::BoolTrue => {
                let target = self.frame_mut().get_register()?;
                self.push_op(SetTrue, &[target]);
            }
            Node::BoolFalse => {
                let target = self.frame_mut().get_register()?;
                self.push_op(SetFalse, &[target]);
            }
            Node::Number(constant) => {
                let target = self.frame_mut().get_register()?;
                let constant = *constant;
                if constant <= u8::MAX as u32 {
                    self.push_op(LoadNumber, &[target, constant as u8]);
                } else {
                    self.push_op(LoadNumberLong, &[target]);
                    self.push_bytes(&constant.to_le_bytes());
                }
            }
            Node::Str(constant) => {
                let target = self.frame_mut().get_register()?;
                let constant = *constant;
                if constant <= u8::MAX as u32 {
                    self.push_op(LoadString, &[target, constant as u8]);
                } else {
                    self.push_op(LoadStringLong, &[target]);
                    self.push_bytes(&constant.to_le_bytes());
                }
            }
            Node::MainBlock { body, local_count } => {
                self.compile_frame(*local_count as u8, body, &[])?;
            }
            Node::Block(expressions) => {
                self.compile_expressions(expressions)?;
            }
            Node::Function(function) => {
                let target = self.frame_mut().get_register()?;
                let arg_count = match u8::try_from(function.args.len()) {
                    Ok(x) => x,
                    Err(_) => {
                        return Err(format!(
                            "Function has too many arguments: {}",
                            function.args.len()
                        ));
                    }
                };
                self.push_op(MakeFunction, &[target, arg_count]);
                let function_size_ip = self.push_offset_placeholder();

                let local_count = match u8::try_from(function.local_count) {
                    Ok(x) => x,
                    Err(_) => {
                        return Err(format!(
                            "Function has too many locals: {}",
                            function.args.len()
                        ));
                    }
                };

                self.compile_frame(local_count, &function.body, &function.args)?;
                self.update_offset_placeholder(function_size_ip);
            }
            Node::Call { function, args } => {
                let function_register = match function {
                    LookupOrId::Id(id) => {
                        let id = *id;
                        if self.frame().is_local(id) {
                            self.frame_mut().get_local_register(id)?
                        } else {
                            self.load_global(id)?
                        }
                    }
                    _ => unimplemented!(),
                };

                let stack_count = self.frame().register_stack.len();

                let first_arg_register = if !args.is_empty() {
                    self.frame().next_temporary_register()
                } else {
                    0
                };

                for arg in args.iter() {
                    self.compile_node(&arg)?;

                    // If the arg value is in a local register, then it needs to be copied to
                    // an argument register
                    let frame = self.frame_mut();
                    if *frame.peek_register().unwrap() < frame.temporary_base {
                        let source = frame.pop_register()?;
                        let target = frame.get_register()?;
                        self.push_op(Copy, &[target, source]);
                    }
                }

                self.push_op(
                    Call,
                    &[function_register, first_arg_register, args.len() as u8],
                );

                // The return value gets placed in the function call register
                // TODO multiple return values
                self.frame_mut().truncate_register_stack(stack_count + 1)?;
            }
            Node::Assign { target, expression } => {
                self.compile_node(expression)?;
                let source = self.frame_mut().pop_register()?;
                let target_id = match target {
                    AssignTarget::Id { id_index, .. } => id_index,
                    AssignTarget::Lookup(_lookup) => unimplemented!(),
                };
                let target = self.frame_mut().get_local_register(*target_id)?;
                if target != source {
                    self.push_op(Copy, &[target, source]);
                }
            }
            Node::Op { op, lhs, rhs } => {
                let op = match op {
                    AstOp::Add => Add,
                    AstOp::Multiply => Multiply,
                    AstOp::Less => Less,
                    AstOp::Greater => Greater,
                    AstOp::Equal => Equal,
                    AstOp::NotEqual => NotEqual,
                    AstOp::And | AstOp::Or => {
                        self.compile_node(&lhs)?;
                        let lhs_register = self.frame_mut().pop_register()?;
                        let jump_op = if matches!(op, AstOp::And) {
                            JumpFalse
                        } else {
                            JumpTrue
                        };

                        self.push_op(jump_op, &[lhs_register]);
                        self.compile_node_with_jump_offset(&rhs)?;

                        return Ok(());
                    }
                    unexpected => unimplemented!("Missing AstOp: {:?}", unexpected),
                };

                self.compile_node(&lhs)?;
                self.compile_node(&rhs)?;

                let frame = self.frame_mut();
                let rhs_register = frame.pop_register()?;
                let lhs_register = frame.pop_register()?;
                let target = frame.get_register()?;
                self.push_op(op, &[target, lhs_register, rhs_register]);
            }
            Node::If(ast_if) => self.compile_if(ast_if)?,
            Node::While(ast_while) => self.compile_while(ast_while)?,
            unexpected => unimplemented!("compile_node: unsupported node: {}", unexpected),
        }

        Ok(())
    }

    fn compile_if(&mut self, ast_if: &AstIf) -> Result<(), String> {
        use Op::*;

        let AstIf {
            condition,
            then_node,
            else_if_condition,
            else_if_node,
            else_node,
        } = ast_if;

        self.compile_node(&condition)?;
        let condition_register = self.frame_mut().pop_register()?;

        self.push_op(JumpFalse, &[condition_register]);
        let if_jump_ip = self.push_offset_placeholder();

        let stack_count = self.frame().register_stack.len();
        self.compile_node(&then_node)?;
        self.frame_mut().truncate_register_stack(stack_count)?;

        let then_jump_ip = {
            if else_if_node.is_some() || else_node.is_some() {
                self.push_op(Jump, &[]);
                Some(self.push_offset_placeholder())
            } else {
                None
            }
        };

        self.update_offset_placeholder(if_jump_ip);

        let else_if_jump_ip = if let Some(condition) = else_if_condition {
            // TODO combine condition and node in ast
            let else_if_node = else_if_node.as_ref().unwrap();

            self.compile_node(&condition)?;
            let condition_register = self.frame_mut().pop_register()?;
            self.push_op(JumpFalse, &[condition_register]);

            let stack_count = self.frame().register_stack.len();
            self.compile_node_with_jump_offset(&else_if_node)?;
            self.frame_mut().truncate_register_stack(stack_count)?;

            if else_node.is_some() {
                self.push_op(Jump, &[]);
                Some(self.push_offset_placeholder())
            } else {
                None
            }
        } else {
            None
        };

        if let Some(else_node) = else_node {
            self.compile_node(else_node)?;
        }

        if let Some(then_jump_ip) = then_jump_ip {
            self.update_offset_placeholder(then_jump_ip);
        }

        if let Some(else_if_jump_ip) = else_if_jump_ip {
            self.update_offset_placeholder(else_if_jump_ip);
        }

        Ok(())
    }

    fn compile_while(&mut self, ast_while: &AstWhile) -> Result<(), String> {
        use Op::*;

        let AstWhile {
            condition,
            body,
            negate_condition,
        } = ast_while;

        let while_jump_ip = self.bytes.len();

        self.compile_node(&condition)?;
        let condition_register = self.frame_mut().pop_register()?;
        let op = if *negate_condition {
            JumpTrue
        } else {
            JumpFalse
        };
        self.push_op(op, &[condition_register]);
        let condition_jump_ip = self.push_offset_placeholder();

        self.compile_node(&body)?;
        self.push_jump_back(while_jump_ip);

        self.update_offset_placeholder(condition_jump_ip);
        self.push_empty()?;

        Ok(())
    }

    fn load_global(&mut self, index: ConstantIndex) -> Result<u8, String> {
        use Op::*;

        let register = self.frame_mut().get_register()?;
        if index <= u8::MAX as u32 {
            self.push_bytes(&[LoadGlobal.into(), register, index as u8]);
        } else {
            self.push_bytes(&[LoadGlobalLong.into(), register]);
            self.push_bytes(&index.to_le_bytes());
        }
        Ok(register)
    }

    fn compile_node_with_jump_offset(&mut self, node: &AstNode) -> Result<(), String> {
        let offset_ip = self.push_offset_placeholder();
        self.compile_node(&node)?;
        self.update_offset_placeholder(offset_ip);
        Ok(())
    }

    fn push_empty(&mut self) -> Result<(), String> {
        let target = self.frame_mut().get_register()?;
        self.push_op(Op::SetEmpty, &[target]);
        Ok(())
    }

    fn push_jump_back(&mut self, target_ip: usize) {
        let offset = self.bytes.len() - target_ip + 3; // 3 for the jump instruction
        self.push_op(Op::JumpBack, &(offset as u16).to_le_bytes())
    }

    fn push_offset_placeholder(&mut self) -> usize {
        let offset_ip = self.bytes.len();
        self.push_bytes(&[0, 0]);
        offset_ip
    }

    fn update_offset_placeholder(&mut self, offset_ip: usize) {
        let offset = self.bytes.len() - offset_ip - 2;
        let offset_bytes = (offset as u16).to_le_bytes();
        self.bytes[offset_ip] = offset_bytes[0];
        self.bytes[offset_ip + 1] = offset_bytes[1];
    }

    fn push_op(&mut self, op: Op, bytes: &[u8]) {
        self.bytes.push(op.into());
        self.bytes.extend_from_slice(bytes);
    }

    fn push_bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }
}
