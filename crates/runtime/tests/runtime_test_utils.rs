#![allow(unused)]

use koto_bytecode::{Chunk, CompilerSettings, Loader};
use koto_runtime::{prelude::*, Result, Value::*};
use std::{cell::RefCell, rc::Rc};

pub fn test_script(script: &str, expected_output: impl Into<Value>) {
    let output = PtrMut::from(String::new());

    let vm = Vm::with_settings(VmSettings {
        stdout: make_ptr!(TestStdout {
            output: output.clone(),
        }),
        stderr: make_ptr!(TestStdout {
            output: output.clone(),
        }),
        ..Default::default()
    });

    if let Err(e) = run_script_with_vm(vm, script, expected_output.into()) {
        let output = output.borrow();
        if !output.is_empty() {
            println!("Stdout:\n-------\n\n{output}\n-------\n");
        }
        panic!("{e}");
    }
}

pub fn run_script_with_vm(mut vm: Vm, script: &str, expected_output: Value) -> Result<()> {
    let mut loader = Loader::default();
    let chunk = match loader.compile_script(script, &None, CompilerSettings::default()) {
        Ok(chunk) => chunk,
        Err(error) => {
            println!("{script}\n");
            return Err(format!("Error while compiling script: {error}").into());
        }
    };

    match vm.run(chunk) {
        Ok(result) => {
            match vm.run_binary_op(BinaryOp::Equal, result.clone(), expected_output.clone()) {
                Ok(Value::Bool(true)) => Ok(()),
                Ok(Value::Bool(false)) => {
                    print_chunk(script, vm.chunk());
                    Err(format!(
                        "Unexpected result - expected: {}, result: {}",
                        vm.value_to_string(&expected_output).unwrap(),
                        vm.value_to_string(&result).unwrap(),
                    )
                    .into())
                }
                Ok(other) => {
                    print_chunk(script, vm.chunk());
                    Err(format!(
                        "Expected bool from equality comparison, found '{}'",
                        vm.value_to_string(&other).unwrap()
                    )
                    .into())
                }
                Err(e) => {
                    print_chunk(script, vm.chunk());
                    Err(format!("Error while comparing output value: ({e})").into())
                }
            }
        }
        Err(e) => {
            print_chunk(script, vm.chunk());
            Err(format!("Error while running script: {e}").into())
        }
    }
}

pub fn print_chunk(script: &str, chunk: Ptr<Chunk>) {
    println!("{script}\n");
    let script_lines = script.lines().collect::<Vec<_>>();

    println!("Constants\n---------\n{}\n", chunk.constants);
    println!(
        "Instructions\n------------\n{}",
        Chunk::instructions_as_string(chunk, &script_lines)
    );
}

pub fn number<T>(value: T) -> Value
where
    T: Copy,
    f64: From<T>,
{
    f64::from(value).into()
}

pub fn number_list<T>(values: &[T]) -> Value
where
    T: Copy,
    i64: From<T>,
{
    let values = values
        .iter()
        .map(|n| Number(i64::from(*n).into()))
        .collect::<Vec<_>>();
    list(&values)
}

pub fn number_tuple<T>(values: &[T]) -> Value
where
    T: Copy,
    i64: From<T>,
{
    let values = values
        .iter()
        .map(|n| Number(i64::from(*n).into()))
        .collect::<Vec<_>>();
    tuple(&values)
}

pub fn list(values: &[Value]) -> Value {
    KList::from_slice(values).into()
}

pub fn tuple(values: &[Value]) -> Value {
    KTuple::from(values).into()
}

pub fn string(s: &str) -> Value {
    KString::from(s).into()
}

#[derive(Debug)]
pub struct TestStdout {
    pub output: PtrMut<String>,
}

impl KotoFile for TestStdout {
    fn id(&self) -> KString {
        "_teststdout_".into()
    }
}

impl KotoRead for TestStdout {}
impl KotoWrite for TestStdout {
    fn write(&self, bytes: &[u8]) -> Result<()> {
        self.output
            .borrow_mut()
            .push_str(std::str::from_utf8(bytes).unwrap());
        Ok(())
    }

    fn write_line(&self, s: &str) -> Result<()> {
        self.output.borrow_mut().push_str(s);
        self.output.borrow_mut().push('\n');
        Ok(())
    }

    fn flush(&self) -> Result<()> {
        Ok(())
    }
}
