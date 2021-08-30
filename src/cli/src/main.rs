mod help;
mod repl;

use {
    koto::{bytecode::Chunk, Koto, KotoSettings},
    repl::{Repl, ReplSettings},
    std::{
        fs,
        io::{self, Read},
    },
};

#[cfg(all(jemalloc, not(target_env = "msvc")))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

fn version_string() -> String {
    format!("Koto {}", env!("CARGO_PKG_VERSION"))
}

fn help_string() -> String {
    format!(
        "{version}

USAGE:
    koto [FLAGS] [script] [<args>...]

FLAGS:
    -e, --eval               Evaluate the script directly (rather than reading it from disk)
    -i, --show_instructions  Show compiled instructions annotated with source lines
    -b, --show_bytecode      Show the script's compiled bytecode
    -t, --tests              Run the script's tests before running the script
    -h, --help               Prints help information
    -v, --version            Prints version information

ARGS:
    <script>     The koto script to run, as a file path, or as a string when --eval is set
    <args>...    Arguments to pass into the script
",
        version = version_string()
    )
}

#[derive(Debug, Default)]
struct KotoArgs {
    help: bool,
    version: bool,
    eval_script: bool,
    run_tests: bool,
    show_bytecode: bool,
    show_instructions: bool,
    script: Option<String>,
    script_args: Vec<String>,
}

fn parse_arguments() -> Result<KotoArgs, String> {
    let mut args = pico_args::Arguments::from_env();

    let eval_script = args.contains(["-e", "--eval"]);
    let show_instructions = args.contains(["-i", "--show_instructions"]);
    let show_bytecode = args.contains(["-b", "--show_bytecode"]);
    let run_tests = args.contains(["-t", "--tests"]);
    let help = args.contains(["-h", "--help"]);
    let version = args.contains(["-v", "--version"]);

    let script = args
        .subcommand()
        .map_err(|e| format!("Error while parsing arguments: {}", e))?;

    let script_args = match args.free() {
        Ok(extra_args) => extra_args,
        Err(e) => {
            return Err(match e {
                pico_args::Error::UnusedArgsLeft(unused) => {
                    format!("Unsupported argument: {}", unused.first().unwrap())
                }
                other => format!("Error while parsing arguments: {}", other),
            })
        }
    };

    Ok(KotoArgs {
        help,
        version,
        eval_script,
        run_tests,
        show_bytecode,
        show_instructions,
        script,
        script_args,
    })
}

fn main() {
    std::process::exit(match run() {
        Ok(_) => 0,
        Err(_) => 1,
    })
}

fn run() -> Result<(), ()> {
    let args = match parse_arguments() {
        Ok(args) => args,
        Err(error) => {
            println!("{}\n\n{}", help_string(), error);
            return Err(());
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
        run_tests: args.run_tests,
        ..Default::default()
    };

    let mut stdin = io::stdin();

    let (script, script_path) = if let Some(script) = args.script {
        if args.eval_script {
            (Some(script), None)
        } else {
            (
                Some(fs::read_to_string(&script).expect("Unable to load script")),
                Some(script),
            )
        }
    } else if termion::is_tty(&stdin) || std::env::var_os("KOTO_FORCE_REPL_MODE").is_some() {
        // Forcing REPL mode is useful for testing the behaviour of the REPL
        (None, None)
    } else {
        let mut script = String::new();
        stdin
            .read_to_string(&mut script)
            .expect("Failed to read script from standard input");
        (Some(script), None)
    };

    if let Some(script) = script {
        let mut koto = Koto::with_settings(koto_settings);
        koto.set_script_path(script_path.map(|path| path.into()));

        let mut prelude = koto.prelude();
        prelude.add_map("json", koto_json::make_module());
        prelude.add_value("random", koto_random::make_module());
        prelude.add_map("tempfile", koto_tempfile::make_module());
        prelude.add_map("toml", koto_toml::make_module());
        prelude.add_map("yaml", koto_yaml::make_module());

        match koto.compile(&script) {
            Ok(chunk) => {
                if args.show_bytecode {
                    println!("{}\n", &Chunk::bytes_as_string(chunk.clone()));
                }
                if args.show_instructions {
                    println!("Constants\n---------\n{}\n", chunk.constants.to_string());

                    let script_lines = script.lines().collect::<Vec<_>>();
                    println!(
                        "Instructions\n------------\n{}",
                        Chunk::instructions_as_string(chunk, &script_lines)
                    );
                }
                match koto.run_with_args(&args.script_args) {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        return Err(());
                    }
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                return Err(());
            }
        }
    } else {
        let mut repl = Repl::with_settings(
            ReplSettings {
                show_instructions: args.show_instructions,
                show_bytecode: args.show_bytecode,
            },
            koto_settings,
        );
        repl.run();
    }

    Ok(())
}
