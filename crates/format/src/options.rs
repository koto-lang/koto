use serde::{Deserialize, Serialize};

/// Formatting options provided to [`format`]
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct FormatOptions {
    /// The width in characters to use when inserting indents. (default: 2)
    pub indent_width: u8,
    /// The maximum line length. (default: 100)
    pub line_length: u8,
    /// Whether or not indented linebreaks will always be inserted after `then` and `else` in
    /// `match` and `switch` arms. (default: false)
    pub match_and_switch_always_indent_arm_bodies: bool,
    /// The threshold that causes chain expressions to be broken onto multiple lines. (default: 3)
    ///
    /// The threshold counts against the number of `.` accesses that are followed by a call or index.
    ///
    /// `a.b.c().d()` - 2 `.` accesses that count against the threshold.
    /// `a[0].b[1].c().d - 3 `.` accesses that count against the threshold`.
    ///
    /// A value of `0` disables the threshold.
    pub chain_break_threshold: u8,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            indent_width: 2,
            line_length: 100,
            match_and_switch_always_indent_arm_bodies: false,
            chain_break_threshold: 3,
        }
    }
}
