use crate::{Bytecode, Op};

use koto_parser::{
    AssignTarget, AstFor, AstIf, AstNode, AstOp, AstWhile, ConstantIndex, Lookup, LookupNode,
    LookupOrId, Node,
};
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

    fn compile_node(&mut self, node: &AstNode) -> Result<(), String> {
        use Op::*;

        match &node.node {
            Node::Empty => {
                self.push_empty()?;
            }
            Node::Id(index) => self.compile_load_id(*index)?,
            Node::Lookup(lookup) => self.compile_lookup(lookup, None)?,
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
                self.load_string(*constant)?;
            }
            Node::List(elements) => {
                let list_register = self.frame_mut().get_register()?;

                // TODO take ranges into account when determining size hint
                let size_hint = elements.len();
                if size_hint <= u8::MAX as usize {
                    self.push_op(MakeList, &[list_register, size_hint as u8]);
                } else {
                    self.push_op(MakeListLong, &[list_register]);
                    self.push_bytes(&size_hint.to_le_bytes());
                }

                for element_node in elements.iter() {
                    self.compile_node(element_node)?;
                    let element = self.frame_mut().pop_register()?;
                    self.push_op(ListPush, &[list_register, element]);
                }
            }
            Node::Map(entries) => {
                let map_register = self.frame_mut().get_register()?;

                let size_hint = entries.len();
                if size_hint <= u8::MAX as usize {
                    self.push_op(MakeMap, &[map_register, size_hint as u8]);
                } else {
                    self.push_op(MakeMapLong, &[map_register]);
                    self.push_bytes(&size_hint.to_le_bytes());
                }

                for (key, value_node) in entries.iter() {
                    self.load_string(*key)?;
                    self.compile_node(value_node)?;
                    let value_register = self.frame_mut().pop_register()?;
                    let key_register = self.frame_mut().pop_register()?;
                    self.push_op(MapInsert, &[map_register, key_register, value_register]);
                }
            }
            Node::Range {
                start,
                end,
                inclusive,
            } => {
                self.compile_node(start)?;
                self.compile_node(end)?;
                let end_register = self.frame_mut().pop_register()?;
                let start_register = self.frame_mut().pop_register()?;

                let op = if *inclusive {
                    RangeInclusive
                } else {
                    RangeExclusive
                };
                let target = self.frame_mut().get_register()?;
                self.push_op(op, &[target, start_register, end_register]);
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

                self.compile_call(function_register, args)?;
            }
            Node::Assign { target, expression } => {
                self.compile_node(expression)?;
                match target {
                    AssignTarget::Id { id_index, .. } => {
                        let source = self.frame_mut().pop_register()?;
                        let register = self.frame_mut().get_local_register(*id_index)?;
                        if register != source {
                            self.push_op(Copy, &[register, source]);
                        }
                    }
                    AssignTarget::Lookup(lookup) => {
                        let source = *self.frame_mut().peek_register().unwrap();
                        self.compile_lookup(lookup, Some(source))?;
                    }
                };
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
            Node::For(ast_for) => self.compile_for(ast_for)?,
            Node::While(ast_while) => self.compile_while(ast_while)?,
            unexpected => unimplemented!("compile_node: unsupported node: {}", unexpected),
        }

        Ok(())
    }

    fn compile_load_id(&mut self, id: ConstantIndex) -> Result<(), String> {
        if self.frame().is_local(id) {
            self.frame_mut().get_local_register(id)?;
        } else {
            self.load_global(id)?;
        }
        Ok(())
    }

    fn compile_lookup(&mut self, lookup: &Lookup, set_value: Option<u8>) -> Result<(), String> {
        use Op::*;

        let lookup_len = lookup.0.len();
        if lookup_len < 2 {
            return Err(format!(
                "compile_lookup: lookup requires at least 2 elements, found {}",
                lookup_len
            ));
        }

        for (i, lookup_node) in lookup.0.iter().enumerate() {
            match lookup_node {
                LookupNode::Id(id) => {
                    if i == 0 {
                        self.compile_load_id(*id)?;
                    } else {
                        self.load_string(*id)?;
                        let key_register = self.frame_mut().pop_register()?;
                        let map_register = self.frame_mut().pop_register()?;

                        if set_value.is_some() && i == lookup_len - 1 {
                            self.push_op(
                                MapInsert,
                                &[map_register, key_register, set_value.unwrap()],
                            );
                        } else {
                            let result_register = self.frame_mut().get_register()?;
                            self.push_op(MapAccess, &[result_register, map_register, key_register]);
                        }
                    }
                }
                LookupNode::Index(index_node) => {
                    self.compile_node(&index_node.0)?;
                    let index_register = self.frame_mut().pop_register()?;
                    let list_register = self.frame_mut().pop_register()?;
                    if set_value.is_some() && i == lookup_len - 1 {
                        self.push_op(
                            ListUpdate,
                            &[list_register, index_register, set_value.unwrap()],
                        );
                    } else {
                        let result_register = self.frame_mut().get_register()?;
                        self.push_op(ListIndex, &[result_register, list_register, index_register]);
                    }
                }
                LookupNode::Call(args) => {
                    if set_value.is_some() && i == lookup_len - 1 {
                        return Err("Assigning to temporary value".to_string());
                    }

                    let function_register = *self.frame_mut().peek_register().unwrap();
                    self.compile_call(function_register, &args)?;
                }
            }
        }

        Ok(())
    }

    fn compile_call(&mut self, function_register: u8, args: &[AstNode]) -> Result<(), String> {
        use Op::*;

        let stack_count = self.frame().register_stack.len();

        let frame_base = if args.is_empty() {
            self.frame_mut().get_register()?
        } else {
            self.frame().next_temporary_register()
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

        self.push_op(Call, &[function_register, frame_base, args.len() as u8]);

        // The return value gets placed in the frame base register
        // TODO multiple return values
        self.frame_mut().truncate_register_stack(stack_count + 1)?;

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

    fn compile_for(&mut self, ast_for: &AstFor) -> Result<(), String> {
        use Op::*;

        let AstFor {
            args,
            ranges,
            condition,
            body,
        } = &ast_for;

        //   make iterator, iterator_register
        //   make local registers for for args
        // loop_start:
        //   iterator_next_or_jump iterator_register arg_register jump -> end
        //   if condition
        //     condition_body
        //     if body result false jump -> loop_start
        //   loop body
        //   jump -> loop_start
        // end:

        let iterator_register = match ranges.as_slice() {
            [range] => {
                self.compile_node(range)?;
                let range_register = self.frame_mut().pop_register()?;
                let iterator_register = self.frame_mut().get_register()?;
                self.push_op(MakeIterator, &[iterator_register, range_register]);
                iterator_register
            }
            [_ranges, ..] => {
                unimplemented!("TODO: multi-range for loop");
            }
            _ => {
                return Err(format!("compile_for: Missing range"));
            }
        };

        let arg_register = match args.as_slice() {
            &[arg] => self.frame_mut().get_local_register(arg)?,
            &[_args, ..] => {
                unimplemented!("TODO: multi-arg for loop");
            }
            _ => {
                return Err(format!("compile_for: Missing argument"));
            }
        };

        let loop_start_ip = self.bytes.len();

        self.push_op(IteratorNext, &[arg_register, iterator_register]);
        let jump_to_loop_end = self.push_offset_placeholder();

        if let Some(condition) = condition {
            self.compile_node(condition)?;
            let condition_register = self.frame_mut().pop_register()?;
            self.push_jump_back_op(JumpBackFalse, &[condition_register], loop_start_ip);
        }

        self.compile_node(body)?;

        self.push_jump_back_op(JumpBack, &[], loop_start_ip);
        self.update_offset_placeholder(jump_to_loop_end);

        self.push_empty()?;
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
        self.push_jump_back_op(JumpBack, &[], while_jump_ip);

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

    fn load_string(&mut self, index: ConstantIndex) -> Result<u8, String> {
        use Op::*;

        let target = self.frame_mut().get_register()?;
        if index <= u8::MAX as u32 {
            self.push_op(LoadString, &[target, index as u8]);
        } else {
            self.push_op(LoadStringLong, &[target]);
            self.push_bytes(&index.to_le_bytes());
        }

        Ok(target)
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

    fn push_jump_back_op(&mut self, op: Op, bytes: &[u8], target_ip: usize) {
        let offset = self.bytes.len() + 3 + bytes.len() - target_ip;
        self.push_op(op, bytes);
        self.push_bytes(&(offset as u16).to_le_bytes());
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

    fn frame(&self) -> &Frame {
        self.frame_stack.last().expect("Frame stack is empty")
    }

    fn frame_mut(&mut self) -> &mut Frame {
        self.frame_stack.last_mut().expect("Frame stack is empty")
    }
}
