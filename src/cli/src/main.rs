use clap::{App, Arg};
use koto::{Error, Parser, Koto};
use std::fs;

mod repl;
use repl::Repl;

fn main() {
    let matches = App::new("Koto")
        .version(env!("CARGO_PKG_VERSION"))
        .arg(
            Arg::with_name("script")
                .help("The koto script to run")
                .index(1),
        )
        .arg(
            Arg::with_name("args")
                .help("Arguments to pass into koto")
                .multiple(true)
                .last(true),
        )
        .get_matches();

    if let Some(path) = matches.value_of("script") {
        let parser = Parser::new();
        let mut koto = Koto::new();

        if let Some(script_args) = matches
            .values_of("args")
            .map(|args| args.map(|s| s.to_string()).collect::<Vec<_>>())
        {
            koto.environment_mut().args = script_args;
        }

        koto.setup_environment();

        let script = fs::read_to_string(path).expect("Unable to load path");
        match parser.parse(&script) {
            Ok(ast) => match koto.run(&ast) {
                Ok(_) => {}
                Err(e) => match e {
                    Error::BuiltinError { message } => {
                        eprintln!("Builtin error: {}\n", message,);
                        panic!();
                    }
                    Error::RuntimeError {
                        message,
                        start_pos,
                        end_pos,
                    } => {
                        let excerpt = script
                            .lines()
                            .skip(start_pos.line - 1)
                            .take(end_pos.line - start_pos.line + 1)
                            .map(|line| format!("  | {}\n", line))
                            .collect::<String>();
                        eprintln!(
                            "Runtime error: {}\n  --> {}:{}\n  |\n{}  |",
                            message, start_pos.line, start_pos.column, excerpt
                        )
                    }
                },
            },
            Err(e) => eprintln!("Error while parsing source: {}", e),
        }
    } else {
        let mut repl = Repl::new();
        repl.run();
    }
}
