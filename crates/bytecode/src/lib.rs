//! Contains Koto's compiler and its bytecode operations

#![warn(missing_docs)]

mod chunk;
mod compiler;
mod frame;
mod instruction;
mod instruction_reader;
mod module_loader;
mod op;

pub use crate::{
    chunk::{Chunk, DebugInfo},
    compiler::{Compiler, CompilerError, CompilerSettings},
    instruction::{FunctionFlags, Instruction, StringFormatFlags},
    instruction_reader::InstructionReader,
    module_loader::{find_module, ModuleLoader, ModuleLoaderError},
    op::Op,
};
