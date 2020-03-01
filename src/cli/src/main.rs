use clap::{App, Arg};
use koto::{Error, Parser, Runtime};
use std::{fs, io::Write};

fn main() {
    let matches = App::new("koto")
        .version("1.0")
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

    let parser = Parser::new();
    let mut runtime = Runtime::new();

    if let Some(script_args) = matches
        .values_of("args")
        .map(|args| args.collect::<Vec<_>>())
    {
        runtime.set_args(&script_args);
    }

    if let Some(path) = matches.value_of("script") {
        let script = fs::read_to_string(path).expect("Unable to load path");
        match parser.parse(&script) {
            Ok(ast) => match runtime.run(&ast) {
                Ok(_) => {}
                Err(e) => match e {
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
        let mut input = String::new();
        loop {
            print!("> ");
            std::io::stdout().flush().expect("Error flushing output");
            std::io::stdin()
                .read_line(&mut input)
                .expect("Error getting input");
            match parser.parse(&input) {
                Ok(ast) => match runtime.run(&ast) {
                    Ok(result) => println!("{}", result),
                    Err(Error::RuntimeError { message, .. }) => println!("Error: {}", message),
                },
                Err(e) => {
                    println!("Error parsing input: {}", e);
                }
            }
            input.clear();
        }
    }
}
