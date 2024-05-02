use koto::{prelude::*, PtrMut};
use std::{
    fs::read_to_string,
    path::{Path, PathBuf},
};

fn run_script(script: &str, script_path: &Path, expected_module_paths: &[PathBuf]) {
    let loaded_module_paths = PtrMut::from(vec![]);

    let mut koto = Koto::with_settings(
        KotoSettings {
            run_tests: true,
            ..Default::default()
        }
        .with_module_imported_callback({
            let loaded_module_paths = loaded_module_paths.clone();
            move |path: &Path| loaded_module_paths.borrow_mut().push(path.to_path_buf())
        }),
    );
    koto.set_script_path(Some(script_path)).unwrap();

    match koto.compile(script) {
        Ok(_) => match koto.run() {
            Ok(_) => {
                for loaded_module_path in loaded_module_paths.borrow().iter() {
                    if !expected_module_paths
                        .iter()
                        .any(|path| path == loaded_module_path)
                    {
                        panic!(
                            "Unexpected imported module: '{}'",
                            loaded_module_path.to_string_lossy()
                        );
                    }
                }
                // Check that the loaded module paths are correct
                assert_eq!(
                    loaded_module_paths.borrow().len(),
                    expected_module_paths.len(),
                    "Mismatch in number of imported module paths"
                );
            }
            Err(error) => {
                panic!("{error}");
            }
        },
        Err(error) => {
            panic!("{error}");
        }
    }
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
        panic!("Path doesn't exist: {script_path:?}");
    }
    let script = read_to_string(&script_path)
        .unwrap_or_else(|_| panic!("Unable to load path '{script_path:?}'"));

    let expected_module_paths = imported_modules
        .iter()
        .map(|path| {
            let mut imported_path = test_folder.clone();
            imported_path.push(path);
            imported_path
        })
        .collect::<Vec<_>>();

    run_script(&script, &script_path, &expected_module_paths);
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

    koto_test!(assignment);
    koto_test!(comments);
    koto_test!(control_flow);
    koto_test!(enums);
    koto_test!(eval);
    koto_test!(function_closures);
    koto_test!(functions);
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
    koto_test!(os);
    koto_test!(primes);
    koto_test!(ranges);
    koto_test!(strings);
    koto_test!(tests);
    koto_test!(tuples);
    koto_test!(types);

    koto_test!(error_handling, "error_handling_module/main.koto");
    koto_test!(import, "test_module/baz.koto", "test_module/main.koto");
}
