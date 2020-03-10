use koto::{Error, Parser, Runtime};
use std::{fs::read_to_string, path::PathBuf};

fn run_script(script_path: &str) {
    let mut path = PathBuf::new();
    path.push(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path.push(script_path);
    if !path.exists() {
        panic!(format!("Path doesn't exist: {:?}", path));
    }
    let script = read_to_string(&path).expect(&format!("Unable to load path '{:?}'", &path));

    match Parser::new().parse(&script) {
        Ok(ast) => {
            let mut runtime = Runtime::new();
            runtime.environment_mut().script_path = Some(path.to_str().unwrap().to_string());
            runtime.setup_environment();
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
koto_test!(files);
koto_test!(functions);
koto_test!(lists);
koto_test!(loops);
koto_test!(maps);
koto_test!(maps_and_lists);
koto_test!(math);
koto_test!(ranges);
koto_test!(references);
koto_test!(strings);
koto_test!(vec4_new);
koto_test!(vec4_ops);
