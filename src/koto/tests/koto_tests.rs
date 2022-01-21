use {
    koto::{Koto, KotoSettings},
    std::{fs::read_to_string, path::PathBuf},
};

fn run_script(
    script: &str,
    script_path: Option<PathBuf>,
    expected_module_paths: &[PathBuf],
    should_fail_at_runtime: bool,
) {
    let mut koto = Koto::with_settings(KotoSettings {
        run_tests: true,
        ..Default::default()
    });
    koto.set_script_path(script_path);

    match koto.compile(script) {
        Ok(_) => match koto.run() {
            Ok(_) => {
                if should_fail_at_runtime {
                    panic!("Expected failure");
                }
            }
            Err(error) => {
                if !should_fail_at_runtime {
                    panic!("{}", error);
                }
            }
        },
        Err(error) => {
            panic!("{}", error);
        }
    }

    // Check that the loaded module paths are correct
    let mut loaded_module_count = 0;
    koto.for_each_module_path(|path| {
        if !expected_module_paths
            .iter()
            .any(|module_path| module_path == path)
        {
            panic!("Not in expected paths: '{}'", path.to_string_lossy());
        }
        loaded_module_count += 1;
    });
    assert_eq!(loaded_module_count, expected_module_paths.len());
}

fn load_and_run_script(script_file_name: &str, imported_modules: &[&str]) {
    let mut test_folder = PathBuf::new();
    test_folder.push(env!("CARGO_MANIFEST_DIR"));
    test_folder.push("..");
    test_folder.push("..");
    test_folder.push("koto");
    test_folder.push("tests");
    test_folder = test_folder.canonicalize().unwrap();

    let mut script_path = test_folder.clone();
    script_path.push(script_file_name);
    if !script_path.exists() {
        panic!("Path doesn't exist: {:?}", script_path);
    }
    let script = read_to_string(&script_path)
        .unwrap_or_else(|_| panic!("Unable to load path '{:?}'", &script_path));

    let expected_module_paths = imported_modules
        .iter()
        .map(|path| {
            let mut imported_path = test_folder.clone();
            imported_path.push(path);
            imported_path
        })
        .collect::<Vec<_>>();

    run_script(&script, Some(script_path), &expected_module_paths, false);
}

macro_rules! koto_test {
    ($name:ident) => {
        #[test]
        fn $name() {
            load_and_run_script(&format!("{}.koto", stringify!($name)), &[]);
        }
    };

    ($name:ident, $($imported_modules:literal),+) => {
        #[test]
        fn $name() {
            load_and_run_script(&format!("{}.koto", stringify!($name)), &[$($imported_modules), +]);
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
        run_script(script, None, &[], true);
    }

    #[test]
    fn check_assert_eq() {
        let script = "
import test.assert_eq
assert_eq 0, 1
";
        run_script(script, None, &[], true);
    }

    #[test]
    fn check_assert_ne() {
        let script = "
import test.assert_ne
assert_ne 1, 1
";
        run_script(script, None, &[], true);
    }

    #[test]
    fn check_assert_near() {
        let script = "
import test.assert_near
assert_near 1, 2, 0.1
";
        run_script(script, None, &[], true);
    }

    koto_test!(assignment);
    koto_test!(comments);
    koto_test!(control_flow);
    koto_test!(enums);
    koto_test!(function_closures);
    koto_test!(functions);
    koto_test!(functions_in_lookups);
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
    koto_test!(meta_maps);
    koto_test!(number_ops);
    koto_test!(numbers);
    koto_test!(num2_4);
    koto_test!(os);
    koto_test!(primes);
    koto_test!(ranges);
    koto_test!(strings);
    koto_test!(string_formatting);
    koto_test!(tests);
    koto_test!(tuples);
    koto_test!(types);

    koto_test!(error_handling, "error_handling_module/main.koto");
    koto_test!(import, "test_module/baz.koto", "test_module/main.koto");
}
