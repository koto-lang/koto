use serde::{Deserialize, Serialize};

/// Formatting options provided to [`format`]
// When updating these options, remember to also update `cli.md`.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct FormatOptions {
    ///Whether or not `match` and `switch` arms should always be indented. (default: `false`)
    pub always_indent_arms: bool,
    /// The width in characters to use when inserting indents. (default: 2)
    pub indent_width: u8,
    /// The maximum line length. (default: 100)
    pub line_length: u8,
    /// The threshold that causes chain expressions to be broken onto multiple lines. (default: 4)
    ///
    /// The threshold counts against the number of `.` accesses that are followed by a call or index.
    ///
    /// `a.b.c().d()` - 2 `.` accesses that count against the threshold.
    /// `a[0].b[1].c().d().e()` - 4 `.` accesses that count against the threshold.
    ///
    /// A value of `0` disables the threshold.
    pub chain_break_threshold: u8,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            always_indent_arms: false,
            chain_break_threshold: 4,
            indent_width: 2,
            line_length: 100,
        }
    }
}
