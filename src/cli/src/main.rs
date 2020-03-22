use clap::{App, Arg};
use koto::Koto;
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
        let mut koto = Koto::new();

        let args = match matches.values_of("args") {
            Some(args) => args.map(|s| s.to_string()).collect::<Vec<_>>(),
            None => Vec::new(),
        };

        let script = fs::read_to_string(path).expect("Unable to load path");
        match koto.run_script_with_args(&script, args) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("{}", e);
            }
        }
    } else {
        let mut repl = Repl::new();
        repl.run();
    }
}
