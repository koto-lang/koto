use koto::{Error, Parser, Runtime};
use std::{env::current_dir, fs::read_to_string};

fn run_script(script_path: &str) {
    let mut path = current_dir().unwrap().canonicalize().unwrap();
    path.push("tests");
    path.push(script_path);
    let script = read_to_string(path).expect("Unable to load path");

    match Parser::new().parse(&script) {
        Ok(ast) => {
            let mut runtime = Runtime::new();
            match runtime.run(&ast) {
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
                            .map(|line| format!("  | {}", line))
                            .collect::<String>();
                        eprintln!(
                            "Runtime error: {}\n  --> {}:{}\n  |\n{}\n  |",
                            message, start_pos.line, start_pos.column, excerpt
                        );
                        assert!(false);
                    }
                },
            }
        }
        Err(e) => assert!(false, format!("Parsing error:\n{}", e)),
    }
}

macro_rules! koto_test {
    ($name:ident) => {
        #[test]
        fn $name() {
            run_script(&format!("{}.koto", stringify!($name)));
        }
    };
}

koto_test!(arithmetic);
koto_test!(assignment);
koto_test!(comments);
koto_test!(control_flow);
koto_test!(functions);
koto_test!(lists);
koto_test!(loops);
koto_test!(maps);
koto_test!(ranges);
koto_test!(strings);
koto_test!(vec4_new);
koto_test!(vec4_ops);
