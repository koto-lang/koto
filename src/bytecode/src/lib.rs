//! # koto_bytecode
//!
//! Contains Koto's compiler and its bytecode operations

mod chunk;
mod compiler;
mod instruction_reader;
mod loader;
mod op;

pub use {
    chunk::{chunk_to_string, chunk_to_string_annotated, Chunk, DebugInfo},
    compiler::{Compiler, CompilerError, CompilerSettings},
    instruction_reader::{FunctionFlags, Instruction, InstructionReader, TypeId},
    loader::{Loader, LoaderError},
    op::Op,
};
