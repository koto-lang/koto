mod koto_bindings;
mod poetry;

use {
    hotwatch::{
        blocking::{Flow, Hotwatch},
        Event,
    },
    koto::{Koto, KotoSettings},
    poetry::*,
    std::{
        error::Error,
        fmt, fs,
        path::{Path, PathBuf},
        str::FromStr,
        time::Duration,
    },
};

#[derive(Debug)]
struct PoetryError {
    prefix: String,
    error: Box<dyn Error>,
}

impl fmt::Display for PoetryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.prefix, self.error)
    }
}

impl Error for PoetryError {}

fn version_string() -> String {
    format!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
}

fn help_string() -> String {
    format!(
        "{version}

Generate poetry with Koto

USAGE:
    {name} [FLAGS]

FLAGS:
    -s, --script             The script to run
    -w, --watch              Watch the script file for changes
    -h, --help               Prints help information
    -v, --version            Prints version information
",
        name = env!("CARGO_PKG_NAME"),
        version = version_string()
    )
}

struct PoetryArgs {
    help: bool,
    version: bool,
    script: String,
    watch: bool,
}

fn parse_arguments() -> Result<PoetryArgs, String> {
    let mut args = pico_args::Arguments::from_env();

    let help = args.contains(["-h", "--help"]);
    let version = args.contains(["-v", "--version"]);
    let watch = args.contains(["-w", "--watch"]);
    let script = args
        .value_from_str(["-s", "--script"])
        .map_err(|_| "Missing script argument".to_string())?;

    Ok(PoetryArgs {
        help,
        version,
        script,
        watch,
    })
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = match parse_arguments() {
        Ok(args) => {
            if args.help {
                println!("{}", help_string());
                return Ok(());
            }
            if args.version {
                println!("{}", version_string());
                return Ok(());
            }
            args
        }
        Err(error) => {
            println!("{}\n\n{}", help_string(), error);
            return Err("Failed to parse arguments".to_string().into());
        }
    };

    let mut koto = Koto::with_settings(KotoSettings {
        run_tests: true,
        ..Default::default()
    });

    koto.prelude()
        .add_map("poetry", koto_bindings::make_module());
    koto.prelude().add_map("random", koto_random::make_module());

    let script_path = PathBuf::from_str(&args.script).expect("Failed to parse script path");
    koto.set_script_path(Some(script_path.clone()));

    if args.watch {
        if let Err(e) = compile_and_run(&mut koto, &script_path) {
            eprintln!("{}", e);
        }

        let mut hotwatch = Hotwatch::new_with_custom_delay(Duration::from_secs_f64(0.25))
            .expect("Failed to initialize file watcher");
        hotwatch
            .watch(&args.script, move |event: Event| {
                match event {
                    Event::Create(script_path) | Event::Write(script_path) => {
                        if let Err(error) = compile_and_run(&mut koto, &script_path) {
                            eprintln!("{error}");
                        }
                    }
                    _ => {}
                }
                Flow::Continue
            })
            .expect("failed to watch file!");
        hotwatch.run();
        Ok(())
    } else {
        match compile_and_run(&mut koto, &script_path) {
            Ok(_) => Ok(()),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
    }
}

fn compile_and_run(koto: &mut Koto, script_path: &Path) -> Result<(), Box<dyn Error>> {
    let script = fs::read_to_string(script_path)?;
    match koto.compile(&script) {
        Ok(_) => match koto.run() {
            Ok(_) => Ok(()),
            Err(e) => Err(PoetryError {
                prefix: "Error while running script".into(),
                error: e.into(),
            }
            .into()),
        },
        Err(e) => Err(PoetryError {
            prefix: "Error while compiling script".into(),
            error: e.into(),
        }
        .into()),
    }
}
