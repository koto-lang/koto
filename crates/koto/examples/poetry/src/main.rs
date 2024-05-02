mod koto_bindings;
mod poetry;

use anyhow::{bail, Context, Result};
use hotwatch::{
    blocking::{Flow, Hotwatch},
    Event,
};
use koto::{Koto, KotoSettings};
use poetry::*;
use std::{fs, path::Path, time::Duration};

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

fn parse_arguments() -> Result<PoetryArgs> {
    let mut args = pico_args::Arguments::from_env();

    let help = args.contains(["-h", "--help"]);
    let version = args.contains(["-v", "--version"]);
    let watch = args.contains(["-w", "--watch"]);
    let script = args
        .value_from_str(["-s", "--script"])
        .context("Missing script argument")?;

    Ok(PoetryArgs {
        help,
        version,
        script,
        watch,
    })
}

fn main() -> Result<()> {
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
            bail!("Failed to parse arguments");
        }
    };

    let mut koto = Koto::with_settings(KotoSettings {
        run_tests: true,
        ..Default::default()
    });

    koto.prelude()
        .insert("poetry", koto_bindings::make_module());
    koto.prelude().insert("random", koto_random::make_module());

    let script_path = Path::new(&args.script);
    koto.set_script_path(Some(script_path))
        .expect("Failed to set script path");

    if args.watch {
        if let Err(e) = compile_and_run(&mut koto, script_path) {
            eprintln!("{e}");
        }

        let mut hotwatch = Hotwatch::new_with_custom_delay(Duration::from_secs_f64(0.25))
            .context("Failed to initialize file watcher")?;
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
            .context("Failed to watch file!")?;
        hotwatch.run();
        Ok(())
    } else {
        compile_and_run(&mut koto, script_path)
    }
}

fn compile_and_run(koto: &mut Koto, script_path: &Path) -> Result<()> {
    let script = fs::read_to_string(script_path)?;
    koto.compile(&script)
        .context("Error while compiling script")?;
    koto.run().context("Error while running script")?;
    Ok(())
}
