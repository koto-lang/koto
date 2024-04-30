use koto::prelude::*;
use std::{
    fs::read_to_string,
    path::{Path, PathBuf},
};

fn run_script(script: &str, path: &Path, should_fail_at_runtime: bool) {
    let mut koto = Koto::with_settings(KotoSettings {
        run_tests: true,
        ..Default::default()
    });
    koto.set_script_path(Some(path)).unwrap();

    let prelude = koto.prelude();
    prelude.insert("color", koto_color::make_module());
    prelude.insert("geometry", koto_geometry::make_module());
    prelude.insert("json", koto_json::make_module());
    prelude.insert("random", koto_random::make_module());
    prelude.insert("regex", koto_regex::make_module());
    prelude.insert("tempfile", koto_tempfile::make_module());
    prelude.insert("toml", koto_toml::make_module());
    prelude.insert("yaml", koto_yaml::make_module());

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
}

fn load_and_run_script(script_path: &str) {
    let mut path = PathBuf::new();
    path.push(env!("CARGO_MANIFEST_DIR"));
    path.push("../../koto/tests/libs");
    path.push(script_path);
    if !path.exists() {
        panic!("Path doesn't exist: {path:?}");
    }
    let script =
        read_to_string(&path).unwrap_or_else(|_| panic!("Unable to load path '{:?}'", &path));

    run_script(&script, &path, false);
}

macro_rules! lib_test {
    ($name:ident) => {
        #[test]
        fn $name() {
            load_and_run_script(&format!("{}.koto", stringify!($name)));
        }
    };
}

mod lib_tests {
    use super::*;

    lib_test!(color);
    lib_test!(geometry);
    lib_test!(json);
    lib_test!(random);
    lib_test!(regex);
    lib_test!(tempfile);
    lib_test!(toml);
    lib_test!(yaml);
}
