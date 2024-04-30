use crate::{script_instructions, OutputCapture};
use koto_bytecode::{CompilerSettings, Loader};
use koto_runtime::{prelude::*, Result};

/// Runs a script and validates its output
pub fn check_script_output(script: &str, expected_output: impl Into<KValue>) {
    let (vm, output) = OutputCapture::make_vm_with_output_capture();

    if let Err(e) = check_script_output_with_vm(vm, script, expected_output) {
        let output = output.captured_output();
        if !output.is_empty() {
            println!("Stdout:\n-------\n\n{output}\n-------\n");
        }
        panic!("{e}");
    }
}

/// Runs a script and validates its output using a provided Vm
pub fn check_script_output_with_vm(
    mut vm: KotoVm,
    script: &str,
    expected_output: impl Into<KValue>,
) -> Result<()> {
    let expected_output = expected_output.into();

    let mut loader = Loader::default();
    let chunk = match loader.compile_script(script, None, CompilerSettings::default()) {
        Ok(chunk) => chunk,
        Err(error) => {
            println!("{script}\n");
            return Err(format!("Error while compiling script: {error}").into());
        }
    };

    match vm.run(chunk) {
        Ok(result) => {
            match vm.run_binary_op(BinaryOp::Equal, result.clone(), expected_output.clone()) {
                Ok(KValue::Bool(true)) => Ok(()),
                Ok(KValue::Bool(false)) => Err(format!(
                    "{}\nUnexpected result - expected: {}, result: {}",
                    script_instructions(script, vm.chunk()),
                    vm.value_to_string(&expected_output).unwrap(),
                    vm.value_to_string(&result).unwrap(),
                )
                .into()),
                Ok(other) => Err(format!(
                    "{}\nExpected bool from equality comparison, found '{}'",
                    script_instructions(script, vm.chunk()),
                    vm.value_to_string(&other).unwrap()
                )
                .into()),
                Err(e) => Err(format!(
                    "{}\nError while comparing output value: ({e})",
                    script_instructions(script, vm.chunk()),
                )
                .into()),
            }
        }
        Err(e) => Err(format!(
            "{}\nError while running script: {e}",
            script_instructions(script, vm.chunk())
        )
        .into()),
    }
}
