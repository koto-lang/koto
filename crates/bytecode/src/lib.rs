//! Contains Koto's compiler and its bytecode operations

#![warn(missing_docs)]

mod chunk;
mod compiler;
mod frame;
mod instruction;
mod instruction_reader;
mod loader;
mod op;

pub use crate::{
    chunk::{Chunk, DebugInfo},
    compiler::{Compiler, CompilerError, CompilerSettings},
    instruction::{FunctionFlags, Instruction, TypeId},
    instruction_reader::InstructionReader,
    loader::{Loader, LoaderError},
    op::Op,
};
