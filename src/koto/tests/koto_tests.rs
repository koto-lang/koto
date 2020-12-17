use {
    koto::{Koto, KotoSettings},
    std::{fs::read_to_string, path::PathBuf},
};

fn run_script(script: &str, path: Option<PathBuf>, should_fail_at_runtime: bool) {
    let mut koto = Koto::with_settings(KotoSettings {
        run_tests: true,
        ..Default::default()
    });
    koto.set_script_path(path);

    match koto.compile(&script) {
        Ok(_) => match koto.run() {
            Ok(_) => {
                if should_fail_at_runtime {
                    panic!("Expected failure");
                }
            }
            Err(error) => {
                if !should_fail_at_runtime {
                    panic!(error);
                }
            }
        },
        Err(error) => {
            panic!("{}", koto.format_loader_error(error, &script));
        }
    }
}

fn load_and_run_script(script_path: &str) {
    let mut path = PathBuf::new();
    path.push(env!("CARGO_MANIFEST_DIR"));
    path.push("../../koto/tests");
    path.push(script_path);
    if !path.exists() {
        panic!(format!("Path doesn't exist: {:?}", path));
    }
    let script =
        read_to_string(&path).unwrap_or_else(|_| panic!("Unable to load path '{:?}'", &path));

    run_script(&script, Some(path), false);
}

macro_rules! koto_test {
    ($name:ident) => {
        #[test]
        fn $name() {
            load_and_run_script(&format!("{}.koto", stringify!($name)));
        }
    };
}

mod koto_tests {
    use super::*;

    #[test]
    fn check_assert() {
        let script = "
import test.assert
test.assert false
";
        run_script(script, None, true);
    }

    #[test]
    fn check_assert_eq() {
        let script = "
import test.assert_eq
assert_eq 0 1
";
        run_script(script, None, true);
    }

    #[test]
    fn check_assert_ne() {
        let script = "
import test.assert_ne
assert_ne 1 1
";
        run_script(script, None, true);
    }

    #[test]
    fn check_assert_near() {
        let script = "
import test.assert_near
assert_near 1 2 0.1
";
        run_script(script, None, true);
    }

    koto_test!(arithmetic);
    koto_test!(assignment);
    koto_test!(comments);
    koto_test!(control_flow);
    koto_test!(enums);
    koto_test!(error_handling);
    koto_test!(function_closures);
    koto_test!(functions);
    koto_test!(functions_in_lookups);
    koto_test!(import);
    koto_test!(io);
    koto_test!(iterators);
    koto_test!(line_breaks);
    koto_test!(list_ops);
    koto_test!(lists);
    koto_test!(logic);
    koto_test!(loops);
    koto_test!(map_ops);
    koto_test!(maps);
    koto_test!(maps_and_lists);
    koto_test!(os);
    koto_test!(numbers);
    koto_test!(num2_4);
    koto_test!(primes);
    koto_test!(ranges);
    koto_test!(strings);
    koto_test!(tests);
    koto_test!(threads);
    koto_test!(tuples);
    koto_test!(types);
}
