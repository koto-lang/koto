mod help;
mod repl;

use anyhow::{bail, Context, Result};
use crossterm::tty::IsTty;
use koto::prelude::*;
use repl::{Repl, ReplSettings};
use rustyline::EditMode;
use std::{
    env,
    error::Error,
    fs, io,
    path::{Path, PathBuf},
};

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
    -c, --config PATH        Config file to load when using the REPL
    -v, --version            Prints version information
    -h, --help               Prints help information

ARGS:
    <script>     The koto script to run, as a file path, or as a string when --eval is set
    <args>...    Arguments to pass into the script

REPL CONFIGURATION:
    Koto will read configuration settings from $HOME/.koto/repl_config.koto,
    or from a file provided with the --config flag.

    The default configuration settings are:

    ```
    export
      colored_output: true
      edit_mode: 'emacs'
      max_history: 100
    ```

ENV VARS:
    KOTO_EDIT_MODE_VI   Enables the VI editing mode (Emacs bindings are enabled by default)
    KOTO_MAX_HISTORY    The maximum number of entries to store in the REPL history (default: 100)
    NO_COLOR            Disables colored output (enabled by default)
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
    script: Option<String>,
    script_args: Vec<String>,
    config_file: Option<String>,
}

fn parse_arguments() -> Result<KotoArgs> {
    let mut args = pico_args::Arguments::from_env();

    let eval_script = args.contains(["-e", "--eval"]);
    let show_instructions = args.contains(["-i", "--show_instructions"]);
    let show_bytecode = args.contains(["-b", "--show_bytecode"]);
    let run_tests = args.contains(["-t", "--tests"]);
    let run_import_tests = args.contains(["-T", "--import_tests"]);
    let help = args.contains(["-h", "--help"]);
    let version = args.contains(["-v", "--version"]);
    let config_file = args.opt_value_from_str(["-c", "--config"])?;

    let script = args.subcommand()?;

    let script_args = match args.free() {
        Ok(extra_args) => extra_args,
        Err(e) => match e {
            pico_args::Error::UnusedArgsLeft(unused) => {
                bail!("Unsupported argument: {}", unused.first().unwrap())
            }
            other => bail!("Error while parsing arguments: {other}"),
        },
    };

    Ok(KotoArgs {
        help,
        version,
        eval_script,
        run_tests,
        run_import_tests,
        show_bytecode,
        show_instructions,
        script,
        script_args,
        config_file,
    })
}

fn main() -> Result<()> {
    let args = match parse_arguments() {
        Ok(args) => args,
        Err(error) => {
            bail!("{}\n\n{}", help_string(), error);
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

    let koto_settings = KotoSettings {
        run_tests: args.run_tests || args.run_import_tests,
        vm_settings: KotoVmSettings {
            run_import_tests: args.run_import_tests,
            ..Default::default()
        },
        ..Default::default()
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
        let mut koto = Koto::with_settings(koto_settings);
        if let Err(error) = koto.set_script_path(script_path.as_deref().map(Path::new)) {
            bail!("{error}");
        }

        add_modules(&koto);

        match koto.compile(&script) {
            Ok(chunk) => {
                if args.show_bytecode {
                    println!("{}\n", &Chunk::bytes_as_string(&chunk));
                }
                if args.show_instructions {
                    println!("Constants\n---------\n{}\n", chunk.constants);

                    let script_lines = script.lines().collect::<Vec<_>>();
                    println!(
                        "Instructions\n------------\n{}",
                        Chunk::instructions_as_string(chunk, &script_lines)
                    );
                }
                koto.set_args(&args.script_args)?;
                match koto.run() {
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
    } else {
        let config = load_config(args.config_file.as_ref())?;

        Repl::with_settings(
            ReplSettings {
                show_instructions: args.show_instructions,
                show_bytecode: args.show_bytecode,
                colored_output: config.colored_output,
                edit_mode: config.edit_mode,
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

struct Config {
    edit_mode: EditMode,
    colored_output: bool,
    max_history: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            edit_mode: EditMode::Emacs,
            colored_output: true,
            max_history: 100,
        }
    }
}

fn load_config(config_path: Option<&String>) -> Result<Config> {
    let mut config = Config::default();

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
    if let Some(config_path) = config_path {
        let script = fs::read_to_string(config_path).context("Failed to load the config file")?;

        let mut koto = Koto::new();
        match koto.compile_and_run(&script) {
            Ok(_) => {
                let exports = koto.exports().data();
                match exports.get("repl") {
                    Some(KValue::Map(repl_config)) => {
                        let repl_config = repl_config.data();
                        match repl_config.get("colored_output") {
                            Some(KValue::Bool(value)) => config.colored_output = *value,
                            Some(_) => bail!("expected bool for colored_output setting"),
                            None => {}
                        }
                        match repl_config.get("edit_mode") {
                            Some(KValue::Str(value)) => match value.as_str() {
                                "emacs" => config.edit_mode = EditMode::Emacs,
                                "vi" => config.edit_mode = EditMode::Vi,
                                other => {
                                    bail!(
                                        "invalid edit mode '{other}',
                                         valid options are 'emacs' or 'vi'"
                                    )
                                }
                            },
                            Some(_) => bail!("expected string for edit_mode setting"),
                            None => {}
                        }
                        match repl_config.get("max_history") {
                            Some(KValue::Number(value)) => match value.as_i64() {
                                value if value > 0 => config.max_history = value as usize,
                                _ => bail!("expected positive number for max_history setting"),
                            },
                            Some(_) => bail!("expected positive number for max_history setting"),
                            None => {}
                        }
                    }
                    Some(_) => bail!("expected map for repl settings"),
                    None => {}
                }
            }
            Err(e) => bail!("error while loading config: {e}",),
        }
    }

    // Apply environment variables
    if env::var("KOTO_EDIT_MODE_VI").is_ok() {
        config.edit_mode = EditMode::Vi
    };

    if let Ok(value) = env::var("KOTO_MAX_HISTORY") {
        if let Ok(value) = value.parse::<usize>() {
            config.max_history = value;
        } else {
            bail!("expected integer for KOTO_MAX_HISTORY environment variable");
        }
    }

    Ok(config)
}
