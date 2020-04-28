use crate::{Bytecode, Op};

use koto_parser::{
    AssignTarget, AstFor, AstIf, AstNode, AstOp, AstWhile, ConstantIndex, Lookup, LookupNode,
    LookupOrId, Node,
};
use std::convert::TryFrom;

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

#[derive(Clone, Debug, Default)]
struct Frame {
    loop_stack: Vec<Loop>,
    register_stack: Vec<u8>,
    local_registers: Vec<ConstantIndex>,
    captures: Vec<ConstantIndex>,
    temporary_base: u8,
    temporary_count: u8,
}

impl Frame {
    fn new(local_count: u8, args: &[ConstantIndex], captures: &[ConstantIndex]) -> Self {
        let mut local_registers = Vec::with_capacity(local_count as usize);
        local_registers.extend_from_slice(args);

        Self {
            register_stack: Vec::with_capacity(local_count as usize),
            local_registers,
            captures: captures.to_vec(),
            temporary_base: local_count,
            ..Default::default()
        }
    }

    fn push_register(&mut self) -> Result<u8, String> {
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

    fn capture_slot(&self, index: ConstantIndex) -> Option<u8> {
        self.captures
            .iter()
            .position(|constant_index| index == *constant_index)
            .map(|position| position as u8)
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

    fn peek_register(&self) -> Result<u8, String> {
        self.register_stack
            .last()
            .cloned()
            .ok_or_else(|| "peek_register: Empty register stack".to_string())
    }

    fn peek_register_n(&self, n: usize) -> Result<u8, String> {
        self.register_stack
            .get(self.register_stack.len() - n)
            .cloned()
            .ok_or_else(|| "peek_register_n: Non enough registers in the stack".to_string())
    }

    fn clone_registers(&self, count: usize) -> Result<Vec<u8>, String> {
        let first_register = self.register_stack.len() - count;
        self.register_stack
            .get(first_register..)
            .map(|registers| registers.iter().cloned().collect::<Vec<_>>())
            .ok_or_else(|| "clone_registers: Non enough registers in the stack".to_string())
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

    fn compile_node(&mut self, node: &AstNode) -> Result<(), String> {
        use Op::*;

        match &node.node {
            Node::Empty => {
                self.push_empty()?;
            }
            Node::Id(index) => {
                self.compile_load_id(*index)?;
            }
            Node::Lookup(lookup) => self.compile_lookup(lookup, None)?,
            Node::BoolTrue => {
                let target = self.frame_mut().push_register()?;
                self.push_op(SetTrue, &[target]);
            }
            Node::BoolFalse => {
                let target = self.frame_mut().push_register()?;
                self.push_op(SetFalse, &[target]);
            }
            Node::Number(constant) => {
                let target = self.frame_mut().push_register()?;
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
            Node::Vec4(elements) => {
                self.compile_make_vec4(&elements)?;
            }
            Node::List(elements) => {
                self.compile_make_list(&elements)?;
            }
            Node::Map(entries) => {
                let map_register = self.frame_mut().push_register()?;

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
                let target = self.frame_mut().push_register()?;
                self.push_op(op, &[target, start_register, end_register]);
            }
            Node::MainBlock { body, local_count } => {
                self.compile_frame(*local_count as u8, body, &[], &[])?;
            }
            Node::Block(expressions) => {
                self.compile_block(expressions)?;
            }
            Node::Expressions(expressions) => {
                // For now, capture the results of multiple expressions in a list.
                // Later, find situations where the list capture can be avoided.
                self.compile_make_list(&expressions)?;
            }
            Node::Negate(expression) => {
                self.compile_node(expression)?;
                let source = self.frame_mut().pop_register()?;
                let register = self.frame_mut().push_register()?;
                self.push_op(Negate, &[register, source]);
            }
            Node::Function(f) => {
                let function_register = self.frame_mut().push_register()?;
                let arg_count = match u8::try_from(f.args.len()) {
                    Ok(x) => x,
                    Err(_) => {
                        return Err(format!("Function has too many arguments: {}", f.args.len()));
                    }
                };

                let capture_count = f.captures.len() as u8;

                if f.is_instance_function {
                    self.push_op(
                        InstanceFunction,
                        &[function_register, arg_count - 1, capture_count],
                    );
                } else {
                    self.push_op(Function, &[function_register, arg_count, capture_count]);
                }

                let function_size_ip = self.push_offset_placeholder();

                let local_count = match u8::try_from(f.local_count) {
                    Ok(x) => x,
                    Err(_) => {
                        return Err(format!("Function has too many locals: {}", f.args.len()));
                    }
                };

                self.compile_frame(local_count, &f.body, &f.args, &f.captures)?;
                self.update_offset_placeholder(function_size_ip);

                for (i, capture) in f.captures.iter().enumerate() {
                    self.compile_load_id(*capture)?;
                    let capture_register = self.frame_mut().pop_register()?;
                    self.push_op(Capture, &[function_register, i as u8, capture_register]);
                }
            }
            Node::Call { function, args } => {
                match function {
                    LookupOrId::Id(id) => {
                        let function_register = self.compile_load_id(*id)?;
                        self.compile_call(function_register, args, None)?;
                    }
                    LookupOrId::Lookup(function_lookup) => {
                        // TODO find a way to avoid the lookup cloning here
                        let mut call_lookup = function_lookup.clone();
                        call_lookup.0.push(LookupNode::Call(args.clone()));
                        self.compile_lookup(&call_lookup, None)?
                    }
                };
            }
            Node::Assign { target, expression } => {
                self.compile_assign(target, expression)?;
            }
            Node::MultiAssign {
                targets,
                expressions,
            } => {
                self.compile_multi_assign(targets, expressions)?;
            }
            Node::Op { op, lhs, rhs } => {
                let op = match op {
                    AstOp::Add => Add,
                    AstOp::Subtract => Subtract,
                    AstOp::Multiply => Multiply,
                    AstOp::Divide => Divide,
                    AstOp::Modulo => Modulo,
                    AstOp::Less => Less,
                    AstOp::LessOrEqual => LessOrEqual,
                    AstOp::Greater => Greater,
                    AstOp::GreaterOrEqual => GreaterOrEqual,
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
                };

                self.compile_node(&lhs)?;
                self.compile_node(&rhs)?;

                let frame = self.frame_mut();
                let rhs_register = frame.pop_register()?;
                let lhs_register = frame.pop_register()?;
                let target = frame.push_register()?;
                self.push_op(op, &[target, lhs_register, rhs_register]);
            }
            Node::If(ast_if) => self.compile_if(ast_if)?,
            Node::For(ast_for) => self.compile_for(ast_for)?,
            Node::While(ast_while) => self.compile_while(ast_while)?,
            Node::Break => {
                self.push_op(Jump, &[]);
                self.push_loop_jump_placeholder()?;
            }
            Node::Continue => {
                self.push_jump_back_op(JumpBack, &[], self.current_loop()?.start_ip);
            }
            Node::Return => {
                let register = self.frame_mut().push_register()?;
                self.push_op(SetEmpty, &[register]);
                self.push_op(Return, &[register]);
            }
            Node::ReturnExpression(expression) => {
                self.compile_node(expression)?;
                let result_register = self.frame_mut().peek_register()?;
                self.push_op(Return, &[result_register]);
            }
            unexpected => unimplemented!("compile_node: unsupported node: {}", unexpected),
        }

        Ok(())
    }

    fn compile_frame(
        &mut self,
        local_count: u8,
        expressions: &[AstNode],
        args: &[ConstantIndex],
        captures: &[ConstantIndex],
    ) -> Result<(), String> {
        self.frame_stack
            .push(Frame::new(local_count, args, captures));

        self.compile_block(expressions)?;

        let result_register = self.frame_mut().pop_register()?;
        self.push_bytes(&[Op::Return.into(), result_register]);

        self.frame_stack.pop();

        Ok(())
    }

    fn compile_block(&mut self, expressions: &[AstNode]) -> Result<(), String> {
        use Op::*;

        if expressions.is_empty() {
            let register = self.frame_mut().push_register()?;
            self.push_bytes(&[SetEmpty.into(), register]);
        } else {
            for (i, expression) in expressions.iter().enumerate() {
                self.compile_node(expression)?;
                // Keep the last expression's result on the stack
                if i < expressions.len() - 1 {
                    self.frame_mut().pop_register()?;
                }
            }
        }

        Ok(())
    }

    fn compile_assign(
        &mut self,
        target: &AssignTarget,
        expression: &AstNode,
    ) -> Result<(), String> {
        use Op::*;

        self.compile_node(expression)?;

        match target {
            AssignTarget::Id { id_index, .. } => {
                if let Some(capture) = self.frame().capture_slot(*id_index) {
                    let source = self.frame_mut().peek_register()?;
                    self.push_op(SetCapture, &[capture, source]);
                } else {
                    let source = self.frame_mut().pop_register()?;
                    let register = self.frame_mut().get_local_register(*id_index)?;
                    if register != source {
                        self.push_op(Copy, &[register, source]);
                    }
                }
            }
            AssignTarget::Lookup(lookup) => {
                let source = self.frame_mut().peek_register()?;
                self.compile_lookup(lookup, Some(source))?;
            }
        };
        Ok(())
    }

    fn compile_multi_assign(
        &mut self,
        targets: &[AssignTarget],
        expressions: &[AstNode],
    ) -> Result<(), String> {
        use Op::*;

        assert!(targets.len() < u8::MAX as usize);
        assert!(expressions.len() < u8::MAX as usize);

        match expressions {
            [] => {
                return Err("compile_multi_assign: Missing expression".to_string());
            }
            [expression] => {
                self.compile_node(expression)?;

                let rhs_register = self.frame_mut().peek_register()?;

                for (i, target) in targets.iter().enumerate() {
                    match target {
                        AssignTarget::Id { id_index, .. } => {
                            if let Some(capture) = self.frame().capture_slot(*id_index) {
                                let register = self.frame_mut().push_register()?;
                                self.push_op(ExpressionIndex, &[register, rhs_register, i as u8]);
                                self.push_op(SetCapture, &[capture, register]);
                            } else {
                                let register = self.frame_mut().get_local_register(*id_index)?;
                                self.push_op(ExpressionIndex, &[register, rhs_register, i as u8]);
                            }
                        }
                        AssignTarget::Lookup(lookup) => {
                            let register = self.frame_mut().push_register()?;
                            self.push_op(ExpressionIndex, &[register, rhs_register, i as u8]);
                            self.compile_lookup(lookup, Some(register))?;
                            self.frame_mut().pop_register()?;
                        }
                    };
                }
            }
            _ => {
                for expression in expressions.iter() {
                    self.compile_node(expression)?;
                }

                let expression_registers = self.frame().clone_registers(expressions.len())?;

                for (i, target) in targets.iter().enumerate() {
                    match target {
                        AssignTarget::Id { id_index, .. } => {
                            if let Some(capture) = self.frame().capture_slot(*id_index) {
                                let register = self.frame_mut().push_register()?;
                                match expression_registers.get(i) {
                                    Some(expression_register) => {
                                        self.push_op(Copy, &[register, *expression_register]);
                                    }
                                    None => {
                                        self.push_op(SetEmpty, &[register]);
                                    }
                                }
                                self.push_op(SetCapture, &[capture, register]);
                                self.frame_mut().pop_register()?;
                            } else {
                                let register = self.frame_mut().get_local_register(*id_index)?;
                                match expression_registers.get(i) {
                                    Some(expression_register) => {
                                        self.push_op(Copy, &[register, *expression_register]);
                                    }
                                    None => {
                                        self.push_op(SetEmpty, &[register]);
                                    }
                                }
                                self.frame_mut().pop_register()?;
                            }
                        }
                        AssignTarget::Lookup(lookup) => match expression_registers.get(i) {
                            Some(expression_register) => {
                                self.compile_lookup(lookup, Some(*expression_register))?;
                            }
                            None => {
                                let register = self.frame_mut().push_register()?;
                                self.push_op(SetEmpty, &[register]);
                                self.compile_lookup(lookup, Some(register))?;
                                self.frame_mut().pop_register()?;
                            }
                        },
                    }
                }
            }
        }

        Ok(())
    }

    fn compile_load_id(&mut self, id: ConstantIndex) -> Result<u8, String> {
        use Op::*;

        let register = if self.frame().is_local(id) {
            // local
            self.frame_mut().get_local_register(id)?
        } else if let Some(capture_slot) = self.frame().capture_slot(id) {
            // capture
            let register = self.frame_mut().push_register()?;
            self.push_op(LoadCapture, &[register, capture_slot]);
            register
        } else {
            // global
            let register = self.frame_mut().push_register()?;
            if id <= u8::MAX as u32 {
                self.push_op(LoadGlobal, &[register, id as u8]);
            } else {
                self.push_op(LoadGlobalLong, &[register]);
                self.push_bytes(&id.to_le_bytes());
            }
            register
        };

        Ok(register)
    }

    fn compile_make_vec4(&mut self, elements: &[AstNode]) -> Result<(), String> {
        use Op::*;

        let vec4_register = self.frame_mut().push_register()?;
        let stack_count = self.frame().register_stack.len();

        for element in elements.iter() {
            self.compile_node(element)?;
            if self.frame().peek_register()? < self.frame().temporary_base {
                let source = self.frame_mut().pop_register()?;
                let target = self.frame_mut().push_register()?;
                self.push_op(Copy, &[target, source]);
            }
        }

        let first_element_register = self.frame().peek_register_n(elements.len())?;
        self.push_op(
            MakeVec4,
            &[vec4_register, elements.len() as u8, first_element_register],
        );

        self.frame_mut().truncate_register_stack(stack_count)?;
        Ok(())
    }

    fn compile_make_list(&mut self, elements: &[AstNode]) -> Result<(), String> {
        use Op::*;

        let list_register = self.frame_mut().push_register()?;

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

        let result_register = self.frame_mut().push_register()?;
        let stack_count = self.frame().register_stack.len();

        for (i, lookup_node) in lookup.0.iter().enumerate() {
            match lookup_node {
                LookupNode::Id(id) => {
                    if i == 0 {
                        self.compile_load_id(*id)?;
                    } else {
                        self.load_string(*id)?;
                        let key_register = self.frame_mut().pop_register()?;
                        let map_register = self.frame_mut().peek_register()?;

                        if set_value.is_some() && i == lookup_len - 1 {
                            self.push_op(
                                MapInsert,
                                &[map_register, key_register, set_value.unwrap()],
                            );
                        } else {
                            let result_register = self.frame_mut().push_register()?;
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
                        let result_register = self.frame_mut().push_register()?;
                        self.push_op(ListIndex, &[result_register, list_register, index_register]);
                    }
                }
                LookupNode::Call(args) => {
                    if set_value.is_some() && i == lookup_len - 1 {
                        return Err("Assigning to temporary value".to_string());
                    }

                    let parent_register = if i > 1 {
                        Some(self.frame_mut().peek_register_n(2)?)
                    } else {
                        None
                    };

                    let function_register = self.frame_mut().peek_register()?;
                    self.compile_call(function_register, &args, parent_register)?;
                }
            }
        }

        let lookup_result_register = self.frame_mut().pop_register()?;
        if lookup_result_register != result_register {
            self.push_op(Copy, &[result_register, lookup_result_register]);
        }

        self.frame_mut().truncate_register_stack(stack_count)?;

        Ok(())
    }

    fn compile_call(
        &mut self,
        function_register: u8,
        args: &[AstNode],
        parent: Option<u8>,
    ) -> Result<(), String> {
        use Op::*;

        let stack_count = self.frame().register_stack.len();

        let frame_base = if args.is_empty() {
            self.frame_mut().push_register()?
        } else {
            self.frame().next_temporary_register()
        };

        for arg in args.iter() {
            self.compile_node(&arg)?;

            // If the arg value is in a local register, then it needs to be copied to
            // an argument register
            let frame = self.frame_mut();
            if frame.peek_register()? < frame.temporary_base {
                let source = frame.pop_register()?;
                let target = frame.push_register()?;
                self.push_op(Copy, &[target, source]);
            }
        }

        match parent {
            Some(parent_register) => {
                self.push_op(
                    CallChild,
                    &[
                        function_register,
                        parent_register,
                        frame_base,
                        args.len() as u8,
                    ],
                );
            }
            None => {
                self.push_op(Call, &[function_register, frame_base, args.len() as u8]);
            }
        }

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

            // re-use registers from if block
            self.frame_mut().truncate_register_stack(stack_count)?;
            self.compile_node_with_jump_offset(&else_if_node)?;

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
            // re-use registers from if/else if blocks
            self.frame_mut().truncate_register_stack(stack_count)?;
            self.compile_node(else_node)?;
        } else {
            self.push_empty()?;
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

        if args.len() != ranges.len() {
            return Err(format!(
                "compile_for: argument and range count mismatch: {} vs {}",
                args.len(),
                ranges.len()
            ));
        }

        let iterator_register = match ranges.as_slice() {
            [] => {
                return Err(format!("compile_for: Missing range"));
            }
            [range] => {
                self.compile_node(range)?;
                let range_register = self.frame_mut().pop_register()?;
                let iterator_register = self.frame_mut().push_register()?;

                self.push_op(MakeIterator, &[iterator_register, range_register]);

                iterator_register
            }
            _ => {
                let mut first_iterator_register = None;
                for range in ranges.iter() {
                    self.compile_node(range)?;
                    let range_register = self.frame_mut().pop_register()?;
                    let iterator_register = self.frame_mut().push_register()?;

                    self.push_op(MakeIterator, &[iterator_register, range_register]);

                    if first_iterator_register.is_none() {
                        first_iterator_register = Some(iterator_register);
                    }
                }
                first_iterator_register.unwrap()
            }
        };

        let loop_start_ip = self.bytes.len();
        self.frame_mut().loop_stack.push(Loop::new(loop_start_ip));

        for (i, arg) in args.iter().enumerate() {
            let arg_register = self.frame_mut().get_local_register(*arg)?;
            self.push_op(IteratorNext, &[arg_register, iterator_register + i as u8]);
            self.push_loop_jump_placeholder()?;
        }

        if let Some(condition) = condition {
            self.compile_node(condition)?;
            let condition_register = self.frame_mut().pop_register()?;
            self.push_jump_back_op(JumpBackFalse, &[condition_register], loop_start_ip);
        }

        self.compile_node(body)?;

        self.push_jump_back_op(JumpBack, &[], loop_start_ip);

        match self.frame_mut().loop_stack.pop() {
            Some(loop_info) => {
                for placeholder in loop_info.jump_placeholders.iter() {
                    self.update_offset_placeholder(*placeholder);
                }
            }
            None => return Err("Empty loop info stack".to_string()),
        }

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

        let loop_start_ip = self.bytes.len();
        self.frame_mut().loop_stack.push(Loop::new(loop_start_ip));

        self.compile_node(&condition)?;
        let condition_register = self.frame_mut().pop_register()?;
        let op = if *negate_condition {
            JumpTrue
        } else {
            JumpFalse
        };
        self.push_op(op, &[condition_register]);
        self.push_loop_jump_placeholder()?;

        self.compile_node(&body)?;
        self.push_jump_back_op(JumpBack, &[], loop_start_ip);

        match self.frame_mut().loop_stack.pop() {
            Some(loop_info) => {
                for placeholder in loop_info.jump_placeholders.iter() {
                    self.update_offset_placeholder(*placeholder);
                }
            }
            None => return Err("Empty loop info stack".to_string()),
        }

        self.push_empty()?;

        Ok(())
    }

    fn load_string(&mut self, index: ConstantIndex) -> Result<u8, String> {
        use Op::*;

        let target = self.frame_mut().push_register()?;
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
        let target = self.frame_mut().push_register()?;
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

    fn current_loop(&self) -> Result<&Loop, String> {
        self.frame()
            .loop_stack
            .last()
            .ok_or_else(|| "Missing loop info".to_string())
    }

    fn push_loop_jump_placeholder(&mut self) -> Result<(), String> {
        let placeholder = self.push_offset_placeholder();
        match self.frame_mut().loop_stack.last_mut() {
            Some(loop_info) => {
                loop_info.jump_placeholders.push(placeholder);
                Ok(())
            }
            None => Err("Missing loop info".to_string()),
        }
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
