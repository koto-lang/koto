use crate::script_instructions;
use koto_bytecode::{CompilerSettings, Loader};
use koto_runtime::{prelude::*, Result};

/// Runs a script using the provided Vm, optionally checking its output
pub fn run_test_script(
    mut vm: KotoVm,
    script: &str,
    expected_output: Option<KValue>,
) -> Result<()> {
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
            if let Some(expected_output) = expected_output {
                match vm.run_binary_op(BinaryOp::Equal, result.clone(), expected_output.clone()) {
                    Ok(KValue::Bool(true)) => {}
                    Ok(KValue::Bool(false)) => {
                        return Err(format!(
                            "{}\nUnexpected result - expected: {}, result: {}",
                            script_instructions(script, vm.chunk()),
                            vm.value_to_string(&expected_output).unwrap(),
                            vm.value_to_string(&result).unwrap(),
                        )
                        .into())
                    }
                    Ok(other) => {
                        return Err(format!(
                            "{}\nExpected bool from equality comparison, found '{}'",
                            script_instructions(script, vm.chunk()),
                            vm.value_to_string(&other).unwrap()
                        )
                        .into())
                    }
                    Err(e) => {
                        return Err(format!(
                            "{}\nError while comparing output value: ({e})",
                            script_instructions(script, vm.chunk()),
                        )
                        .into())
                    }
                }
            }

            match vm.run_tests(vm.exports().clone()) {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("{}\n {e}", script_instructions(script, vm.chunk())).into()),
            }
        }

        Err(e) => Err(format!(
            "{}\nError while running script: {e}",
            script_instructions(script, vm.chunk())
        )
        .into()),
    }
}
