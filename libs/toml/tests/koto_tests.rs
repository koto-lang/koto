use koto_runtime::prelude::*;
use koto_test_utils::run_test_script;
use std::{error::Error, fs, path::PathBuf};

#[test]
fn json_tests() -> Result<(), Box<dyn Error>> {
    let vm = KotoVm::default();
    vm.prelude().insert("toml", koto_toml::make_module());

    let script_path = PathBuf::from_iter(&[env!("CARGO_MANIFEST_DIR"), "tests", "toml.koto"]);
    let script = fs::read_to_string(&script_path)?;

    run_test_script(vm, &script, Some(script_path), None)?;

    Ok(())
}
