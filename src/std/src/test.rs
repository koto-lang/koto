use koto_runtime::{external_error, type_as_string, Value, ValueMap};

pub fn register(prelude: &mut ValueMap) {
    use Value::*;

    let mut test = ValueMap::new();

    test.add_fn("assert", |_, args| {
        for value in args.iter() {
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

    test.add_fn("assert_eq", |_, args| match &args {
        [a, b] => {
            if a == b {
                Ok(Empty)
            } else {
                external_error!(
                    "Assertion failed, '{}' is not equal to '{}'",
                    args[0],
                    args[1],
                )
            }
        }
        _ => external_error!("assert_eq expects two arguments, found {}", args.len()),
    });

    test.add_fn("assert_ne", |_, args| match &args {
        [a, b] => {
            if a != b {
                Ok(Empty)
            } else {
                external_error!(
                    "Assertion failed, '{}' should not be equal to '{}'",
                    args[0],
                    args[1],
                )
            }
        }
        _ => external_error!("assert_ne expects two arguments, found {}", args.len()),
    });

    test.add_fn("assert_near", |_, args| match &args {
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
        _ => external_error!("assert_eq expects three arguments, found {}", args.len()),
    });

    test.add_fn("run_tests", |runtime, args| match &args {
        [Map(tests)] => runtime.run_tests(tests.clone()),
        _ => external_error!("run_tests expects a map as argument"),
    });

    prelude.add_value("test", Map(test));
}

fn f32_near(a: f32, b: f32, allowed_diff: f32) -> bool {
    (a - b).abs() <= allowed_diff
}

fn f64_near(a: f64, b: f64, allowed_diff: f64) -> bool {
    (a - b).abs() <= allowed_diff
}
