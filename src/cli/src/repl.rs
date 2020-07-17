use {
    koto::{Koto, Options},
    std::{
        fmt,
        io::{self, Stdout, Write},
    },
    termion::{
        clear, color, cursor, cursor::DetectCursorPos, event::Key, input::TermRead,
        raw::IntoRawMode, raw::RawTerminal, style,
    },
};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const PROMPT: &str = "» ";

pub struct Repl {
    koto: Koto,
    input_history: Vec<String>,
    history_position: Option<usize>,
    input: String,
    cursor: Option<usize>,
}

impl Repl {
    pub fn with_options(mut options: Options) -> Self {
        options.repl_mode = true;

        Self {
            koto: Koto::with_options(options),
            input_history: Vec::new(),
            history_position: None,
            input: String::new(),
            cursor: None,
        }
    }

    pub fn run(&mut self) {
        let stdin = io::stdin();
        let mut stdout = io::stdout();
        let mut tty = if termion::is_tty(&stdout) {
            match termion::get_tty() {
                Ok(tty) => Some(tty.into_raw_mode().expect("Failed to activate raw mode")),
                Err(_) => None,
            }
        } else {
            None
        };

        write!(stdout, "Welcome to Koto v{}\r\n{}", VERSION, PROMPT).unwrap();
        stdout.flush().unwrap();

        for c in stdin.keys() {
            self.handle_keypress(c.unwrap(), &mut stdout, &mut tty);

            if let Some(ref mut tty) = tty {
                let (_, cursor_y) = stdout.cursor_pos().unwrap();

                write!(
                    tty,
                    "{}{}{}{}",
                    cursor::Goto(1, cursor_y),
                    clear::CurrentLine,
                    PROMPT,
                    self.input
                )
                .unwrap();

                if let Some(position) = self.cursor {
                    if position < self.input.len() {
                        let x_offset = (self.input.len() - position) as u16;
                        let (cursor_x, cursor_y) = stdout.cursor_pos().unwrap();
                        write!(tty, "{}", cursor::Goto(cursor_x - x_offset, cursor_y),).unwrap();
                    }
                }
            }

            stdout.flush().unwrap();
        }
    }

    fn handle_keypress<T>(
        &mut self,
        key: Key,
        stdout: &mut Stdout,
        tty: &mut Option<RawTerminal<T>>,
    ) where
        T: Write,
    {
        match key {
            Key::Up => {
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
            Key::Down => {
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
            Key::Left => match self.cursor {
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
            Key::Right => {
                if let Some(position) = self.cursor {
                    if position < self.input.len() - 1 {
                        self.cursor = Some(position + 1);
                    } else {
                        self.cursor = None;
                    }
                }
            }
            Key::Backspace => {
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
            Key::Char(c) => match c {
                '\n' => {
                    write!(stdout, "\r\n").unwrap();

                    if let Some(tty) = tty {
                        tty.suspend_raw_mode().unwrap();
                    }

                    match self.koto.compile(&self.input) {
                        Ok(_) => match self.koto.run() {
                            Ok(result) => write!(stdout, "{}\n", result).unwrap(),
                            Err(error) => self.print_error(stdout, tty, &error),
                        },
                        Err(e) => self.print_error(stdout, tty, &e),
                    }

                    if let Some(tty) = tty {
                        tty.activate_raw_mode().unwrap();
                    }

                    if self.input_history.is_empty()
                        || self.input_history.last().unwrap() != &self.input
                    {
                        self.input_history.push(self.input.clone());
                    }

                    self.history_position = None;
                    self.cursor = None;
                    self.input.clear();
                }
                _ => {
                    let cursor = self.cursor;
                    match cursor {
                        Some(position) => {
                            self.input.insert(position, c);
                            self.cursor = Some(position + 1);
                        }
                        None => self.input.push(c),
                    }
                }
            },
            Key::Ctrl(c) => match c {
                'c' => {
                    if self.input.is_empty() {
                        write!(stdout, "^C\r\n").unwrap();
                    } else {
                        self.input.clear();
                        self.cursor = None;
                    }
                }
                'd' => {
                    if self.input.is_empty() {
                        write!(stdout, "^D\r\n").unwrap();
                        stdout.flush().unwrap();
                        std::process::exit(0)
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn print_error<T, E>(&self, stdout: &mut Stdout, tty: &mut Option<RawTerminal<T>>, error: &E)
    where
        T: Write,
        E: fmt::Display,
    {
        if let Some(tty) = tty {
            write!(
                tty,
                "{red}error{reset}: {bold}",
                red = color::Fg(color::Red),
                bold = style::Bold,
                reset = style::Reset,
            )
            .unwrap();
            tty.suspend_raw_mode().unwrap();
            println!("{}", error);
            tty.activate_raw_mode().unwrap();
            write!(tty, "{}", style::Reset).unwrap();
        } else {
            write!(stdout, "{}", error).unwrap();
        }
    }
}
