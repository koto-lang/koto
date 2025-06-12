use std::{
    fmt,
    io::{self, Stdout, Write},
    path::PathBuf,
    rc::Rc,
};

use anyhow::Result;
use crossterm::{execute, style, tty::IsTty};
use koto::prelude::*;
use rustyline::{CompletionType, Config, Editor, error::ReadlineError, history::DefaultHistory};
use serde::{Deserialize, Serialize};

use crate::{
    help::{HELP_INDENT, Help},
    wrap_string_with_indent, wrap_string_with_prefix,
};

macro_rules! print_wrapped_indented {
    ($stdout:expr, $indent:expr, $text:expr) => {
        $stdout.write_all(wrap_string_with_indent(&format!($text), $indent).as_bytes())
    };
    ($stdout:expr, $indent:expr, $text:literal, $($y:expr),* $(,)?) => {
        $stdout.write_all(wrap_string_with_indent(&format!($text,  $($y),*), $indent).as_bytes())
    };
}
macro_rules! print_wrapped {
    ($stdout:expr, $text:expr) => {
        print_wrapped_indented!($stdout, "", $text)
    };
    ($stdout:expr, $text:literal, $($y:expr),* $(,)?) => {
        print_wrapped_indented!($stdout, "", $text, $($y),*)
    };
}

const PROMPT: &str = "» ";
const CONTINUED_PROMPT: &str = "… ";
const RESULT_PROMPT: &str = "➝ ";
const INDENT_SIZE: usize = 2;
const HISTORY_DIR: &str = ".koto";
const HISTORY_FILE: &str = "repl_history.txt";

pub struct ReplSettings {
    pub show_bytecode: bool,
    pub show_instructions: bool,
    pub colored_output: bool,
    pub edit_mode: EditMode,
    pub max_history_size: usize,
}

#[derive(Copy, Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EditMode {
    #[default]
    Emacs,
    Vi,
}

impl From<EditMode> for rustyline::EditMode {
    fn from(mode: EditMode) -> Self {
        match mode {
            EditMode::Emacs => rustyline::EditMode::Emacs,
            EditMode::Vi => rustyline::EditMode::Vi,
        }
    }
}

type ReplEditor = Editor<ReplHelper, DefaultHistory>;

pub struct Repl {
    koto: Koto,
    settings: ReplSettings,
    editor: ReplEditor,
    stdout: Stdout,
    // A buffer of lines for expressions that continue over multiple lines
    continued_lines: Vec<String>,
    indent: usize,
    colored_output: bool,
}

fn history_dir() -> Option<PathBuf> {
    home::home_dir().map(|mut path| {
        path.push(HISTORY_DIR);
        path
    })
}

fn history_path() -> Option<PathBuf> {
    history_dir().map(|mut path| {
        path.push(HISTORY_FILE);
        path
    })
}

fn help() -> Rc<Help> {
    thread_local! {
        static HELP: Rc<Help> = Rc::new(Help::new());
    }

    HELP.with(|help| help.clone())
}

impl Repl {
    pub fn with_settings(settings: ReplSettings, koto_settings: KotoSettings) -> Result<Self> {
        let koto = Koto::with_settings(koto_settings);
        super::add_modules(&koto);

        let mut editor = ReplEditor::with_config(
            Config::builder()
                .max_history_size(settings.max_history_size)?
                .edit_mode(settings.edit_mode.into())
                .completion_type(CompletionType::List)
                .completion_show_all_if_ambiguous(true)
                .build(),
        )?;
        editor.set_helper(Some(ReplHelper {
            exports: koto.exports().clone(),
            prelude: koto.prelude().clone(),
        }));

        if let Some(path) = history_path() {
            editor.load_history(&path).ok();
        }

        let stdout = io::stdout();
        let colored_output = settings.colored_output && stdout.is_tty();

        Ok(Self {
            koto,
            settings,
            editor,
            stdout,
            continued_lines: Vec::new(),
            indent: 0,
            colored_output,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        let version = env!("CARGO_PKG_VERSION");
        writeln!(
            self.stdout,
            "\
Welcome to Koto v{version}
Run `help` for more information
"
        )?;

        loop {
            let result = if self.continued_lines.is_empty() {
                self.editor.readline(PROMPT)
            } else {
                let indent = " ".repeat(self.indent);
                self.editor
                    .readline_with_initial(CONTINUED_PROMPT, (&indent, ""))
            };

            match result {
                Ok(line) => {
                    self.on_line(&line)?;
                }
                Err(ReadlineError::Interrupted) => {
                    writeln!(self.stdout, "^C")?;
                    self.stdout.flush()?;
                    self.continued_lines.clear();
                    self.indent = 0;
                }
                Err(ReadlineError::Eof) => {
                    break;
                }
                Err(err) => {
                    writeln!(self.stdout, "Error: {err:?}")?;
                    break;
                }
            }
        }

        if let Some(mut path) = history_dir() {
            std::fs::create_dir_all(&path)?;
            path.push(HISTORY_FILE);
            self.editor.save_history(&path)?;
        }

        Ok(())
    }

    fn on_line(&mut self, line: &str) -> Result<()> {
        let input_is_whitespace = line.chars().all(|c| c.is_whitespace());

        let mut indent_next_line = false;

        if self.continued_lines.is_empty() || input_is_whitespace {
            let mut input = self.continued_lines.join("\n");

            if !input_is_whitespace {
                input += line;
            }

            self.editor.add_history_entry(&input)?;

            let compile_args = CompileArgs::new(&input).export_top_level_ids(true);
            match self.koto.compile(compile_args) {
                Ok(chunk) => {
                    if self.settings.show_bytecode {
                        print_wrapped!(self.stdout, "{}\n", &Chunk::bytes_as_string(&chunk))?;
                    }
                    if self.settings.show_instructions {
                        print_wrapped!(self.stdout, "Constants\n---------\n{}\n", chunk.constants)?;

                        let script_lines = input.lines().collect::<Vec<_>>();

                        print_wrapped!(
                            self.stdout,
                            "Instructions\n------------\n{}",
                            Chunk::instructions_as_string(chunk.clone(), &script_lines)
                        )?;
                    }
                    match self.koto.run(chunk) {
                        Ok(result) => match self.koto.value_to_string(result.clone()) {
                            Ok(result_string) => {
                                self.print_result(&result_string)?;
                            }
                            Err(e) => {
                                print_wrapped!(
                                    self.stdout,
                                    "Error while getting display string for return value ({})",
                                    e
                                )?;
                            }
                        },
                        Err(error) => {
                            if let Some(help) = self.run_help(&input) {
                                print_wrapped_indented!(self.stdout, HELP_INDENT, "{help}")?;
                                writeln!(self.stdout)?;
                            } else {
                                self.print_error(&error)?;
                            }
                        }
                    }
                    self.continued_lines.clear();
                }
                Err(compile_error) => {
                    if let Some(help) = self.run_help(&input) {
                        print_wrapped!(self.stdout, "{}\n", help)?;
                        self.continued_lines.clear();
                    } else if compile_error.is_indentation_error()
                        && self.continued_lines.is_empty()
                    {
                        self.continued_lines.push(line.to_string());
                        indent_next_line = true;
                    } else {
                        self.editor.add_history_entry(&input)?;

                        self.print_error(&compile_error.to_string())?;
                        self.continued_lines.clear();
                    }
                }
            }
        } else {
            // We're in a continued expression, so cache the input for execution later
            self.continued_lines.push(line.to_string());

            // Check if we should add indentation on the next line
            let input = self.continued_lines.join("\n");
            if let Err(e) = self.koto.compile(&input) {
                if e.is_indentation_error() {
                    indent_next_line = true;
                }
            }
        }

        if self.continued_lines.is_empty() {
            self.indent = 0;
        } else {
            let current_indent = self
                .continued_lines
                .last()
                .unwrap()
                .find(|c: char| !c.is_whitespace())
                .unwrap_or(0);

            self.indent = if indent_next_line {
                current_indent + INDENT_SIZE
            } else {
                current_indent
            };
        };

        Ok(())
    }

    fn run_help(&mut self, input: &str) -> Option<String> {
        let input = input.trim();
        if input == "help" {
            Some(help().get_help(None))
        } else {
            input
                .strip_prefix("help ")
                .map(|search_string| format!("\n{}\n", help().get_help(Some(search_string))))
        }
    }

    fn print_result(&mut self, result: &str) -> Result<()> {
        if self.colored_output {
            use style::*;

            execute!(
                self.stdout,
                Print(RESULT_PROMPT),
                SetAttribute(Attribute::Bold),
                Print(wrap_string_with_prefix(
                    &format!("{result}\n\n"),
                    RESULT_PROMPT
                )),
                SetAttribute(Attribute::Reset),
            )?;
        } else {
            print_wrapped!(self.stdout, "{RESULT_PROMPT}{result}\n\n")?;
        }

        Ok(())
    }

    fn print_error<E>(&mut self, error: &E) -> Result<()>
    where
        E: fmt::Display,
    {
        if self.colored_output {
            use style::*;

            execute!(
                self.stdout,
                SetForegroundColor(Color::DarkRed),
                Print("error"),
                ResetColor,
                Print(": "),
                SetAttribute(Attribute::Bold),
                Print(wrap_string_with_prefix(
                    &format!("{error:#}\n\n"),
                    "error: "
                )),
                SetAttribute(Attribute::Reset),
            )?;
        } else {
            print_wrapped!(self.stdout, "error: {error:#}\n\n")?;
        }

        Ok(())
    }
}

#[derive(Default)]
struct ReplHelper {
    exports: KMap,
    prelude: KMap,
}

impl ReplHelper {
    fn candidates_from_help(
        &self,
        search: &str,
        line: &str,
    ) -> rustyline::Result<(usize, Vec<CompletionCandidate>)> {
        let stripped_search = search.trim_start();
        let offset = line.len() - stripped_search.len();
        let candidates: Vec<_> = if stripped_search.is_empty() {
            help()
                .topics()
                .map(|topic| CompletionCandidate {
                    contents: topic.clone(),
                })
                .collect()
        } else {
            let lowercase_search = stripped_search.to_lowercase();
            help()
                .all_entries()
                .filter(|(key, _entry)| key.starts_with(&lowercase_search))
                .map(|(key, _entry)| CompletionCandidate {
                    contents: key.clone(),
                })
                .collect()
        };
        Ok((offset, candidates))
    }

    fn candidates_from_koto_items(
        &self,
        line: &str,
        pos: usize,
    ) -> rustyline::Result<(usize, Vec<CompletionCandidate>)> {
        let offset = if let Some(whitespace) = line[..pos].rfind(char::is_whitespace) {
            whitespace + 1
        } else {
            0
        };
        let search = &line[offset..pos];

        let candidates: Vec<_> = self
            .exports
            .data()
            .keys()
            .chain(self.prelude.data().keys())
            .filter_map(|key| match key.value() {
                KValue::Str(s) if s.starts_with(search) => Some(CompletionCandidate {
                    contents: s.as_str().into(),
                }),
                _ => None,
            })
            .collect();

        if candidates.is_empty() && "help".starts_with(search) {
            Ok((
                offset,
                vec![CompletionCandidate {
                    contents: "help".into(),
                }],
            ))
        } else {
            Ok((offset, candidates))
        }
    }
}

impl rustyline::completion::Completer for ReplHelper {
    type Candidate = CompletionCandidate;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        if let Some(search) = line.trim_start().strip_prefix("help ") {
            self.candidates_from_help(search, line)
        } else {
            self.candidates_from_koto_items(line, pos)
        }
    }
}

impl rustyline::hint::Hinter for ReplHelper {
    type Hint = String;
}
impl rustyline::highlight::Highlighter for ReplHelper {}
impl rustyline::validate::Validator for ReplHelper {}
impl rustyline::Helper for ReplHelper {}

struct CompletionCandidate {
    contents: Rc<str>,
}

impl rustyline::completion::Candidate for CompletionCandidate {
    fn display(&self) -> &str {
        &self.contents
    }

    fn replacement(&self) -> &str {
        &self.contents
    }
}
