#![allow(unused)]

use {
    koto_bytecode::{Chunk, CompilerSettings, Loader},
    koto_runtime::{prelude::*, Value::*},
    std::{cell::RefCell, rc::Rc},
};

pub fn test_script(script: &str, expected_output: impl Into<Value>) {
    let output = PtrMut::from(String::new());

    let vm = Vm::with_settings(VmSettings {
        stdout: Rc::new(TestStdout {
            output: output.clone(),
        }),
        stderr: Rc::new(TestStdout {
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

pub fn run_script_with_vm(mut vm: Vm, script: &str, expected_output: Value) -> Result<(), String> {
    let mut loader = Loader::default();
    let chunk = match loader.compile_script(script, &None, CompilerSettings::default()) {
        Ok(chunk) => chunk,
        Err(error) => {
            print_chunk(script, vm.chunk());
            return Err(format!("Error while compiling script: {error}"));
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
                    ))
                }
                Ok(other) => {
                    print_chunk(script, vm.chunk());
                    Err(format!(
                        "Expected bool from equality comparison, found '{}'",
                        vm.value_to_string(&other).unwrap()
                    ))
                }
                Err(e) => {
                    print_chunk(script, vm.chunk());
                    Err(format!("Error while comparing output value: {e}"))
                }
            }
        }
        Err(e) => {
            print_chunk(script, vm.chunk());
            Err(format!("Error while running script: {e}"))
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
    Number(f64::from(value).into())
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
    value_list(&values)
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
    value_tuple(&values)
}

pub fn value_list(values: &[Value]) -> Value {
    List(ValueList::from_slice(values))
}

pub fn value_tuple(values: &[Value]) -> Value {
    Tuple(values.into())
}

pub fn string(s: &str) -> Value {
    Str(s.into())
}

#[derive(Debug)]
pub struct TestStdout {
    pub output: PtrMut<String>,
}

impl KotoFile for TestStdout {
    fn id(&self) -> ValueString {
        "_teststdout_".into()
    }
}

impl KotoRead for TestStdout {}
impl KotoWrite for TestStdout {
    fn write(&self, bytes: &[u8]) -> Result<(), RuntimeError> {
        self.output
            .borrow_mut()
            .push_str(std::str::from_utf8(bytes).unwrap());
        Ok(())
    }

    fn write_line(&self, s: &str) -> Result<(), RuntimeError> {
        self.output.borrow_mut().push_str(s);
        self.output.borrow_mut().push('\n');
        Ok(())
    }

    fn flush(&self) -> Result<(), RuntimeError> {
        Ok(())
    }
}
