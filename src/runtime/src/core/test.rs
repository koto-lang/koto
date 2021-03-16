use crate::{external_error, BinaryOp, Value, ValueMap, ValueNumber};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("assert", |vm, args| {
        for value in vm.get_args(args).iter() {
            match value {
                Bool(b) => {
                    if !b {
                        return external_error!("Assertion failed");
                    }
                }
                unexpected => {
                    return external_error!(
                        "assert expects booleans as arguments, found '{}'",
                        unexpected.type_as_string(),
                    )
                }
            }
        }
        Ok(Empty)
    });

    result.add_fn("assert_eq", |vm, args| match vm.get_args(args) {
        [a, b] => {
            let a = a.clone();
            let b = b.clone();
            let result = vm
                .child_vm()
                .run_binary_op(BinaryOp::Equal, a.clone(), b.clone());
            match result {
                Ok(Bool(true)) => Ok(Empty),
                Ok(Bool(false)) => {
                    external_error!("Assertion failed, '{}' is not equal to '{}'", a, b)
                }
                Ok(unexpected) => external_error!(
                    "assert_eq: expected Bool from comparison, found '{}'",
                    unexpected.type_as_string()
                ),
                Err(e) => Err(e.with_prefix("assert_eq")),
            }
        }
        _ => external_error!("assert_eq expects two arguments"),
    });

    result.add_fn("assert_ne", |vm, args| match vm.get_args(args) {
        [a, b] => {
            let a = a.clone();
            let b = b.clone();
            let result = vm
                .child_vm()
                .run_binary_op(BinaryOp::NotEqual, a.clone(), b.clone());
            match result {
                Ok(Bool(true)) => Ok(Empty),
                Ok(Bool(false)) => {
                    external_error!("Assertion failed, '{}' should not be equal to '{}'", a, b)
                }
                Ok(unexpected) => external_error!(
                    "assert_ne: expected Bool from comparison, found '{}'",
                    unexpected.type_as_string()
                ),
                Err(e) => Err(e.with_prefix("assert_ne")),
            }
        }
        _ => external_error!("assert_ne expects two arguments"),
    });

    result.add_fn("assert_near", |vm, args| match vm.get_args(args) {
        [Number(a), Number(b), Number(allowed_diff)] => {
            if number_near(*a, *b, *allowed_diff) {
                Ok(Empty)
            } else {
                external_error!(
                    "Assertion failed, '{}' and '{}' are not within {} of each other",
                    a,
                    b,
                    allowed_diff,
                )
            }
        }
        [Num2(a), Num2(b), Number(allowed_diff)] => {
            let allowed_diff: f64 = allowed_diff.into();
            if f64_near(a.0, b.0, allowed_diff) && f64_near(a.1, b.1, allowed_diff) {
                Ok(Empty)
            } else {
                external_error!(
                    "Assertion failed, '{}' and '{}' are not within {} of each other",
                    a,
                    b,
                    allowed_diff,
                )
            }
        }
        [Num4(a), Num4(b), Number(allowed_diff)] => {
            let allowed_diff: f32 = allowed_diff.into();
            if f32_near(a.0, b.0, allowed_diff)
                && f32_near(a.1, b.1, allowed_diff)
                && f32_near(a.2, b.2, allowed_diff)
                && f32_near(a.3, b.3, allowed_diff)
            {
                Ok(Empty)
            } else {
                external_error!(
                    "Assertion failed, '{}' and '{}' are not within {} of each other",
                    a,
                    b,
                    allowed_diff,
                )
            }
        }
        [a, b, c] => external_error!(
            "assert_near expects Numbers as arguments, found '{}', '{}', and '{}'",
            a.type_as_string(),
            b.type_as_string(),
            c.type_as_string(),
        ),
        _ => external_error!("assert_eq expects three arguments"),
    });

    result.add_fn("run_tests", |vm, args| match vm.get_args(args) {
        [Map(tests)] => {
            let tests = tests.clone();
            vm.run_tests(tests)
        }
        _ => external_error!("run_tests expects a map as argument"),
    });

    result
}

fn f32_near(a: f32, b: f32, allowed_diff: f32) -> bool {
    (a - b).abs() <= allowed_diff
}

fn f64_near(a: f64, b: f64, allowed_diff: f64) -> bool {
    (a - b).abs() <= allowed_diff
}

fn number_near(a: ValueNumber, b: ValueNumber, allowed_diff: ValueNumber) -> bool {
    (a - b).abs() <= allowed_diff
}
