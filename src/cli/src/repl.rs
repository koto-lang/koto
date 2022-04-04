use {
    crate::help::Help,
    crossterm::{
        cursor,
        event::{read, Event, KeyCode, KeyEvent, KeyModifiers},
        execute, queue, style,
        terminal::{self, ClearType},
        tty::IsTty,
        Result,
    },
    koto::{bytecode::Chunk, runtime::Value as KotoValue, Koto, KotoSettings},
    std::{
        cmp::Ordering,
        fmt,
        io::{self, Stdout, Write},
    },
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

const PROMPT: &str = "» ";
const CONTINUED_PROMPT: &str = "… ";

const RESULT_CHAR: &str = "➝";

const INDENT_SIZE: usize = 2;

#[derive(Default)]
pub struct ReplSettings {
    pub show_bytecode: bool,
    pub show_instructions: bool,
}

#[derive(Default)]
pub struct Repl {
    koto: Koto,
    settings: ReplSettings,
    help: Option<Help>,
    input: String,
    continued_lines: Vec<String>,
    input_history: Vec<String>,
    history_position: Option<usize>,
    cursor: Option<usize>,
}

impl Repl {
    pub fn with_settings(repl_settings: ReplSettings, mut koto_settings: KotoSettings) -> Self {
        koto_settings.repl_mode = true;

        let koto = Koto::with_settings(koto_settings);

        let prelude = koto.prelude();
        prelude.add_map("json", koto_json::make_module());
        prelude.add_map("random", koto_random::make_module());
        prelude.add_map("tempfile", koto_tempfile::make_module());
        prelude.add_map("toml", koto_toml::make_module());
        prelude.add_map("yaml", koto_yaml::make_module());

        Self {
            koto,
            settings: repl_settings,
            ..Self::default()
        }
    }

    pub fn run(&mut self) -> Result<()> {
        let mut stdout = io::stdout();

        write!(stdout, "Welcome to Koto v{VERSION}\r\n{PROMPT}").unwrap();
        stdout.flush().unwrap();

        loop {
            if stdout.is_tty() {
                terminal::enable_raw_mode()?;
            }

            if let Event::Key(key_event) = read()? {
                self.on_keypress(key_event, &mut stdout)?;

                if stdout.is_tty() {
                    let (_, cursor_y) = cursor::position()?;

                    let prompt = if self.continued_lines.is_empty() {
                        PROMPT
                    } else {
                        CONTINUED_PROMPT
                    };

                    queue!(
                        stdout,
                        cursor::MoveTo(0, cursor_y),
                        terminal::Clear(ClearType::CurrentLine),
                        style::Print(prompt),
                        style::Print(&self.input),
                    )?;

                    if let Some(position) = self.cursor {
                        if position < self.input.len() {
                            let x_offset = (self.input.len() - position) as u16;
                            let (cursor_x, cursor_y) = cursor::position()?;
                            queue!(stdout, cursor::MoveTo(cursor_x - x_offset, cursor_y))?;
                        }
                    }

                    stdout.flush().unwrap();
                }
            }
        }
    }

    fn on_keypress(&mut self, event: KeyEvent, stdout: &mut Stdout) -> Result<()> {
        match event.code {
            KeyCode::Up => {
                if !self.input_history.is_empty() {
                    let new_position = match self.history_position {
                        Some(position) => {
                            if position > 0 {
                                position - 1
                            } else {
                                0
                            }
                        }
                        None => self.input_history.len() - 1,
                    };
                    self.input = self.input_history[new_position].clone();
                    self.cursor = None;
                    self.history_position = Some(new_position);
                }
            }
            KeyCode::Down => {
                self.history_position = match self.history_position {
                    Some(position) => {
                        if position < self.input_history.len() - 1 {
                            Some(position + 1)
                        } else {
                            None
                        }
                    }
                    None => None,
                };
                if let Some(position) = self.history_position {
                    self.input = self.input_history[position].clone();
                } else {
                    self.input.clear();
                }
                self.cursor = None;
            }
            KeyCode::Left => match self.cursor {
                Some(position) => {
                    if position > 0 {
                        self.cursor = Some(position - 1);
                    }
                }
                None => {
                    if !self.input.is_empty() {
                        self.cursor = Some(self.input.len() - 1);
                    }
                }
            },
            KeyCode::Right => {
                if let Some(position) = self.cursor {
                    if position < self.input.len() - 1 {
                        self.cursor = Some(position + 1);
                    } else {
                        self.cursor = None;
                    }
                }
            }
            KeyCode::Backspace => {
                let cursor = self.cursor;
                match cursor {
                    Some(position) => {
                        let new_position = position - 1;
                        self.input.remove(new_position);
                        if self.input.is_empty() {
                            self.cursor = None;
                        } else {
                            self.cursor = Some(new_position);
                        }
                    }
                    None => {
                        self.input.pop();
                    }
                }
            }
            KeyCode::Delete => {
                if let Some(position) = self.cursor {
                    match position.cmp(&(self.input.len() - 1)) {
                        Ordering::Less => {
                            self.input.remove(position);
                        }
                        Ordering::Equal => {
                            self.input.pop();
                        }
                        Ordering::Greater => {}
                    }

                    if self.input.is_empty() {
                        self.cursor = None;
                    }
                }
            }
            KeyCode::Enter => self.on_enter(stdout)?,
            KeyCode::Char(c) if event.modifiers.contains(KeyModifiers::CONTROL) => match c {
                'c' => {
                    if self.input.is_empty() {
                        write!(stdout, "^C\r\n").unwrap();
                        stdout.flush().unwrap();
                        if stdout.is_tty() {
                            terminal::disable_raw_mode()?;
                        }
                        std::process::exit(0)
                    } else {
                        self.input.clear();
                        self.cursor = None;
                    }
                }
                'd' if self.input.is_empty() => {
                    write!(stdout, "^D\r\n").unwrap();
                    stdout.flush().unwrap();
                    if stdout.is_tty() {
                        terminal::disable_raw_mode()?;
                    }
                    std::process::exit(0)
                }
                _ => {}
            },
            KeyCode::Char(c) => {
                let cursor = self.cursor;
                match cursor {
                    Some(position) => {
                        self.input.insert(position, c);
                        self.cursor = Some(position + 1);
                    }
                    None => self.input.push(c),
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn on_enter(&mut self, stdout: &mut Stdout) -> Result<()> {
        if stdout.is_tty() {
            terminal::disable_raw_mode()?;
        }

        println!();

        let mut indent_next_line = false;

        let input_is_whitespace = self.input.chars().all(char::is_whitespace);

        if self.continued_lines.is_empty() || input_is_whitespace {
            let mut input = self.continued_lines.join("\n");

            if !input_is_whitespace {
                input += &self.input;
            }

            match self.koto.compile(&input) {
                Ok(chunk) => {
                    if self.settings.show_bytecode {
                        println!("{}\n", &Chunk::bytes_as_string(chunk.clone()));
                    }
                    if self.settings.show_instructions {
                        println!("Constants\n---------\n{}\n", chunk.constants);

                        let script_lines = input.lines().collect::<Vec<_>>();
                        println!(
                            "Instructions\n------------\n{}",
                            Chunk::instructions_as_string(chunk, &script_lines)
                        );
                    }
                    match self.koto.run() {
                        Ok(result) => match self.koto.value_to_string(result.clone()) {
                            Ok(result_string) => {
                                self.print_result(stdout, &result_string)?;
                            }
                            Err(e) => {
                                writeln!(
                                    stdout,
                                    "Error while getting display string for value '{}' - {}",
                                    result, e
                                )
                                .unwrap();
                            }
                        },
                        Err(error) => {
                            if let Some(help) = self.run_help(&input) {
                                writeln!(stdout, "{}\n", help).unwrap()
                            } else {
                                self.print_error(stdout, &error)?;
                            }
                        }
                    }
                    self.continued_lines.clear();
                }
                Err(e) => {
                    if e.is_indentation_error() && self.continued_lines.is_empty() {
                        self.continued_lines.push(self.input.clone());
                        indent_next_line = true;
                    } else if let Some(help) = self.run_help(&input) {
                        writeln!(stdout, "{}\n", help).unwrap()
                    } else {
                        self.print_error(stdout, &e.to_string())?;
                        self.continued_lines.clear();
                    }
                }
            }
        } else {
            // We're in a continued expression, so cache the input for execution later
            self.continued_lines.push(self.input.clone());

            // Check if we should add indentation on the next line
            let input = self.continued_lines.join("\n");
            if let Err(e) = self.koto.compile(&input) {
                if e.is_indentation_error() {
                    indent_next_line = true;
                }
            }
        }

        if !input_is_whitespace
            && (self.input_history.is_empty() || self.input_history.last().unwrap() != &self.input)
        {
            self.input_history.push(self.input.clone());
        }

        self.history_position = None;
        self.cursor = None;

        let current_indent = if self.continued_lines.is_empty() {
            0
        } else {
            self.continued_lines
                .last()
                .unwrap()
                .find(|c: char| !c.is_whitespace())
                .unwrap_or(0)
        };

        let indent = if indent_next_line {
            current_indent + INDENT_SIZE
        } else {
            current_indent
        };

        self.input = " ".repeat(indent);

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

    fn print_result(&self, stdout: &mut Stdout, result: &KotoValue) -> Result<()> {
        if stdout.is_tty() {
            use style::*;

            execute!(
                stdout,
                Print(RESULT_CHAR),
                SetAttribute(Attribute::Bold),
                Print(format!(" {result}\n\n")),
                SetAttribute(Attribute::Reset),
            )
        } else {
            writeln!(stdout, "{RESULT_CHAR} {result}\n\n")
        }
    }

    fn print_error<E>(&self, stdout: &mut Stdout, error: &E) -> Result<()>
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
                Print(format!("{error:#}\n\n")),
                SetAttribute(Attribute::Reset),
            )
        } else {
            writeln!(stdout, "{error:#}")
        }
    }
}
