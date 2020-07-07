use {crate::{Value, ValueList}, koto_bytecode::Chunk, std::sync::Arc};

#[derive(Debug, Default)]
pub(crate) struct Frame {
    // The chunk being interpreted in this frame
    pub chunk: Arc<Chunk>,
    // The index in the VM value stack of the first argument register,
    // or the first local register if there are no arguments.
    pub register_base: usize,
    // When returning to this frame, the register for the return value and the ip to resume from.
    pub return_register_and_ip: Option<(u8, usize)>,
    // A stack of catch points for handling errors
    pub catch_stack: Vec<(u8, usize)>, // catch error register, catch ip
    // True if the frame should prevent errors from being caught further down the stack,
    // e.g. when an external function is calling back into the VM with a functor
    pub catch_barrier: bool,
    // The captures that are available in this frame
    captures: Option<ValueList>,
}

impl Frame {
    pub fn new(
        chunk: Arc<Chunk>,
        register_base: usize,
        captures: ValueList,
    ) -> Self {
        Self {
            chunk,
            register_base,
            captures: Some(captures),
            ..Default::default()
        }
    }

    pub fn get_capture(&self, capture: u8) -> Option<Value> {
        if let Some(captures) = &self.captures {
            captures.data().get(capture as usize).cloned()
        } else {
            None
        }
    }

    pub fn set_capture(&self, capture_index: u8, value: Value) -> bool {
        if let Some(captures) = &self.captures {
            if let Some(capture) = captures.data_mut().get_mut(capture_index as usize) {
                *capture = value;
                return true;
            }
        }
        false
    }
}
