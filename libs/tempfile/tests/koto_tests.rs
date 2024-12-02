use koto_runtime::{prelude::*, Result};
use koto_test_utils::run_test_script;

#[test]
fn tempfile_tests() -> Result<()> {
    let vm = KotoVm::default();
    vm.prelude()
        .insert("tempfile", koto_tempfile::make_module());

    run_test_script(vm, include_str!("tempfile.koto"), None)
}
