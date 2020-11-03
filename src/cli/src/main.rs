use clap::{App, Arg};
use koto::Koto;
use std::{fs, path::Path};

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

mod repl;
use repl::Repl;

fn main() {
    let matches = App::new("Koto")
        .version(env!("CARGO_PKG_VERSION"))
        .arg(Arg::with_name("script").about("The koto script to run"))
        .arg(
            Arg::with_name("tests")
                .short('t')
                .long("tests")
                .about("Run the script's tests"),
        )
        .arg(
            Arg::with_name("show_bytecode")
                .short('b')
                .long("show_bytecode")
                .about("Show the script's compiled bytecode"),
        )
        .arg(
            Arg::with_name("show_annotated")
                .short('B')
                .long("show_annotated")
                .about("Show compiled bytecode annotated with source lines"),
        )
        .arg(
            Arg::with_name("args")
                .about("Arguments to pass into the script")
                .multiple(true)
                .last(true),
        )
        .get_matches();

    let mut settings = koto::Settings::default();
    settings.run_tests = matches.is_present("tests");
    settings.show_bytecode = matches.is_present("show_bytecode");
    settings.show_annotated = matches.is_present("show_annotated");

    if let Some(path) = matches.value_of("script") {
        let mut koto = Koto::with_settings(settings);

        let mut prelude = koto.context().prelude.clone();
        koto_json::register(&mut prelude);
        koto_toml::register(&mut prelude);

        let args = match matches.values_of("args") {
            Some(args) => args.map(|s| s.to_string()).collect::<Vec<_>>(),
            None => Vec::new(),
        };

        let script = fs::read_to_string(path).expect("Unable to load path");
        koto.set_script_path(Some(Path::new(path).to_path_buf()));
        match koto.compile(&script) {
            Ok(_) => match koto.run_with_args(&args) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("{}", e);
                }
            },
            Err(e) => {
                eprintln!("{}", koto.format_loader_error(e, &script));
            }
        }
    } else {
        let mut repl = Repl::with_settings(settings);
        repl.run();
    }
}
