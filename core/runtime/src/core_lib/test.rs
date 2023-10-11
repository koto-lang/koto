//! The `test` core library module

use crate::{prelude::*, Result};

/// Initializes the `test` core library module
pub fn make_module() -> KMap {
    let result = KMap::with_type("core.test");

    result.add_fn("assert", |ctx| {
        for value in ctx.args().iter() {
            match value {
                Value::Bool(b) => {
                    if !b {
                        return runtime_error!("Assertion failed");
                    }
                }
                unexpected => return type_error("Bool as argument", unexpected),
            }
        }
        Ok(Value::Null)
    });

    result.add_fn("assert_eq", |ctx| match ctx.args() {
        [a, b] => {
            let a = a.clone();
            let b = b.clone();
            let result = ctx.vm.run_binary_op(BinaryOp::Equal, a.clone(), b.clone());
            match result {
                Ok(Value::Bool(true)) => Ok(Value::Null),
                Ok(Value::Bool(false)) => {
                    runtime_error!(
                        "Assertion failed, '{}' is not equal to '{}'",
                        ctx.vm.value_to_string(&a)?,
                        ctx.vm.value_to_string(&b)?,
                    )
                }
                Ok(unexpected) => type_error("Bool from equality comparison", &unexpected),
                Err(e) => Err(e),
            }
        }
        unexpected => type_error_with_slice("two Values", unexpected),
    });

    result.add_fn("assert_ne", |ctx| match ctx.args() {
        [a, b] => {
            let a = a.clone();
            let b = b.clone();
            let result = ctx
                .vm
                .run_binary_op(BinaryOp::NotEqual, a.clone(), b.clone());
            match result {
                Ok(Value::Bool(true)) => Ok(Value::Null),
                Ok(Value::Bool(false)) => {
                    runtime_error!(
                        "Assertion failed, '{}' should not be equal to '{}'",
                        ctx.vm.value_to_string(&a)?,
                        ctx.vm.value_to_string(&b)?
                    )
                }
                Ok(unexpected) => type_error("Bool from equality comparison", &unexpected),
                Err(e) => Err(e),
            }
        }
        unexpected => type_error_with_slice("two Values", unexpected),
    });

    result.add_fn("assert_near", |ctx| match ctx.args() {
        [Value::Number(a), Value::Number(b)] => number_near(*a, *b, 1.0e-12),
        [Value::Number(a), Value::Number(b), Value::Number(allowed_diff)] => {
            number_near(*a, *b, allowed_diff.into())
        }
        unexpected => type_error_with_slice(
            "two Numbers as arguments, \
             followed by an optional Number that specifies the allowed difference",
            unexpected,
        ),
    });

    result.add_fn("run_tests", |ctx| match ctx.args() {
        [Value::Map(tests)] => {
            let tests = tests.clone();
            ctx.vm.run_tests(tests)
        }
        unexpected => type_error_with_slice("a Map as argument", unexpected),
    });

    result
}

fn f64_near(a: f64, b: f64, allowed_diff: f64) -> bool {
    (a - b).abs() <= allowed_diff
}

fn number_near(a: KNumber, b: KNumber, allowed_diff: f64) -> Result<Value> {
    if f64_near(a.into(), b.into(), allowed_diff) {
        Ok(Value::Null)
    } else {
        runtime_error!(
            "Assertion failed, '{a}' and '{b}' are not within {allowed_diff} of each other"
        )
    }
}
