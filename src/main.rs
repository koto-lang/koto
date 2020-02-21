use clap::{App, Arg};
use std::{fs, io::Write};

fn main() {
    let matches = App::new("ks")
        .version("1.0")
        .arg(
            Arg::with_name("script")
                .help("The ks script to run")
                .index(1),
        )
        .get_matches();

    let parser = holz::MyParser::new();

    if let Some(path) = matches.value_of("script") {
        let script = fs::read_to_string(path).expect("Unable to load path");
        match parser.parse(&script) {
            Ok(ast) => {
                let mut runtime = holz::Runtime::new();
                match runtime.run(&ast) {
                    Ok(_) => {}
                    Err(e) => match e {
                        holz::Error::RuntimeError {
                            message,
                            start_pos,
                            end_pos,
                        } => {
                            let excerpt = script
                                .lines()
                                .skip(start_pos.line - 1)
                                .take(end_pos.line - start_pos.line + 1)
                                .map(|line| format!("  | {}", line))
                                .collect::<String>();
                            eprintln!(
                                "Runtime error: {}\n  --> {}:{}\n  |\n{}\n  |",
                                message, start_pos.line, start_pos.column, excerpt
                            )
                        }
                    },
                }
            }
            Err(e) => eprintln!("Error while parsing source: {}", e),
        }
    } else {
        let mut runtime = holz::Runtime::new();
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
                    Err(holz::Error::RuntimeError { message, .. }) => {
                        println!("Error: {}", message)
                    }
                },
                Err(e) => {
                    println!("Error parsing input: {}", e);
                }
            }
            input.clear();
        }
    }
}
