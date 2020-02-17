use clap::{App, Arg};
use std::fs;

fn main() {
    let matches = App::new("ks")
        .version("1.0")
        .arg(
            Arg::with_name("script")
                .help("The ks script to run")
                .index(1),
        )
        .get_matches();

    if let Some(path) = matches.value_of("script") {
        let script = fs::read_to_string(path).expect("Unable to load path");
        match holz::parse(&script) {
            Ok(ast) => {
                // println!("{:?}\n", ast);
                let mut runtime = holz::Runtime::new();
                match runtime.run(&ast) {
                    Ok(_) => {}
                    Err(e) => eprintln!("{}", e),
                }
            }
            Err(e) => eprintln!("Error while parsing source: {}", e),
        }
    }
}
