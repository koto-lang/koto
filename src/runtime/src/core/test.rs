use crate::{
    num2, num4, runtime_error, unexpected_type_error_with_slice, BinaryOp, RuntimeResult, Value,
    ValueMap, ValueNumber,
};

pub fn make_module() -> ValueMap {
    use Value::*;

    let result = ValueMap::new();

    result.add_fn("assert", |vm, args| {
        for value in vm.get_args(args).iter() {
            match value {
                Bool(b) => {
                    if !b {
                        return runtime_error!("Assertion failed");
                    }
                }
                unexpected => {
                    return unexpected_type_error_with_slice(
                        "test.assert",
                        "Bool as argument",
                        &[unexpected.clone()],
                    )
                }
            }
        }
        Ok(Null)
    });

    result.add_fn("assert_eq", |vm, args| match vm.get_args(args) {
        [a, b] => {
            let a = a.clone();
            let b = b.clone();
            let result = vm.run_binary_op(BinaryOp::Equal, a.clone(), b.clone());
            match result {
                Ok(Bool(true)) => Ok(Null),
                Ok(Bool(false)) => {
                    runtime_error!("Assertion failed, '{a}' is not equal to '{b}'")
                }
                Ok(unexpected) => unexpected_type_error_with_slice(
                    "test.assert_eq",
                    "Bool from equality comparison",
                    &[unexpected],
                ),
                Err(e) => Err(e.with_prefix("assert_eq")),
            }
        }
        unexpected => unexpected_type_error_with_slice("test.assert_eq", "two Values", unexpected),
    });

    result.add_fn("assert_ne", |vm, args| match vm.get_args(args) {
        [a, b] => {
            let a = a.clone();
            let b = b.clone();
            let result = vm.run_binary_op(BinaryOp::NotEqual, a.clone(), b.clone());
            match result {
                Ok(Bool(true)) => Ok(Null),
                Ok(Bool(false)) => {
                    runtime_error!("Assertion failed, '{a}' should not be equal to '{b}'")
                }
                Ok(unexpected) => unexpected_type_error_with_slice(
                    "test.assert_ne",
                    "Bool from equality comparison",
                    &[unexpected],
                ),
                Err(e) => Err(e.with_prefix("assert_ne")),
            }
        }
        unexpected => unexpected_type_error_with_slice("test.assert_ne", "two Values", unexpected),
    });

    result.add_fn("assert_near", |vm, args| match vm.get_args(args) {
        [Number(a), Number(b)] => number_near(*a, *b, 1.0e-12),
        [Number(a), Number(b), Number(allowed_diff)] => number_near(*a, *b, allowed_diff.into()),
        [Num2(a), Num2(b)] => num2_near(*a, *b, 1.0e-12),
        [Num2(a), Num2(b), Number(allowed_diff)] => num2_near(*a, *b, allowed_diff.into()),
        [Num4(a), Num4(b)] => num4_near(*a, *b, 1.0e-6),
        [Num4(a), Num4(b), Number(allowed_diff)] => num4_near(*a, *b, allowed_diff.into()),
        unexpected => unexpected_type_error_with_slice(
            "test.assert_near",
            "two Numbers (or Num2s or Num4s) as arguments, \
             followed by an optional Number that specifies the allowed difference",
            unexpected,
        ),
    });

    result.add_fn("run_tests", |vm, args| match vm.get_args(args) {
        [Map(tests)] => {
            let tests = tests.clone();
            let mut vm = vm.spawn_shared_vm(); // TODO is spawning a VM still necessary?
            vm.run_tests(tests)
        }
        unexpected => {
            unexpected_type_error_with_slice("test.run_tests", "a Map as argument", unexpected)
        }
    });

    result
}

fn f32_near(a: f32, b: f32, allowed_diff: f32) -> bool {
    (a - b).abs() <= allowed_diff
}

fn f64_near(a: f64, b: f64, allowed_diff: f64) -> bool {
    (a - b).abs() <= allowed_diff
}

fn number_near(a: ValueNumber, b: ValueNumber, allowed_diff: f64) -> RuntimeResult {
    if f64_near(a.into(), b.into(), allowed_diff) {
        Ok(Value::Null)
    } else {
        runtime_error!(
            "Assertion failed, '{a}' and '{b}' are not within {allowed_diff} of each other"
        )
    }
}

fn num2_near(a: num2::Num2, b: num2::Num2, allowed_diff: f64) -> RuntimeResult {
    if f64_near(a.0, b.0, allowed_diff) && f64_near(a.1, b.1, allowed_diff) {
        Ok(Value::Null)
    } else {
        runtime_error!(
            "Assertion failed, '{a}' and '{b}' are not within {allowed_diff} of each other"
        )
    }
}

fn num4_near(a: num4::Num4, b: num4::Num4, allowed_diff: f32) -> RuntimeResult {
    if f32_near(a.0, b.0, allowed_diff)
        && f32_near(a.1, b.1, allowed_diff)
        && f32_near(a.2, b.2, allowed_diff)
        && f32_near(a.3, b.3, allowed_diff)
    {
        Ok(Value::Null)
    } else {
        runtime_error!(
            "Assertion failed, '{a}' and '{b}' are not within {allowed_diff} of each other"
        )
    }
}
