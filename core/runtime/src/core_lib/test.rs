//! The `test` core library module

use crate::{prelude::*, Result};

/// Initializes the `test` core library module
pub fn make_module() -> ValueMap {
    use Value::*;

    let result = ValueMap::with_type("core.test");

    result.add_fn("assert", |vm, args| {
        for value in vm.get_args(args).iter() {
            match value {
                Bool(b) => {
                    if !b {
                        return runtime_error!("Assertion failed");
                    }
                }
                unexpected => return type_error("Bool as argument", unexpected),
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
                    runtime_error!(
                        "Assertion failed, '{}' is not equal to '{}'",
                        vm.value_to_string(&a)?,
                        vm.value_to_string(&b)?,
                    )
                }
                Ok(unexpected) => type_error("Bool from equality comparison", &unexpected),
                Err(e) => Err(e),
            }
        }
        unexpected => type_error_with_slice("two Values", unexpected),
    });

    result.add_fn("assert_ne", |vm, args| match vm.get_args(args) {
        [a, b] => {
            let a = a.clone();
            let b = b.clone();
            let result = vm.run_binary_op(BinaryOp::NotEqual, a.clone(), b.clone());
            match result {
                Ok(Bool(true)) => Ok(Null),
                Ok(Bool(false)) => {
                    runtime_error!(
                        "Assertion failed, '{}' should not be equal to '{}'",
                        vm.value_to_string(&a)?,
                        vm.value_to_string(&b)?
                    )
                }
                Ok(unexpected) => type_error("Bool from equality comparison", &unexpected),
                Err(e) => Err(e),
            }
        }
        unexpected => type_error_with_slice("two Values", unexpected),
    });

    result.add_fn("assert_near", |vm, args| match vm.get_args(args) {
        [Number(a), Number(b)] => number_near(*a, *b, 1.0e-12),
        [Number(a), Number(b), Number(allowed_diff)] => number_near(*a, *b, allowed_diff.into()),
        unexpected => type_error_with_slice(
            "two Numbers as arguments, \
             followed by an optional Number that specifies the allowed difference",
            unexpected,
        ),
    });

    result.add_fn("run_tests", |vm, args| match vm.get_args(args) {
        [Map(tests)] => {
            let tests = tests.clone();
            vm.run_tests(tests)
        }
        unexpected => type_error_with_slice("a Map as argument", unexpected),
    });

    result
}

fn f64_near(a: f64, b: f64, allowed_diff: f64) -> bool {
    (a - b).abs() <= allowed_diff
}

fn number_near(a: ValueNumber, b: ValueNumber, allowed_diff: f64) -> Result<Value> {
    if f64_near(a.into(), b.into(), allowed_diff) {
        Ok(Value::Null)
    } else {
        runtime_error!(
            "Assertion failed, '{a}' and '{b}' are not within {allowed_diff} of each other"
        )
    }
}
