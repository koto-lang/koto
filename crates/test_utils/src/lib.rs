//! Testing utilities for Koto crates

#![warn(missing_docs)]

mod check_script_output;
mod doc_examples;
mod output_capture;
mod script_instructions;
mod type_helpers;

pub use check_script_output::{run_script_with_vm, test_script};
pub use doc_examples::run_koto_examples_in_markdown;
pub use output_capture::OutputCapture;
pub use script_instructions::script_instructions;
pub use type_helpers::*;
