use koto::Koto;
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

    let mut koto = Koto::with_settings(koto::Settings {
        run_tests: true,
        ..Default::default()
    });
    koto.set_script_path(Some(path));

    let prelude = koto.prelude_mut();
    koto_json::register(prelude);
    koto_toml::register(prelude);

    match koto.compile(&script) {
        Ok(_) => {
            if let Err(error) = koto.run() {
                panic!(error);
            }
        }
        Err(error) => {
            panic!(error);
        }
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

mod koto_tests {
    use super::*;

    koto_test!(arithmetic);
    koto_test!(assignment);
    koto_test!(comments);
    koto_test!(control_flow);
    koto_test!(error_handling);
    koto_test!(function_closures);
    koto_test!(functions);
    koto_test!(functions_in_lookups);
    koto_test!(generators);
    koto_test!(import);
    koto_test!(io);
    koto_test!(iterators);
    koto_test!(json);
    koto_test!(line_breaks);
    koto_test!(list_ops);
    koto_test!(lists);
    koto_test!(logic);
    koto_test!(loops);
    koto_test!(map_ops);
    koto_test!(maps);
    koto_test!(maps_and_lists);
    koto_test!(math);
    koto_test!(num2_4);
    koto_test!(primes);
    koto_test!(random);
    koto_test!(ranges);
    koto_test!(strings);
    koto_test!(threads);
    koto_test!(toml);
    koto_test!(types);
}
