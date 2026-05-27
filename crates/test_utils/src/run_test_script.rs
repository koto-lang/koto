use crate::script_instructions;
use koto_bytecode::{CompilerSettings, ModuleLoader};
use koto_runtime::{Result, prelude::*};

fn value_to_string(vm: &mut KotoVm, value: &KValue) -> String {
    match vm
        .value_to_string(value)
        .and_then(|output| output.into_task().block_on(vm))
        .unwrap()
    {
        KValue::Str(result) => result.as_str().to_owned(),
        unexpected => panic!("Expected String from @display, found {unexpected:?}"),
    }
}

/// Runs a script using the provided Vm, optionally checking its output
pub fn run_test_script(
    mut vm: KotoVm,
    script: &str,
    script_path: Option<KString>,
    expected_output: Option<KValue>,
) -> Result<()> {
    let mut loader = ModuleLoader::default();
    let chunk = match loader.compile_script(script, script_path, CompilerSettings::default()) {
        Ok(chunk) => chunk,
        Err(error) => {
            println!("{script}\n");
            return Err(format!("Error while compiling script: {error}").into());
        }
    };

    match vm
        .run(chunk)
        .and_then(|output| output.into_task().block_on(&vm))
    {
        Ok(result) => {
            if let Some(expected_output) = expected_output {
                match vm
                    .run_binary_op(BinaryOp::Equal, result.clone(), expected_output.clone())
                    .and_then(|output| output.into_task().block_on(&vm))
                {
                    Ok(KValue::Bool(true)) => {}
                    Ok(KValue::Bool(false)) => {
                        return Err(format!(
                            "{}\nUnexpected result - expected: {}, result: {}",
                            script_instructions(script, vm.chunk()),
                            value_to_string(&mut vm, &expected_output),
                            value_to_string(&mut vm, &result),
                        )
                        .into());
                    }
                    Ok(other) => {
                        return Err(format!(
                            "{}\nExpected bool from equality comparison, found '{}'",
                            script_instructions(script, vm.chunk()),
                            value_to_string(&mut vm, &other)
                        )
                        .into());
                    }
                    Err(e) => {
                        return Err(format!(
                            "{}\nError while comparing output value: ({e})",
                            script_instructions(script, vm.chunk()),
                        )
                        .into());
                    }
                }
            }

            match vm
                .run_tests(vm.exports().clone())
                .and_then(|output| output.into_task().block_on(&vm))
            {
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
