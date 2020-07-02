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
            if (a - b).abs() <= *allowed_diff {
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

    prelude.add_value("test", Map(test));
}
