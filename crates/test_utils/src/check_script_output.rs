use crate::{run_test_script, OutputCapture};
use koto_runtime::{prelude::*, Result};

/// Runs a script and validates its output
pub fn check_script_output(script: &str, expected_output: impl Into<KValue>) {
    let (vm, output) = OutputCapture::make_vm_with_output_capture();

    if let Err(e) = run_test_script(vm, script, None, Some(expected_output.into())) {
        let output = output.captured_output();
        if !output.is_empty() {
            println!("Stdout:\n-------\n\n{output}\n-------\n");
        }
        panic!("{e}");
    }
}

/// Runs a script and validates its output using a provided Vm
pub fn check_script_output_with_vm(
    vm: KotoVm,
    script: &str,
    expected_output: impl Into<KValue>,
) -> Result<()> {
    run_test_script(vm, script, None, Some(expected_output.into()))
}
