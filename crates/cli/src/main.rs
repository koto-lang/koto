mod help;
mod repl;

use anyhow::{Context, Result, bail};
use crossterm::{terminal, tty::IsTty};
use koto::{
    prelude::*,
    runtime::{SystemStderr, SystemStdin, SystemStdout},
    serde::{from_koto_value, to_koto_value},
};
use koto_format::FormatOptions;
use repl::{EditMode, Repl, ReplSettings};
use serde::{Deserialize, Serialize};
use std::{env, error::Error, fs, io, path::PathBuf};

#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn help_string() -> String {
    format!(
        "{version}

USAGE:
    koto [FLAGS] [script] [<args>...]

FLAGS:
    -e, --eval               Evaluate the script as a string instead of loading it from disk
    -i, --show_instructions  Show compiled instructions annotated with source lines
    -b, --show_bytecode      Show the script's compiled bytecode
    -t, --tests              Run the script's tests before running the script
    -T, --import_tests       Run the script's tests, along with any tests in imported modules
    -f, --format             Formats the input, reading from the script path if given, or from stdin
    -c, --config PATH        Config file to load
    -C, --print_config       Prints the default config
    -v, --version            Prints version information
    -h, --help               Prints help information

ARGS:
    <script>     The koto script to run, as a file path, or as a string when --eval is set
    <args>...    Arguments to pass into the script

CONFIGURATION:
    Koto will read configuration settings from $HOME/.koto/config.koto,
    or from a file provided with the --config flag.

    Configuration settings are available for the REPL and for formatting options.
    The default configuration can be displayed with the --print_config flag.

ENV VARS:
    NO_COLOR     Disables colored output (enabled by default)
",
        version = version_string()
    )
}

fn version_string() -> String {
    format!("Koto {}", env!("CARGO_PKG_VERSION"))
}

#[derive(Debug, Default)]
struct KotoArgs {
    help: bool,
    version: bool,
    eval_script: bool,
    run_tests: bool,
    run_import_tests: bool,
    show_bytecode: bool,
    show_instructions: bool,
    format: bool,
    script: Option<String>,
    script_args: Vec<String>,
    config_file: Option<String>,
    print_config: bool,
}

fn parse_arguments() -> Result<KotoArgs> {
    let mut args = pico_args::Arguments::from_env();

    let eval_script = args.contains(["-e", "--eval"]);
    let show_instructions = args.contains(["-i", "--show_instructions"]);
    let show_bytecode = args.contains(["-b", "--show_bytecode"]);
    let run_tests = args.contains(["-t", "--tests"]);
    let run_import_tests = args.contains(["-T", "--import_tests"]);
    let format = args.contains(["-f", "--format"]);
    let config_file = args.opt_value_from_str(["-c", "--config"])?;
    let print_config = args.contains(["-C", "--print_config"]);
    let help = args.contains(["-h", "--help"]);
    let version = args.contains(["-v", "--version"]);

    let script = args.subcommand()?;

    let script_args = match args
        .finish()
        .drain(..)
        .map(|s| s.into_string())
        .collect::<Result<_, _>>()
    {
        Ok(args) => args,
        Err(_) => bail!("Arguments must be valid unicode strings"),
    };

    Ok(KotoArgs {
        help,
        version,
        eval_script,
        run_tests,
        run_import_tests,
        show_bytecode,
        show_instructions,
        format,
        script,
        script_args,
        config_file,
        print_config,
    })
}

fn main() -> Result<()> {
    let args = match parse_arguments() {
        Ok(args) => args,
        Err(error) => {
            bail!("{}\n\n{}", error, help_string());
        }
    };

    if args.help {
        println!("{}", help_string());
        return Ok(());
    }

    if args.version {
        println!("{}", version_string());
        return Ok(());
    }

    if args.print_config {
        return Config::print_default();
    }

    let koto_settings = KotoSettings {
        run_tests: args.run_tests || args.run_import_tests,
        vm_settings: KotoVmSettings {
            run_import_tests: args.run_import_tests,
            args: args.script_args,
            stdin: make_ptr!(SystemStdin::default()),
            stdout: make_ptr!(SystemStdout::default()),
            stderr: make_ptr!(SystemStderr::default()),
            ..Default::default()
        },
    };

    let mut stdin = io::stdin();

    let (script, script_path) = if let Some(script) = args.script {
        if args.eval_script {
            (Some(script), None)
        } else {
            let script_path = script;
            let script_contents = match fs::read_to_string(&script_path) {
                Ok(contents) => contents,
                Err(e) => {
                    bail!("Error while loading script: {e}");
                }
            };
            (Some(script_contents), Some(script_path))
        }
    } else if stdin.is_tty() {
        (None, None)
    } else {
        let script =
            io::read_to_string(&mut stdin).expect("Failed to read script from standard input");
        (Some(script), None)
    };

    if let Some(script) = script {
        if args.format {
            let config = load_config(args.config_file.as_ref())?;
            let formatted = koto_format::format(&script, config.format).with_context(|| {
                if let Some(path) = &script_path {
                    format!("failed to format '{path}'")
                } else {
                    "failed to format input from stdin".to_string()
                }
            })?;
            if let Some(path) = script_path {
                fs::write(path, formatted)?;
            } else {
                print!("{formatted}");
            }
            Ok(())
        } else {
            let mut koto = Koto::with_settings(koto_settings);

            add_modules(&koto);

            match koto.compile(CompileArgs {
                script: &script,
                script_path: script_path.map(KString::from),
                compiler_settings: Default::default(),
            }) {
                Ok(chunk) => {
                    if args.show_bytecode {
                        println!("{}\n", &Chunk::bytes_as_string(&chunk));
                    }
                    if args.show_instructions {
                        println!("Constants\n---------\n{}\n", chunk.constants);

                        let script_lines = script.lines().collect::<Vec<_>>();
                        println!(
                            "Instructions\n------------\n{}",
                            Chunk::instructions_as_string(chunk.clone(), &script_lines)
                        );
                    }
                    match koto.run(chunk) {
                        Ok(_) => {}
                        Err(error) if error.source().is_some() => {
                            bail!("{error}\n{}", error.source().unwrap())
                        }
                        Err(error) => {
                            bail!("{error}")
                        }
                    }
                }
                Err(error) => {
                    bail!("{error}")
                }
            }

            Ok(())
        }
    } else {
        let config = load_config(args.config_file.as_ref())?;

        Repl::with_settings(
            ReplSettings {
                show_instructions: args.show_instructions,
                show_bytecode: args.show_bytecode,
                colored_output: config.repl.colored_output,
                edit_mode: config.repl.edit_mode,
                max_history_size: config.repl.max_history,
            },
            koto_settings,
        )?
        .run()
    }
}

fn add_modules(koto: &Koto) {
    let prelude = koto.prelude();
    prelude.insert("color", koto_color::make_module());
    prelude.insert("geometry", koto_geometry::make_module());
    prelude.insert("json", koto_json::make_module());
    prelude.insert("random", koto_random::make_module());
    prelude.insert("regex", koto_regex::make_module());
    prelude.insert("tempfile", koto_tempfile::make_module());
    prelude.insert("toml", koto_toml::make_module());
    prelude.insert("yaml", koto_yaml::make_module());
}

#[derive(Deserialize, Serialize, Default, Debug)]
#[serde(default)]
struct Config {
    format: FormatOptions,
    repl: ReplConfig,
}

impl Config {
    fn print_default() -> Result<()> {
        let render_script = include_str!("render_export_map.koto");
        let mut koto = Koto::default();
        koto.compile_and_run(render_script)?;
        let rendered: String = from_koto_value(
            koto.call_exported_function("render_export_map", &[to_koto_value(Config::default())?])?,
        )?;
        println!("{rendered}");
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
struct ReplConfig {
    edit_mode: EditMode,
    colored_output: bool,
    max_history: usize,
}

impl Default for ReplConfig {
    fn default() -> Self {
        Self {
            edit_mode: EditMode::Emacs,
            colored_output: true,
            max_history: 100,
        }
    }
}

fn load_config(config_path: Option<&String>) -> Result<Config> {
    let config_path = config_path.map_or_else(
        || {
            home::home_dir()
                .map(|mut path| {
                    path.push(".koto");
                    path.push("config.koto");
                    path
                })
                .filter(|path| path.exists())
        },
        |path| Some(PathBuf::from(path)),
    );

    // Load the config file if it exists
    let config = if let Some(config_path) = config_path {
        let script = fs::read_to_string(&config_path).context("Failed to load the config file")?;

        let mut koto = Koto::new();
        if let Err(e) = koto.compile_and_run(CompileArgs::new(&script).script_path(config_path)) {
            bail!("error while loading config: {e}");
        }

        from_koto_value(koto.exports().clone()).context("error while loading config file")?
    } else {
        Config::default()
    };

    Ok(config)
}

fn terminal_width() -> usize {
    100.min(terminal::size().expect("Failed to get terminal width").0 as usize)
}

fn wrap_string_with_prefix(input: &str, prefix: &str) -> String {
    textwrap::fill(input, terminal_width().saturating_sub(prefix.len()))
}

fn wrap_string_with_indent(input: &str, indent: &str) -> String {
    textwrap::fill(
        input,
        textwrap::Options::new(terminal_width().saturating_sub(indent.len()))
            .initial_indent(indent)
            .subsequent_indent(indent),
    )
}
