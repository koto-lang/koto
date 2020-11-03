use crate::{external_error, type_as_string, Value, ValueMap};

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
                        type_as_string(unexpected),
                    )
                }
            }
        }
        Ok(Empty)
    });

    result.add_fn("assert_eq", |vm, args| match vm.get_args(args) {
        [a, b] => {
            if a == b {
                Ok(Empty)
            } else {
                external_error!("Assertion failed, '{}' is not equal to '{}'", a, b,)
            }
        }
        _ => external_error!("assert_eq expects two arguments"),
    });

    result.add_fn("assert_ne", |vm, args| match vm.get_args(args) {
        [a, b] => {
            if a != b {
                Ok(Empty)
            } else {
                external_error!("Assertion failed, '{}' should not be equal to '{}'", a, b,)
            }
        }
        _ => external_error!("assert_ne expects two arguments"),
    });

    result.add_fn("assert_near", |vm, args| match vm.get_args(args) {
        [Number(a), Number(b), Number(allowed_diff)] => {
            if f64_near(*a, *b, *allowed_diff) {
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
            if f64_near(a.0, b.0, *allowed_diff) && f64_near(a.1, b.1, *allowed_diff) {
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
            let allowed_diff = *allowed_diff as f32;
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
            type_as_string(&a),
            type_as_string(&b),
            type_as_string(&c),
        ),
        _ => external_error!("assert_eq expects three arguments"),
    });

    result.add_fn("run_tests", |vm, args| {
        let args = vm.get_args_as_vec(args);
        match args.as_slice() {
            [Map(tests)] => vm.run_tests(tests.clone()),
            _ => external_error!("run_tests expects a map as argument"),
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
