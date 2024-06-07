use std::collections::HashSet;

use koto_parser::{AstIndex, ConstantIndex, Span};
use thiserror::Error;

/// The different error types that can be thrown while compiling a [Frame]
#[derive(Error, Clone, Debug)]
pub enum FrameError {
    #[error("the loop stack is empty")]
    EmptyLoopInfoStack,
    #[error("empty register stack")]
    EmptyRegisterStack,
    #[error("local register overflow")]
    LocalRegisterOverflow,
    #[error("the frame has reached the maximum number of registers")]
    StackOverflow,
    #[error("unable to commit register {0}")]
    UnableToCommitRegister(u8),
    #[error("unable to peek register {0}")]
    UnableToPeekRegister(usize),
    #[error("unexpected temporary register {0}")]
    UnexpectedTemporaryRegister(u8),
    #[error("register {0} hasn't been reserved")]
    UnreservedRegister(u8),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AssignedOrReserved {
    Assigned(u8),
    Reserved(u8),
    Unassigned,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DeferredOp {
    pub bytes: Vec<u8>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub(crate) struct Loop {
    // The loop's result register,
    pub result_register: Option<u8>,
    // The ip of the start of the loop, used for continue statements
    pub start_ip: usize,
    // Placeholders for jumps to the end of the loop, updated when the loop compilation is complete
    pub jump_placeholders: Vec<usize>,
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

pub(crate) enum Arg {
    Local(ConstantIndex),
    Unpacked(ConstantIndex),
    Placeholder,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Frame {
    loop_stack: Vec<Loop>,
    register_stack: Vec<u8>,
    local_registers: Vec<LocalRegister>,
    exported_ids: HashSet<ConstantIndex>,
    temporary_base: u8,
    temporary_count: u8,
    // Used to decide if an additional return instruction is needed,
    // e.g. `f = |x| return x`
    //               ^ explicit return as final expression, implicit return not needed
    // This is a coarse check, e.g. we currently don't check if the last expression
    // returns in all branches, but it'll do for now as an optimization for simple cases.
    pub last_node_was_return: bool,
    // An optional output type hint that should cause type checks to be emitted when the frame is
    // exited.
    // If the frame is representing a generator, then yield expressions will be checked, otherwise
    // the type check will be applied to any return paths.
    pub output_type: Option<AstIndex>,
    // Used to decide if return types should be checked (output type hints only apply to yield
    // expressions in genertors).
    pub is_generator: bool,
}

impl Frame {
    pub fn new(
        local_count: u8,
        args: &[Arg],
        captures: &[ConstantIndex],
        output_type: Option<AstIndex>,
        is_generator: bool,
    ) -> Self {
        let temporary_base =
            // register 0 is always self
            1
            // Includes all named args (including unpacked args),
            // and any locally assigned values.
            + local_count
            // Captures get copied to local registers when the function is called.
            + captures.len() as u8
            // To get the first temporary register, we also need to include 'unnamed' args, which
            // are represented in the args list as Placeholders.
            + args
                .iter()
                .filter(|arg| matches!(arg, Arg::Placeholder))
                .count() as u8;

        // First, assign registers to the 'top-level' args, including placeholder registers
        let mut local_registers = Vec::with_capacity(1 + args.len() + captures.len());
        local_registers.push(LocalRegister::Allocated); // self
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
            output_type,
            is_generator,
            ..Default::default()
        }
    }

    pub fn push_register(&mut self) -> Result<u8, FrameError> {
        let new_register = self.temporary_base + self.temporary_count;
        self.temporary_count += 1;

        if new_register == u8::MAX {
            Err(FrameError::StackOverflow)
        } else {
            self.register_stack.push(new_register);
            Ok(new_register)
        }
    }

    pub fn get_local_assigned_register(&self, local_name: ConstantIndex) -> Option<u8> {
        self.local_registers
            .iter()
            .position(|local_register| {
                matches!(local_register,
                    LocalRegister::Assigned(assigned) if *assigned == local_name
                )
            })
            .map(|position| position as u8)
    }

    pub fn get_local_assigned_or_reserved_register(
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

    pub fn reserve_local_register(&mut self, local: ConstantIndex) -> Result<u8, FrameError> {
        match self.get_local_assigned_or_reserved_register(local) {
            AssignedOrReserved::Assigned(assigned) => Ok(assigned),
            AssignedOrReserved::Reserved(reserved) => Ok(reserved),
            AssignedOrReserved::Unassigned => {
                self.local_registers
                    .push(LocalRegister::Reserved(local, vec![]));

                let new_local_register = self.local_registers.len() - 1;

                if new_local_register < self.temporary_base as usize {
                    Ok(new_local_register as u8)
                } else {
                    Err(FrameError::LocalRegisterOverflow)
                }
            }
        }
    }

    pub fn add_to_exported_ids(&mut self, id: ConstantIndex) {
        self.exported_ids.insert(id);
    }

    pub fn defer_op_until_register_is_committed(
        &mut self,
        reserved_register: u8,
        bytes: Vec<u8>,
        span: Span,
    ) -> Result<(), FrameError> {
        match self.local_registers.get_mut(reserved_register as usize) {
            Some(LocalRegister::Reserved(_, deferred_ops)) => {
                deferred_ops.push(DeferredOp { bytes, span });
                Ok(())
            }
            _ => Err(FrameError::UnreservedRegister(reserved_register)),
        }
    }

    pub fn commit_local_register(
        &mut self,
        local_register: u8,
    ) -> Result<Vec<DeferredOp>, FrameError> {
        let local_register = local_register as usize;
        let (index, deferred_ops) = match self.local_registers.get(local_register) {
            Some(LocalRegister::Assigned(_)) => {
                return Ok(vec![]);
            }
            Some(LocalRegister::Reserved(index, deferred_ops)) => (*index, deferred_ops.to_vec()),
            _ => return Err(FrameError::UnreservedRegister(local_register as u8)),
        };

        self.local_registers[local_register] = LocalRegister::Assigned(index);
        Ok(deferred_ops)
    }

    pub fn assign_local_register(&mut self, local: ConstantIndex) -> Result<u8, FrameError> {
        match self.get_local_assigned_or_reserved_register(local) {
            AssignedOrReserved::Assigned(assigned) => Ok(assigned),
            AssignedOrReserved::Reserved(reserved) => {
                let deferred_ops = self.commit_local_register(reserved)?;
                if deferred_ops.is_empty() {
                    Ok(reserved)
                } else {
                    Err(FrameError::UnableToCommitRegister(reserved))
                }
            }
            AssignedOrReserved::Unassigned => {
                self.local_registers.push(LocalRegister::Assigned(local));
                let new_local_register = self.local_registers.len() - 1;
                if new_local_register < self.temporary_base as usize {
                    Ok(new_local_register as u8)
                } else {
                    Err(FrameError::LocalRegisterOverflow)
                }
            }
        }
    }

    pub fn pop_register(&mut self) -> Result<u8, FrameError> {
        let Some(register) = self.register_stack.pop() else {
            return Err(FrameError::EmptyRegisterStack);
        };

        if register >= self.temporary_base {
            if self.temporary_count == 0 {
                return Err(FrameError::UnexpectedTemporaryRegister(register));
            }

            self.temporary_count -= 1;
        }

        Ok(register)
    }

    pub fn peek_register(&self, n: usize) -> Result<u8, FrameError> {
        self.register_stack
            .get(self.register_stack.len() - n - 1)
            .cloned()
            .ok_or(FrameError::UnableToPeekRegister(n))
    }

    pub fn register_stack_size(&self) -> usize {
        self.register_stack.len()
    }

    pub fn truncate_register_stack(&mut self, stack_count: usize) -> Result<(), FrameError> {
        while self.register_stack.len() > stack_count {
            self.pop_register()?;
        }

        Ok(())
    }

    pub fn next_temporary_register(&self) -> u8 {
        self.temporary_count + self.temporary_base
    }

    pub fn available_registers_count(&self) -> u8 {
        u8::MAX - self.next_temporary_register()
    }

    pub fn captures_for_nested_frame(
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

    pub fn push_loop(&mut self, loop_start_ip: usize, result_register: Option<u8>) {
        self.loop_stack.push(Loop {
            start_ip: loop_start_ip,
            result_register,
            jump_placeholders: Vec::new(),
        });
    }

    pub fn push_loop_jump_placeholder(&mut self, placeholder_ip: usize) -> Result<(), FrameError> {
        match self.loop_stack.last_mut() {
            Some(loop_info) => {
                loop_info.jump_placeholders.push(placeholder_ip);
                Ok(())
            }
            None => Err(FrameError::EmptyLoopInfoStack),
        }
    }

    pub fn current_loop(&self) -> Option<&Loop> {
        self.loop_stack.last()
    }

    pub fn pop_loop(&mut self) -> Result<Loop, FrameError> {
        self.loop_stack.pop().ok_or(FrameError::EmptyLoopInfoStack)
    }
}
