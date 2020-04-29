use crate::{Bytecode, Op};

use koto_parser::{
    AssignTarget, AstFor, AstIf, AstNode, AstOp, AstWhile, ConstantIndex, Lookup, LookupNode,
    LookupOrId, Node, Scope,
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
                    return Err("declare_local_register: Locals overflowed".to_string());
                }

                new_local_register
            }
        } as u8;

        Ok(local_register)
    }

    fn push_local_register(&mut self, local: ConstantIndex) -> Result<u8, String> {
        let local_register = self.get_local_register(local)?;
        self.register_stack.push(local_register);
        Ok(local_register)
    }

    fn pop_register(&mut self) -> Result<u8, String> {
        let register = match self.register_stack.pop() {
            Some(register) => register,
            None => {
                panic!();
                // return Err("pop_register: Empty register stack".to_string());
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
        assert!(self.frame_stack.is_empty());
        self.bytes.clear();
        self.compile_node(0, ast)?;
        Ok(&self.bytes)
    }

    fn compile_node(&mut self, result_register: u8, node: &AstNode) -> Result<(), String> {
        use Op::*;

        match &node.node {
            Node::Empty => {
                self.push_op(SetEmpty, &[result_register]);
            }
            Node::Id(index) => {
                self.compile_load_id(result_register, *index)?;
            }
            Node::Lookup(lookup) => self.compile_lookup(Some(result_register), lookup, None)?,
            Node::Copy(lookup_or_id) => {
                let source_register = self.push_register()?;
                self.compile_lookup_or_id(source_register, lookup_or_id)?;
                self.push_op(DeepCopy, &[result_register, source_register]);
                self.pop_register()?;
            }
            Node::BoolTrue => {
                self.push_op(SetTrue, &[result_register]);
            }
            Node::BoolFalse => {
                self.push_op(SetFalse, &[result_register]);
            }
            Node::Number(constant) => {
                let constant = *constant;
                if constant <= u8::MAX as u32 {
                    self.push_op(LoadNumber, &[result_register, constant as u8]);
                } else {
                    self.push_op(LoadNumberLong, &[result_register]);
                    self.push_bytes(&constant.to_le_bytes());
                }
            }
            Node::Str(constant) => {
                self.load_string(result_register, *constant);
            }
            Node::Vec4(elements) => {
                self.compile_make_vec4(result_register, &elements)?;
            }
            Node::List(elements) => {
                self.compile_make_list(result_register, &elements)?;
            }
            Node::Map(entries) => {
                let size_hint = entries.len();
                if size_hint <= u8::MAX as usize {
                    self.push_op(MakeMap, &[result_register, size_hint as u8]);
                } else {
                    self.push_op(MakeMapLong, &[result_register]);
                    self.push_bytes(&size_hint.to_le_bytes());
                }

                for (key, value_node) in entries.iter() {
                    let key_register = self.push_register()?;
                    self.load_string(key_register, *key);

                    let value_register = self.push_register()?;
                    self.compile_node(value_register, value_node)?;

                    self.push_op(MapInsert, &[result_register, key_register, value_register]);

                    self.pop_register()?;
                    self.pop_register()?;
                }
            }
            Node::Range {
                start,
                end,
                inclusive,
            } => {
                let start_register = self.push_register()?;
                self.compile_node(start_register, start)?;

                let end_register = self.push_register()?;
                self.compile_node(end_register, end)?;

                let op = if *inclusive { RangeInclusive } else { Range };
                self.push_op(op, &[result_register, start_register, end_register]);

                self.pop_register()?;
                self.pop_register()?;
            }
            Node::RangeFrom { start } => {
                let start_register = self.push_register()?;

                self.compile_node(start_register, start)?;
                self.push_op(RangeFrom, &[result_register, start_register]);

                self.pop_register()?;
            }
            Node::RangeTo { end, inclusive } => {
                let end_register = self.push_register()?;
                self.compile_node(end_register, end)?;

                let op = if *inclusive {
                    RangeToInclusive
                } else {
                    RangeTo
                };
                self.push_op(op, &[result_register, end_register]);

                self.pop_register()?;
            }
            Node::RangeFull => {
                self.push_op(RangeFull, &[result_register]);
            }
            Node::MainBlock { body, local_count } => {
                self.compile_frame(*local_count as u8, body, &[], &[])?;
            }
            Node::Block(expressions) => {
                self.compile_block(result_register, expressions)?;
            }
            Node::Expressions(expressions) => {
                // For now, capture the results of multiple expressions in a list.
                // Later, find situations where the list capture can be avoided.
                self.compile_make_list(result_register, &expressions)?;
            }
            Node::CopyExpression(expression) => {
                let source_register = self.push_register()?;
                self.compile_node(source_register, expression)?;

                self.push_op(DeepCopy, &[result_register, source_register]);

                self.pop_register()?;
            }
            Node::Negate(expression) => {
                let source_register = self.push_register()?;
                self.compile_node(source_register, expression)?;

                self.push_op(Negate, &[result_register, source_register]);

                self.pop_register()?;
            }
            Node::Function(f) => {
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
                        &[result_register, arg_count - 1, capture_count],
                    );
                } else {
                    self.push_op(Function, &[result_register, arg_count, capture_count]);
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
                    let capture_register = self.push_register()?;
                    self.compile_load_id(capture_register, *capture)?;

                    self.push_op(Capture, &[result_register, i as u8, capture_register]);

                    self.pop_register()?;
                }
            }
            Node::Call { function, args } => {
                match function {
                    LookupOrId::Id(id) => {
                        let function_register = self.push_register()?;
                        self.compile_load_id(function_register, *id)?;
                        self.compile_call(result_register, function_register, args, None)?;
                        self.pop_register()?;
                    }
                    LookupOrId::Lookup(function_lookup) => {
                        // TODO find a way to avoid the lookup cloning here
                        let mut call_lookup = function_lookup.clone();
                        call_lookup.0.push(LookupNode::Call(args.clone()));
                        self.compile_lookup(Some(result_register), &call_lookup, None)?
                    }
                };
            }
            Node::Assign { target, expression } => {
                self.compile_assign(Some(result_register), target, expression)?;
            }
            Node::MultiAssign {
                targets,
                expressions,
            } => {
                self.compile_multi_assign(result_register, targets, expressions)?;
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
                        self.compile_node(result_register, &lhs)?;

                        let jump_op = if matches!(op, AstOp::And) {
                            JumpFalse
                        } else {
                            JumpTrue
                        };
                        self.push_op(jump_op, &[result_register]);

                        // If the lhs causes a jump then that's the result,
                        // otherwise the rhs is the result
                        self.compile_node_with_jump_offset(result_register, &rhs)?;

                        return Ok(());
                    }
                };

                let lhs_register = self.push_register()?;
                let rhs_register = self.push_register()?;
                self.compile_node(lhs_register, &lhs)?;
                self.compile_node(rhs_register, &rhs)?;

                self.push_op(op, &[result_register, lhs_register, rhs_register]);

                self.pop_register()?; // rhs
                self.pop_register()?; // lhs
            }
            Node::If(ast_if) => self.compile_if(result_register, ast_if)?,
            Node::For(ast_for) => self.compile_for(result_register, ast_for, None)?,
            Node::While(ast_while) => self.compile_while(result_register, ast_while, None)?,
            Node::Break => {
                self.push_op(Jump, &[]);
                self.push_loop_jump_placeholder()?;
            }
            Node::Continue => {
                self.push_jump_back_op(JumpBack, &[], self.current_loop()?.start_ip);
            }
            Node::Return => {
                self.push_op(SetEmpty, &[result_register]);
                self.push_op(Return, &[result_register]);
            }
            Node::ReturnExpression(expression) => {
                self.compile_node(result_register, expression)?;
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

        let result_register = self.push_register()?;
        self.compile_block(result_register, expressions)?;
        self.push_op(Op::Return, &[result_register]);

        self.frame_stack.pop();

        Ok(())
    }

    fn compile_block(
        &mut self,
        result_register: u8,
        expressions: &[AstNode],
    ) -> Result<(), String> {
        use Op::*;

        if expressions.is_empty() {
            self.push_op(SetEmpty, &[result_register]);
        } else {
            for expression in expressions.iter() {
                self.compile_node(result_register, expression)?;
            }
        }

        Ok(())
    }

    fn local_register_for_assign_target(
        &mut self,
        target: &AssignTarget,
    ) -> Result<Option<u8>, String> {
        let result = match target {
            AssignTarget::Id { id_index, scope } => match *scope {
                Scope::Local => {
                    if self.frame().capture_slot(*id_index).is_some() {
                        None
                    } else {
                        Some(self.frame_mut().get_local_register(*id_index)?)
                    }
                }
                Scope::Global => None,
            },
            _ => None,
        };

        Ok(result)
    }

    fn compile_assign(
        &mut self,
        result_register: Option<u8>,
        target: &AssignTarget,
        expression: &AstNode,
    ) -> Result<(), String> {
        use Op::*;

        let local_assign_register = self.local_register_for_assign_target(target)?;
        let assign_register = match local_assign_register {
            Some(local) => local,
            None => self.push_register()?,
        };
        self.compile_node(assign_register, expression)?;

        match target {
            AssignTarget::Id { id_index, scope } => {
                match scope {
                    Scope::Local => {
                        if let Some(capture) = self.frame().capture_slot(*id_index) {
                            self.push_op(SetCapture, &[capture, assign_register]);
                        }
                    }
                    Scope::Global => {
                        if *id_index <= u8::MAX as u32 {
                            self.push_op(SetGlobal, &[*id_index as u8, assign_register]);
                        } else {
                            self.push_op(SetGlobalLong, &id_index.to_le_bytes());
                            self.push_bytes(&[assign_register]);
                        }
                    }
                }

                if let Some(result_register) = result_register {
                    if result_register != assign_register {
                        self.push_op(Copy, &[result_register, assign_register]);
                    }
                }
            }

            AssignTarget::Lookup(lookup) => {
                self.compile_lookup(result_register, lookup, Some(assign_register))?;
            }
        };

        if local_assign_register.is_none() {
            self.pop_register()?;
        }

        Ok(())
    }

    fn compile_multi_assign(
        &mut self,
        _result_register: u8,
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
                let rhs_register = self.push_register()?;
                self.compile_node(rhs_register, expression)?;

                for (i, target) in targets.iter().enumerate() {
                    match target {
                        AssignTarget::Id { id_index, .. } => {
                            if let Some(capture_slot) = self.frame().capture_slot(*id_index) {
                                let capture_register = self.push_register()?;

                                self.push_op(
                                    ExpressionIndex,
                                    &[capture_register, rhs_register, i as u8],
                                );
                                self.push_op(SetCapture, &[capture_slot, capture_register]);

                                self.pop_register()?;
                            } else {
                                let local_register =
                                    self.frame_mut().push_local_register(*id_index)?;

                                self.push_op(
                                    ExpressionIndex,
                                    &[local_register, rhs_register, i as u8],
                                );

                                self.pop_register()?;
                            }
                        }
                        AssignTarget::Lookup(lookup) => {
                            let register = self.push_register()?;

                            self.push_op(ExpressionIndex, &[register, rhs_register, i as u8]);
                            self.compile_lookup(None, lookup, Some(register))?;

                            self.pop_register()?;
                        }
                    };
                }

                self.pop_register()?; // rhs_register
            }
            _ => {
                let mut i = 0;

                for target in targets.iter() {
                    match expressions.get(i) {
                        Some(expression) => {
                            self.compile_assign(None, target, expression)?;
                        }
                        None => match target {
                            AssignTarget::Id { id_index, .. } => {
                                if let Some(capture) = self.frame().capture_slot(*id_index) {
                                    let register = self.push_register()?;
                                    self.push_op(SetEmpty, &[register]);
                                    self.push_op(SetCapture, &[capture, register]);
                                    self.pop_register()?;
                                } else {
                                    let local_register =
                                        self.frame_mut().get_local_register(*id_index)?;
                                    self.push_op(SetEmpty, &[local_register]);
                                }
                            }
                            AssignTarget::Lookup(lookup) => {
                                let register = self.push_register()?;
                                self.push_op(SetEmpty, &[register]);
                                self.compile_lookup(None, lookup, Some(register))?;
                                self.pop_register()?;
                            }
                        },
                    }

                    i += 1;
                }

                while i < expressions.len() {
                    let temp_register = self.push_register()?;
                    self.compile_node(temp_register, &expressions[i])?;
                    i += 1;
                }
            }
        }

        Ok(())
    }

    fn compile_load_id(&mut self, result_register: u8, id: ConstantIndex) -> Result<(), String> {
        use Op::*;

        if self.frame().is_local(id) {
            // local
            let local_register = self.frame_mut().get_local_register(id)?;
            if local_register != result_register {
                self.push_op(Copy, &[result_register, local_register]);
            }
        } else if let Some(capture_slot) = self.frame().capture_slot(id) {
            // capture
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

    fn compile_make_vec4(
        &mut self,
        result_register: u8,
        elements: &[AstNode],
    ) -> Result<(), String> {
        use Op::*;

        if elements.len() < 1 || elements.len() > 4 {
            return Err(format!(
                "compile_make_vec4: unexpected number of elements: {}",
                elements.len()
            ));
        }

        let stack_count = self.frame().register_stack.len();

        for element_node in elements.iter() {
            let element_register = self.push_register()?;
            self.compile_node(element_register, element_node)?;
            // if self.frame().peek_register()? < self.frame().temporary_base {
            //     let source = self.pop_register()?;
            //     let target = self.push_register()?;
            //     self.push_op(Copy, &[target, source]);
            // }
        }

        let first_element_register = self.frame().peek_register_n(elements.len() - 1)?;
        self.push_op(
            MakeVec4,
            &[
                result_register,
                elements.len() as u8,
                first_element_register,
            ],
        );

        self.frame_mut().truncate_register_stack(stack_count)?;
        Ok(())
    }

    fn compile_make_list(
        &mut self,
        result_register: u8,
        elements: &[AstNode],
    ) -> Result<(), String> {
        use Op::*;

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
            match &element_node.node {
                Node::For(for_loop) => {
                    self.compile_for(element_register, &for_loop, Some(result_register))?;
                }
                Node::While(while_loop) => {
                    self.compile_while(element_register, &while_loop, Some(result_register))?;
                }
                _ => {
                    self.compile_node(element_register, element_node)?;
                    self.push_op(ListPush, &[result_register, element_register]);
                }
            }
        }
        self.pop_register()?;

        Ok(())
    }

    fn compile_lookup_or_id(
        &mut self,
        result_register: u8,
        lookup_or_id: &LookupOrId,
    ) -> Result<(), String> {
        match lookup_or_id {
            LookupOrId::Id(id) => {
                self.compile_load_id(result_register, *id)?;
            }
            LookupOrId::Lookup(lookup) => {
                self.compile_lookup(Some(result_register), lookup, None)?;
            }
        }
        Ok(())
    }

    fn compile_lookup(
        &mut self,
        result_register: Option<u8>,
        lookup: &Lookup,
        set_value: Option<u8>,
    ) -> Result<(), String> {
        use Op::*;

        let lookup_len = lookup.0.len();
        if lookup_len < 2 {
            return Err(format!(
                "compile_lookup: lookup requires at least 2 elements, found {}",
                lookup_len
            ));
        }

        let stack_count = self.frame().register_stack.len();

        for (i, lookup_node) in lookup.0.iter().enumerate() {
            // Push a register for each lookup node.
            // This produces a lookup chain, allowing lookup operations to access parent containers
            let node_register = self.push_register()?;

            match lookup_node {
                LookupNode::Id(id) => {
                    if i == 0 {
                        // Root
                        self.compile_load_id(node_register, *id)?;
                    } else {
                        // Map key

                        // The map key can be stored temporarily in the node register
                        let key_register = node_register;
                        self.load_string(key_register, *id);
                        let map_register = self.frame_mut().peek_register_n(1)?;

                        if set_value.is_some() && i == lookup_len - 1 {
                            self.push_op(
                                MapInsert,
                                &[map_register, node_register, set_value.unwrap()],
                            );
                        } else {
                            self.push_op(MapAccess, &[node_register, map_register, key_register]);
                        }
                    }
                }
                LookupNode::Index(index_node) => {
                    let index_register = node_register;
                    self.compile_node(index_register, &index_node.0)?;
                    let list_register = self.frame_mut().peek_register_n(1)?;
                    if set_value.is_some() && i == lookup_len - 1 {
                        self.push_op(
                            ListUpdate,
                            &[list_register, index_register, set_value.unwrap()],
                        );
                    } else {
                        self.push_op(ListIndex, &[node_register, list_register, index_register]);
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

                    let function_register = self.frame_mut().peek_register_n(1)?;
                    self.compile_call(node_register, function_register, &args, parent_register)?;
                }
            }
        }

        if let Some(result_register) = result_register {
            match set_value {
                Some(value_register) => {
                    if value_register != result_register {
                        self.push_op(Copy, &[result_register, value_register]);
                    }
                }
                None => {
                    let lookup_result_register = self.frame_mut().peek_register()?;
                    if lookup_result_register != result_register {
                        self.push_op(Copy, &[result_register, lookup_result_register]);
                    }
                }
            }
        }

        self.frame_mut().truncate_register_stack(stack_count)?;

        Ok(())
    }

    fn compile_call(
        &mut self,
        result_register: u8,
        function_register: u8,
        args: &[AstNode],
        parent: Option<u8>,
    ) -> Result<(), String> {
        use Op::*;

        let stack_count = self.frame().register_stack.len();

        let frame_base = if args.is_empty() {
            self.push_register()?
        } else {
            self.frame().next_temporary_register()
        };

        for arg in args.iter() {
            let arg_register = self.push_register()?;
            self.compile_node(arg_register, &arg)?;
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
        if result_register != frame_base {
            self.push_op(Copy, &[result_register, frame_base]);
        }

        self.frame_mut().truncate_register_stack(stack_count)?;

        Ok(())
    }

    fn compile_if(&mut self, result_register: u8, ast_if: &AstIf) -> Result<(), String> {
        use Op::*;

        let AstIf {
            condition,
            then_node,
            else_if_condition,
            else_if_node,
            else_node,
        } = ast_if;

        let condition_register = self.push_register()?;
        self.compile_node(condition_register, &condition)?;
        self.push_op(JumpFalse, &[condition_register]);
        let if_jump_ip = self.push_offset_placeholder();
        self.pop_register()?;

        // let stack_count = self.frame().register_stack.len();
        self.compile_node(result_register, &then_node)?;

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

            let condition_register = self.push_register()?;
            self.compile_node(condition_register, &condition)?;

            self.push_op(JumpFalse, &[condition_register]);
            let then_jump_ip = self.push_offset_placeholder();

            self.pop_register()?; // condition register

            // re-use registers from if block - TODO, still necessary with result registers?
            // self.frame_mut().truncate_register_stack(stack_count)?;
            self.compile_node(result_register, &else_if_node)?;

            let else_if_jump_ip = if else_node.is_some() {
                self.push_op(Jump, &[]);
                Some(self.push_offset_placeholder())
            } else {
                None
            };
            self.update_offset_placeholder(then_jump_ip);

            else_if_jump_ip
        } else {
            None
        };

        if let Some(else_node) = else_node {
            // re-use registers from if/else if blocks - TODO, still necessary?
            // self.frame_mut().truncate_register_stack(stack_count)?;
            self.compile_node(result_register, else_node)?;
        } else {
            self.push_op(SetEmpty, &[result_register]);
        }

        if let Some(then_jump_ip) = then_jump_ip {
            self.update_offset_placeholder(then_jump_ip);
        }

        if let Some(else_if_jump_ip) = else_if_jump_ip {
            self.update_offset_placeholder(else_if_jump_ip);
        }

        Ok(())
    }

    fn compile_for(
        &mut self,
        result_register: u8,
        ast_for: &AstFor,
        expand_register: Option<u8>,
    ) -> Result<(), String> {
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

        if ranges.len() > 1 {
            if args.len() != ranges.len() {
                return Err(format!(
                    "compile_for: argument and range count mismatch: {} vs {}",
                    args.len(),
                    ranges.len()
                ));
            }
        }

        let stack_count = self.frame().register_stack.len();

        let iterator_register = match ranges.as_slice() {
            [] => {
                return Err(format!("compile_for: Missing range"));
            }
            [range_node] => {
                let iterator_register = self.push_register()?;
                let range_register = self.push_register()?;
                self.compile_node(range_register, range_node)?;

                self.push_op(MakeIterator, &[iterator_register, range_register]);
                self.pop_register()?; // range register

                iterator_register
            }
            _ => {
                let mut first_iterator_register = None;
                for range_node in ranges.iter() {
                    let iterator_register = self.push_register()?;
                    let range_register = self.push_register()?;
                    self.compile_node(range_register, range_node)?;

                    self.push_op(MakeIterator, &[iterator_register, range_register]);
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
            let temp_register = self.push_register()?;

            self.push_op(IteratorNext, &[temp_register, iterator_register]);
            self.push_loop_jump_placeholder()?;

            for (i, arg) in args.iter().enumerate() {
                let arg_register = self.frame_mut().push_local_register(*arg)?;
                self.push_op(ExpressionIndex, &[arg_register, temp_register, i as u8]);
            }

            self.pop_register()?; // temp_register
        } else {
            for (i, arg) in args.iter().enumerate() {
                let arg_register = self.frame_mut().get_local_register(*arg)?;
                self.push_op(IteratorNext, &[arg_register, iterator_register + i as u8]);
                self.push_loop_jump_placeholder()?;
            }
        }

        if let Some(condition) = condition {
            let condition_register = self.push_register()?;
            self.compile_node(condition_register, condition)?;
            self.push_jump_back_op(JumpBackFalse, &[condition_register], loop_start_ip);
            self.pop_register()?;
        }

        self.compile_node(result_register, body)?;

        if let Some(active_list) = expand_register {
            self.push_op(ListPush, &[active_list, result_register]);
        }

        self.push_jump_back_op(JumpBack, &[], loop_start_ip);

        match self.frame_mut().loop_stack.pop() {
            Some(loop_info) => {
                for placeholder in loop_info.jump_placeholders.iter() {
                    self.update_offset_placeholder(*placeholder);
                }
            }
            None => return Err("Empty loop info stack".to_string()),
        }

        self.frame_mut().truncate_register_stack(stack_count)?;

        Ok(())
    }

    fn compile_while(
        &mut self,
        result_register: u8,
        ast_while: &AstWhile,
        expand_register: Option<u8>,
    ) -> Result<(), String> {
        use Op::*;

        let AstWhile {
            condition,
            body,
            negate_condition,
        } = ast_while;

        let loop_start_ip = self.bytes.len();
        self.frame_mut().loop_stack.push(Loop::new(loop_start_ip));

        let condition_register = self.push_register()?;
        self.compile_node(condition_register, &condition)?;
        let op = if *negate_condition {
            JumpTrue
        } else {
            JumpFalse
        };
        self.push_op(op, &[condition_register]);
        self.push_loop_jump_placeholder()?;
        self.pop_register()?; // condition register

        self.compile_node(result_register, &body)?;

        if let Some(active_list) = expand_register {
            self.push_op(ListPush, &[active_list, result_register]);
        }

        self.push_jump_back_op(JumpBack, &[], loop_start_ip);

        match self.frame_mut().loop_stack.pop() {
            Some(loop_info) => {
                for placeholder in loop_info.jump_placeholders.iter() {
                    self.update_offset_placeholder(*placeholder);
                }
            }
            None => return Err("Empty loop info stack".to_string()),
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
        result_register: u8,
        node: &AstNode,
    ) -> Result<(), String> {
        let offset_ip = self.push_offset_placeholder();
        self.compile_node(result_register, &node)?;
        self.update_offset_placeholder(offset_ip);
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

    fn push_register(&mut self) -> Result<u8, String> {
        self.frame_mut().push_register()
    }

    fn pop_register(&mut self) -> Result<u8, String> {
        self.frame_mut().pop_register()
    }
}
