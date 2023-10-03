use std::{
    fmt,
    io::{self, Stdout, Write},
    path::PathBuf,
};

use crossterm::{
    execute, style,
    terminal::{self},
    tty::IsTty,
};
use koto::prelude::*;
use rustyline::{error::ReadlineError, Config, DefaultEditor, EditMode};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

use crate::help::Help;

macro_rules! print_wrapped {
    ($stdout:expr, $text:expr) => {
        writeln!($stdout, "{}", wrap_string($text))
    };
    ($stdout:expr, $text:literal, $($y:expr),+ $(,)?) => {
        print_wrapped!($stdout, &format!($text, $($y),+))
    };
}

const PROMPT: &str = "» ";
const CONTINUED_PROMPT: &str = "… ";
const RESULT_PROMPT: &str = "➝ ";
const INDENT_SIZE: usize = 2;
const HISTORY_DIR: &str = ".koto";
const HISTORY_FILE: &str = "repl_history.txt";
const MAX_HISTORY_ENTRIES: usize = 500;

#[derive(Default)]
pub struct ReplSettings {
    pub show_bytecode: bool,
    pub show_instructions: bool,
}

pub struct Repl {
    koto: Koto,
    settings: ReplSettings,
    help: Option<Help>,
    editor: DefaultEditor,
    // A buffer of lines for expressions that continue over multiple lines
    continued_lines: Vec<String>,
    indent: usize,
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

impl Repl {
    pub fn with_settings(
        repl_settings: ReplSettings,
        mut koto_settings: KotoSettings,
    ) -> Result<Self> {
        koto_settings.export_top_level_ids = true;

        let koto = Koto::with_settings(koto_settings);
        super::add_modules(&koto);

        let mut editor = DefaultEditor::with_config(
            Config::builder()
                .max_history_size(MAX_HISTORY_ENTRIES)?
                .edit_mode(EditMode::Emacs)
                .build(),
        )?;

        if let Some(path) = history_path() {
            editor.load_history(&path).ok();
        }

        Ok(Self {
            koto,
            settings: repl_settings,
            help: None,
            editor,
            continued_lines: Vec::new(),
            indent: 0,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        let mut stdout = io::stdout();

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
                    self.on_line(&line, &mut stdout)?;
                }
                Err(ReadlineError::Interrupted) => {
                    writeln!(stdout, "^C")?;
                    break;
                }
                Err(ReadlineError::Eof) => {
                    writeln!(stdout, "^D")?;
                    break;
                }
                Err(err) => {
                    writeln!(stdout, "Error: {:?}", err)?;
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

    fn on_line(&mut self, line: &str, stdout: &mut Stdout) -> Result<()> {
        let input_is_whitespace = line.chars().all(|c| c.is_whitespace());

        let mut indent_next_line = false;

        if self.continued_lines.is_empty() || input_is_whitespace {
            let mut input = self.continued_lines.join("\n");

            if !input_is_whitespace {
                input += line;
            }

            self.editor.add_history_entry(&input)?;

            match self.koto.compile(&input) {
                Ok(chunk) => {
                    if self.settings.show_bytecode {
                        print_wrapped!(stdout, "{}\n", &Chunk::bytes_as_string(&chunk))?;
                    }
                    if self.settings.show_instructions {
                        print_wrapped!(stdout, "Constants\n---------\n{}\n", chunk.constants)?;

                        let script_lines = input.lines().collect::<Vec<_>>();
                        print_wrapped!(
                            stdout,
                            "Instructions\n------------\n{}",
                            Chunk::instructions_as_string(chunk, &script_lines)
                        )?;
                    }
                    match self.koto.run() {
                        Ok(result) => match self.koto.value_to_string(result.clone()) {
                            Ok(result_string) => {
                                print_result(stdout, &result_string)?;
                            }
                            Err(e) => {
                                print_wrapped!(
                                    stdout,
                                    "Error while getting display string for return value ({})",
                                    e
                                )?;
                            }
                        },
                        Err(error) => {
                            if let Some(help) = self.run_help(&input) {
                                print_wrapped!(stdout, "{}\n", help)?;
                            } else {
                                print_error(stdout, &error)?;
                            }
                        }
                    }
                    self.continued_lines.clear();
                }
                Err(compile_error) => {
                    if let Some(help) = self.run_help(&input) {
                        print_wrapped!(stdout, "{}\n", help)?;
                        self.continued_lines.clear();
                    } else if compile_error.is_indentation_error()
                        && self.continued_lines.is_empty()
                    {
                        self.continued_lines.push(line.to_string());
                        indent_next_line = true;
                    } else {
                        self.editor.add_history_entry(&input)?;

                        print_error(stdout, &compile_error.to_string())?;
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
            Some(self.get_help(None))
        } else if input.starts_with("help") {
            input
                .split_once(char::is_whitespace)
                .map(|(_, search_string)| format!("\n{}", self.get_help(Some(search_string))))
        } else {
            None
        }
    }

    fn get_help(&mut self, search: Option<&str>) -> String {
        let help = self.help.get_or_insert_with(Help::new);
        help.get_help(search)
    }
}

fn terminal_width() -> usize {
    terminal::size().expect("Failed to get terminal width").0 as usize
}

fn wrap_string(input: &str) -> String {
    textwrap::fill(input, terminal_width())
}

fn wrap_string_with_prefix(input: &str, prefix: &str) -> String {
    textwrap::fill(input, terminal_width().saturating_sub(prefix.len()))
}

fn print_result(stdout: &mut Stdout, result: &str) -> Result<()> {
    if stdout.is_tty() {
        use style::*;

        execute!(
            stdout,
            Print(RESULT_PROMPT),
            SetAttribute(Attribute::Bold),
            Print(wrap_string_with_prefix(
                &format!("{result}\n\n"),
                RESULT_PROMPT
            )),
            SetAttribute(Attribute::Reset),
        )?;
    } else {
        print_wrapped!(stdout, "{RESULT_CHAR}{result}\n\n")?;
    }

    Ok(())
}

fn print_error<E>(stdout: &mut Stdout, error: &E) -> Result<()>
where
    E: fmt::Display,
{
    if stdout.is_tty() {
        use style::*;

        execute!(
            stdout,
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
        print_wrapped!(stdout, "{error:#}")?;
    }

    Ok(())
}
