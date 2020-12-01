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
        fs,
        path::{Path, PathBuf},
        str::FromStr,
        time::Duration,
    },
};

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

fn main() {
    let args = match parse_arguments() {
        Ok(args) => {
            if args.help {
                println!("{}", help_string());
                return;
            }
            if args.version {
                println!("{}", version_string());
                return;
            }
            args
        }
        Err(error) => {
            println!("{}\n\n{}", help_string(), error);
            return;
        }
    };

    let mut koto = Koto::with_settings(KotoSettings {
        run_tests: true,
        ..Default::default()
    });

    koto.context_mut()
        .prelude
        .add_map("poetry", koto_bindings::make_module());

    let script_path = PathBuf::from_str(&args.script).expect("Failed to parse script path");
    koto.set_script_path(Some(script_path.clone()));

    compile_and_run(&mut koto, &script_path);

    if args.watch {
        let mut hotwatch = Hotwatch::new_with_custom_delay(Duration::from_secs_f64(0.25))
            .expect("Failed to initialize file watcher");
        hotwatch
            .watch(&args.script, move |event: Event| {
                // dbg!(&event);
                match event {
                    Event::Create(script_path) | Event::Write(script_path) => {
                        compile_and_run(&mut koto, &script_path);
                    }
                    _ => {}
                }
                Flow::Continue
            })
            .expect("failed to watch file!");
        hotwatch.run();
    }
}

fn compile_and_run(koto: &mut Koto, script_path: &Path) {
    let script = fs::read_to_string(script_path).expect("Unable to load script");
    match koto.compile(&script) {
        Ok(_) => match koto.run() {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error while running script: {}", e);
            }
        },
        Err(e) => {
            eprintln!(
                "Error while compiling script: {}",
                koto.format_loader_error(e, &script)
            );
        }
    }
}
