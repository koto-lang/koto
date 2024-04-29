use crate::InstructionReader;
use koto_memory::Ptr;
use koto_parser::{ConstantPool, Span};
use std::{
    fmt::{self, Write},
    path::{Path, PathBuf},
};

/// Debug information for a Koto program
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DebugInfo {
    source_map: Vec<(u32, Span)>,
    /// The source of the program that the debug info was derived from
    pub source: String,
}

impl DebugInfo {
    /// Adds a span to the source map for a given ip
    ///
    /// Instructions with matching spans share the same entry, so if the span matches the
    /// previously pushed span then this is a no-op.
    pub fn push(&mut self, ip: u32, span: Span) {
        if let Some(entry) = self.source_map.last() {
            if entry.1 == span {
                // Don't add entries with matching spans, a search is performed in
                // get_source_span which will find the correct span
                // for intermediate ips.
                return;
            }
        }
        self.source_map.push((ip, span));
    }

    /// Returns a source span for a given instruction pointer
    pub fn get_source_span(&self, ip: u32) -> Option<Span> {
        // Find the last entry with an ip less than or equal to the input
        // an upper_bound would nice here, but this isn't currently a performance sensitive function
        // so a scan through the entries will do.
        let mut result = None;
        for entry in self.source_map.iter() {
            if entry.0 <= ip {
                result = Some(entry.1);
            } else {
                break;
            }
        }
        result
    }
}

/// A compiled chunk of bytecode, along with its associated constants and metadata
#[derive(Clone, Default, PartialEq)]
pub struct Chunk {
    /// The bytes representing the chunk's bytecode
    pub bytes: Box<[u8]>,
    /// The constant data associated with the chunk's bytecode
    pub constants: ConstantPool,
    /// The path of the program's source file
    pub source_path: Option<PathBuf>,
    /// Debug information associated with the chunk's bytecode
    pub debug_info: DebugInfo,
}

impl Chunk {
    /// Initializes a Chunk
    pub fn new(
        bytes: Box<[u8]>,
        constants: ConstantPool,
        source_path: Option<&Path>,
        debug_info: DebugInfo,
    ) -> Self {
        Self {
            bytes,
            constants,
            source_path: source_path.map(Path::to_path_buf),
            debug_info,
        }
    }

    /// Returns a [String] displaying the instructions contained in the compiled [Chunk]
    pub fn bytes_as_string(chunk: &Chunk) -> String {
        let mut iter = chunk.bytes.iter();
        let mut result = String::new();

        'outer: loop {
            for i in 1..=16 {
                match iter.next() {
                    Some(byte) => write!(result, "{byte:02x}").ok(),
                    None => break 'outer,
                };

                if i < 16 {
                    result += " ";

                    if i % 4 == 0 {
                        result += " ";
                    }
                }
            }
            result += "\n";
        }

        result
    }

    /// Returns a [String] displaying the annotated instructions contained in the compiled [Chunk]
    pub fn instructions_as_string(chunk: Ptr<Chunk>, source_lines: &[&str]) -> String {
        let mut result = String::new();
        let mut reader = InstructionReader::new(chunk);
        let mut ip = reader.ip;
        let mut span: Option<Span> = None;
        let mut first = true;

        while let Some(instruction) = reader.next() {
            let instruction_span = reader
                .chunk
                .debug_info
                .get_source_span(ip as u32)
                .expect("Missing source span");

            let print_source_lines = if let Some(span) = span {
                instruction_span.start.line != span.start.line
            } else {
                true
            };

            if print_source_lines && !source_lines.is_empty() {
                if !first {
                    result += "\n";
                }
                first = false;

                let line = instruction_span
                    .start
                    .line
                    .clamp(0, source_lines.len() as u32 - 1) as usize;
                writeln!(result, "|{line}| {}", source_lines[line]).ok();
                span = Some(instruction_span);
            }

            writeln!(result, "{ip}\t{instruction:?}").ok();
            ip = reader.ip;
        }

        result
    }
}

impl fmt::Debug for Chunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "Chunk ({self:p})")
    }
}
