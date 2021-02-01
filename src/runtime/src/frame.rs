use {koto_bytecode::Chunk, std::sync::Arc};

#[derive(Debug)]
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
}

impl Frame {
    pub fn new(chunk: Arc<Chunk>, register_base: usize) -> Self {
        Self {
            chunk,
            register_base,
            return_register_and_ip: None,
            catch_stack: vec![],
            catch_barrier: false,
        }
    }
}
