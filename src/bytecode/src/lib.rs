//!
//! Contains Koto's compiler and its bytecode operations

#![warn(missing_docs)]

mod chunk;
mod compiler;
mod instruction_reader;
mod loader;
mod op;

pub use {
    chunk::{Chunk, DebugInfo},
    compiler::{Compiler, CompilerError, CompilerSettings},
    instruction_reader::{FunctionFlags, Instruction, InstructionReader, TypeId},
    loader::{Loader, LoaderError},
    op::Op,
};
