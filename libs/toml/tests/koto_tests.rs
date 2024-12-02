use koto_runtime::{prelude::*, Result};
use koto_test_utils::run_test_script;
use std::path::PathBuf;

#[test]
fn json_tests() -> Result<()> {
    let vm = KotoVm::default();
    vm.prelude().insert("toml", koto_toml::make_module());

    match vm.prelude().data_mut().get("koto") {
        Some(KValue::Map(m)) => m.insert(
            "script_dir",
            PathBuf::from_iter(&[env!("CARGO_MANIFEST_DIR"), "tests"])
                .to_string_lossy()
                .to_string(),
        ),
        _ => return runtime_error!("Missing koto module"),
    }

    run_test_script(vm, include_str!("toml.koto"), None)
}
