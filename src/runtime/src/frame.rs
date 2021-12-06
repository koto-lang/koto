use {koto_bytecode::Chunk, std::rc::Rc};

#[derive(Clone, Debug)]
pub(crate) struct Frame {
    // The chunk being interpreted in this frame
    pub chunk: Rc<Chunk>,
    // The index in the VM value stack of the first argument register,
    // or the first local register if there are no arguments.
    pub register_base: usize,
    // When returning to this frame, the ip that produced the most recently read instruction
    pub return_instruction_ip: usize,
    // When returning to this frame, the register for the return value and the ip to resume from.
    pub return_register_and_ip: Option<(u8, usize)>,
    // A stack of catch points for handling errors
    pub catch_stack: Vec<(u8, usize)>, // catch error register, catch ip
    // True if the frame should prevent execution from continuing after the frame is exited.
    // e.g. when an overloaded operator is being executed as a result of a regular instruction,
    //      or when an external function is calling back into the VM with a functor,
    pub execution_barrier: bool,
}

impl Frame {
    pub fn new(chunk: Rc<Chunk>, register_base: usize) -> Self {
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
