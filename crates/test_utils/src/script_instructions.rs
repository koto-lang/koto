use koto_bytecode::Chunk;
use koto_runtime::Ptr;
use std::fmt::Write;

/// Renders a script to a string with its corresponding compiled instructions
pub fn script_instructions(script: &str, chunk: Ptr<Chunk>) -> String {
    let mut result = format!("{script}\n\n");

    let script_lines = script.lines().collect::<Vec<_>>();

    write!(result, "Constants\n---------\n{}\n", chunk.constants).ok();
    write!(
        result,
        "Instructions\n------------\n{}",
        Chunk::instructions_as_string(chunk, &script_lines)
    )
    .ok();

    result
}
