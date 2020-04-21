use crate::{Bytecode, Op};

use koto_parser::{AssignTarget, AstIf, AstNode, AstOp, ConstantIndex, Node};

const BYTE_MAX: u32 = std::u8::MAX as u32;

#[derive(Clone)]
enum Register {
    Inactive,
    Active,
    Local,
}

#[derive(Clone, Default)]
struct Frame {
    registers: Vec<Register>,
    register_stack: Vec<u8>,
    local_registers: Vec<(ConstantIndex, u8)>,
}

impl Frame {
    fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    fn get_register(&mut self) -> Result<u8, String> {
        let register = match self
            .registers
            .iter()
            .position(|assigned| matches!(assigned, Register::Inactive))
        {
            Some(inactive) => {
                self.registers[inactive] = Register::Active;
                inactive
            }
            None => {
                self.registers.push(Register::Active);
                let new_register = self.registers.len() - 1;
                if new_register > BYTE_MAX as usize {
                    return Err("Reached maximum number of registers".to_string());
                }
                new_register
            }
        } as u8;

        self.register_stack.push(register);
        Ok(register)
    }

    fn get_local_register(&mut self, local: ConstantIndex) -> Result<u8, String> {
        let local_register = match self
            .local_registers
            .iter()
            .find(|(index, _)| local == *index)
        {
            Some((_, assigned)) => {
                self.register_stack.push(*assigned);
                *assigned
            }
            None => {
                let new_register = self.get_register()?;
                self.registers[new_register as usize] = Register::Local;
                self.local_registers.push((local, new_register));
                new_register
            }
        };

        Ok(local_register)
    }

    fn pop_register(&mut self) -> Result<u8, String> {
        let register = match self.register_stack.pop() {
            Some(register) => register,
            None => {
                panic!("pop_register: Empty register stack".to_string());
                // return Err("pop_register: Empty register stack".to_string());
            }
        };

        match &mut self.registers[register as usize] {
            r @ Register::Active => *r = Register::Inactive,
            Register::Local => {}
            _ => unreachable!(),
        }

        Ok(register)
    }

    fn truncate_register_stack(&mut self, stack_count: usize) -> Result<(), String> {
        while self.register_stack.len() > stack_count {
            self.pop_register()?;
        }

        Ok(())
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

    fn compile_frame(&mut self, expressions: &[AstNode]) -> Result<(), String> {
        self.frame_stack.push(Frame::new());

        self.compile_expressions(expressions)?;

        let result_register = self.frame_mut().pop_register()?;
        self.push(&[Op::Return.into(), result_register]);

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
                self.push(&[Move.into(), register, result_register]);
            }
        } else {
            let register = self.frame_mut().get_register()?;
            self.push(&[SetEmpty.into(), register]);
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
                let target = self.frame_mut().get_register()?;
                self.push(&[SetEmpty.into(), target]);
            }
            Node::Id(index) => {
                self.frame_mut().get_local_register(*index)?;
            }
            Node::BoolTrue => {
                let target = self.frame_mut().get_register()?;
                self.push(&[SetTrue.into(), target]);
            }
            Node::BoolFalse => {
                let target = self.frame_mut().get_register()?;
                self.push(&[SetFalse.into(), target]);
            }
            Node::Number(constant) => {
                let target = self.frame_mut().get_register()?;
                let constant = *constant;
                if constant <= BYTE_MAX {
                    self.push(&[LoadNumber.into(), target, constant as u8]);
                } else {
                    self.push(&[LoadNumberLong.into(), target]);
                    self.push(&constant.to_le_bytes());
                }
            }
            Node::Str(constant) => {
                let target = self.frame_mut().get_register()?;
                let constant = *constant;
                if constant <= BYTE_MAX {
                    self.push(&[LoadString.into(), target, constant as u8]);
                } else {
                    self.push(&[LoadStringLong.into(), target]);
                    self.push(&constant.to_le_bytes());
                }
            }
            Node::MainBlock { body, .. } => {
                self.compile_frame(body)?;
            }
            Node::Block(expressions) => {
                self.compile_expressions(expressions)?;
            }
            Node::Call { function, args } => {
                let _function = function;
                let _args = args;
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
                    self.push(&[Move.into(), target, source]);
                }
            }
            Node::Op { op, lhs, rhs } => {
                let op = match op {
                    AstOp::Add => Add,
                    AstOp::Multiply => Multiply,
                    AstOp::Less => Less,
                    AstOp::Greater => Greater,
                    AstOp::And | AstOp::Or => {
                        self.compile_node(&lhs)?;
                        let lhs_register = self.frame_mut().pop_register()?;
                        let jump_op = if matches!(op, AstOp::And) {
                            JumpFalse
                        } else {
                            JumpTrue
                        };

                        self.push(&[jump_op.into(), lhs_register]);
                        self.compile_node_with_jump_offset(&rhs)?;

                        return Ok(());
                    }
                    _ => unimplemented!("missing AstOp"),
                };

                self.compile_node(&lhs)?;
                self.compile_node(&rhs)?;

                let frame = self.frame_mut();
                let rhs_register = frame.pop_register()?;
                let lhs_register = frame.pop_register()?;
                let target = frame.get_register()?;
                self.push(&[op.into(), target, lhs_register, rhs_register]);
            }
            Node::If(AstIf {
                condition,
                then_node,
                else_if_condition,
                else_if_node,
                else_node,
            }) => {
                self.compile_node(&condition)?;
                let condition_register = self.frame_mut().pop_register()?;

                self.push(&[JumpFalse.into(), condition_register]);
                let if_jump_ip = self.push_offset_placeholder();

                let stack_count = self.frame().register_stack.len();
                self.compile_node(&then_node)?;
                self.frame_mut().truncate_register_stack(stack_count)?;

                let then_jump_ip = {
                    if else_if_node.is_some() || else_node.is_some() {
                        self.push(&[Jump.into()]);
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
                    self.push(&[JumpFalse.into(), condition_register]);

                    let stack_count = self.frame().register_stack.len();
                    self.compile_node_with_jump_offset(&else_if_node)?;
                    self.frame_mut().truncate_register_stack(stack_count)?;

                    if else_node.is_some() {
                        self.push(&[Jump.into()]);
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
            }
            unexpected => unimplemented!("compile_node: unsupported node: {}", unexpected),
        }

        Ok(())
    }

    fn compile_node_with_jump_offset(&mut self, node: &AstNode) -> Result<(), String> {
        let offset_ip = self.push_offset_placeholder();
        self.compile_node(&node)?;
        self.update_offset_placeholder(offset_ip);
        Ok(())
    }

    fn push_offset_placeholder(&mut self) -> usize {
        let offset_ip = self.bytes.len();
        self.push(&[0, 0]);
        offset_ip
    }

    fn update_offset_placeholder(&mut self, offset_ip: usize) {
        let offset = self.bytes.len() - offset_ip - 2;
        let offset_bytes = (offset as u16).to_le_bytes();
        self.bytes[offset_ip] = offset_bytes[0];
        self.bytes[offset_ip + 1] = offset_bytes[1];
    }

    fn push(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }
}
