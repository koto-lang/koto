use {crate::Ptr, koto_bytecode::Chunk};

#[derive(Clone, Debug)]
pub(crate) struct Frame {
    // The chunk being interpreted in this frame
    pub chunk: Ptr<Chunk>,
    // The index in the VM's value stack of the first frame register.
    // The frame's instance is always in register 0 (Null if not set).
    // Call arguments followed by local values are in registers starting from index 1.
    pub register_base: usize,
    // When returning to this frame, the ip that produced the most recently read instruction
    pub return_instruction_ip: usize,
    // When returning to this frame, the register for the return value and the ip to resume from.
    pub return_register_and_ip: Option<(u8, usize)>,
    // A stack of catch points for handling errors
    pub catch_stack: Vec<(u8, usize)>, // catch error register, catch ip
    // True if the frame should prevent execution from continuing after the frame is exited.
    // e.g.
    //   - a function is being called externally from the VM
    //   - an overloaded operator is being executed as a result of a regular instruction
    //   - an external function is calling back into the VM with a functor
    //   - a module is being imported
    pub execution_barrier: bool,
}

impl Frame {
    pub fn new(chunk: Ptr<Chunk>, register_base: usize) -> Self {
        Self {
            chunk,
            register_base,
            return_register_and_ip: None,
            return_instruction_ip: 0,
            catch_stack: vec![],
            execution_barrier: false,
        }
    }
}
