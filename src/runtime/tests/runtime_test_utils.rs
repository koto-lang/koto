#![allow(unused)]

use {
    koto_bytecode::Chunk,
    koto_runtime::{
        num2, num4, BinaryOp, Loader,
        Value::{self, *},
        ValueList, Vm,
    },
    std::rc::Rc,
};

pub fn test_script(script: &str, expected_output: Value) {
    test_script_with_vm(Vm::default(), script, expected_output);
}

pub fn test_script_with_vm(mut vm: Vm, script: &str, expected_output: Value) {
    let mut loader = Loader::default();
    let chunk = match loader.compile_script(script, &None) {
        Ok(chunk) => chunk,
        Err(error) => {
            print_chunk(script, vm.chunk());
            panic!("Error while compiling script: {}", error);
        }
    };

    match vm.run(chunk) {
        Ok(result) => {
            match vm.run_binary_op(BinaryOp::Equal, result.clone(), expected_output.clone()) {
                Ok(Value::Bool(true)) => {}
                Ok(Value::Bool(false)) => {
                    print_chunk(script, vm.chunk());
                    panic!(
                        "Unexpected result - expected: {}, result: {}",
                        expected_output, result
                    );
                }
                Ok(other) => {
                    print_chunk(script, vm.chunk());
                    panic!("Expected bool from equality comparison, found '{}'", other);
                }
                Err(e) => {
                    print_chunk(script, vm.chunk());
                    panic!("Error while comparing output value: {}", e.to_string());
                }
            }
        }
        Err(e) => {
            print_chunk(script, vm.chunk());
            panic!("Error while running script: {}", e.to_string());
        }
    }
}

pub fn print_chunk(script: &str, chunk: Rc<Chunk>) {
    println!("{}\n", script);
    let script_lines = script.lines().collect::<Vec<_>>();

    println!("Constants\n---------\n{}\n", chunk.constants.to_string());
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

pub fn num2(a: f64, b: f64) -> Value {
    Num2(num2::Num2(a, b))
}

pub fn num4(a: f32, b: f32, c: f32, d: f32) -> Value {
    Num4(num4::Num4(a, b, c, d))
}

pub fn string(s: &str) -> Value {
    Str(s.into())
}
