//! The `number` core library module

use crate::prelude::*;

/// Initializes the `number` core library module
pub fn make_module() -> ValueMap {
    use Value::*;

    let result = ValueMap::with_type("core.number");

    macro_rules! number_fn {
        ($fn:ident) => {
            number_fn!(stringify!($fn), $fn)
        };
        ($name:expr, $fn:ident) => {
            result.add_fn($name, |ctx| {
                let expected_error = "a Number";

                match ctx.instance_and_args(is_number, expected_error)? {
                    (Number(n), []) => Ok(Number(n.$fn())),
                    (_, unexpected) => type_error_with_slice(expected_error, unexpected),
                }
            });
        };
    }

    macro_rules! number_f64_fn {
        ($fn:ident) => {
            number_f64_fn!(stringify!($fn), $fn)
        };
        ($name:expr, $fn:ident) => {
            result.add_fn($name, |ctx| {
                let expected_error = "a Number";

                match ctx.instance_and_args(is_number, expected_error)? {
                    (Number(n), []) => Ok(Number(f64::from(n).$fn().into())),
                    (_, unexpected) => type_error_with_slice(expected_error, unexpected),
                }
            });
        };
    }

    macro_rules! bitwise_fn {
        ($name:ident, $op:tt) => {
            result.add_fn(stringify!($name), |ctx| {
                use ValueNumber::I64;
                let expected_error = "two Integers";

                match ctx.instance_and_args(is_integer, expected_error)? {
                    (Number(I64(a)), [Number(I64(b))]) => Ok(Number((a $op b).into())),
                    (_, unexpected) => type_error_with_slice(expected_error, unexpected),
                }
            })
        };
    }

    macro_rules! bitwise_fn_positive_arg {
        ($name:ident, $op:tt) => {
            result.add_fn(stringify!($name), |ctx| {
                use ValueNumber::I64;

                let expected_error = "two Integers (with non-negative second Integer)";

                match ctx.instance_and_args(is_integer, expected_error)? {
                    (Number(I64(a)), [Number(I64(b))]) if *b >= 0 => Ok(Number((a $op b).into())),
                    (_, unexpected) => type_error_with_slice(expected_error, unexpected),
                }
            })
        };
    }

    number_fn!(abs);
    number_f64_fn!(acos);
    number_f64_fn!(acosh);
    bitwise_fn!(and, &);
    number_f64_fn!(asin);
    number_f64_fn!(asinh);
    number_f64_fn!(atan);
    number_f64_fn!(atanh);

    result.add_fn("atan2", |ctx| {
        let expected_error = "two Numbers";

        match ctx.instance_and_args(is_number, expected_error)? {
            (Number(y), [Number(x)]) => {
                let result = f64::from(y).atan2(f64::from(x));
                Ok(Number(result.into()))
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    number_fn!(ceil);

    result.add_fn("clamp", |ctx| {
        let expected_error = "three Numbers";

        match ctx.instance_and_args(is_number, expected_error)? {
            (Number(x), [Number(a), Number(b)]) => Ok(Number(*a.max(b.min(x)))),
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    number_f64_fn!(cos);
    number_f64_fn!(cosh);
    number_f64_fn!("degrees", to_degrees);

    result.add_value("e", Number(std::f64::consts::E.into()));

    number_f64_fn!(exp);
    number_f64_fn!(exp2);

    result.add_fn("flip_bits", |ctx| {
        let expected_error = "an Integer";

        match ctx.instance_and_args(is_integer, expected_error)? {
            (Number(ValueNumber::I64(n)), []) => Ok(Number((!n).into())),
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    number_fn!(floor);

    result.add_value("infinity", Number(std::f64::INFINITY.into()));

    result.add_fn("is_nan", |ctx| {
        let expected_error = "a Number";

        match ctx.instance_and_args(is_number, expected_error)? {
            (Number(n), []) => Ok(n.is_nan().into()),
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("lerp", |ctx| {
        let expected_error = "three Numbers";

        match ctx.instance_and_args(is_number, expected_error)? {
            (Number(a), [Number(b), Number(t)]) => {
                let result = *a + (b - a) * *t;
                Ok(result.into())
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    number_f64_fn!(ln);
    number_f64_fn!(log2);
    number_f64_fn!(log10);

    result.add_fn("max", |ctx| {
        let expected_error = "two Numbers";

        match ctx.instance_and_args(is_number, expected_error)? {
            (Number(a), [Number(b)]) => Ok(Number(*a.max(b))),
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("min", |ctx| {
        let expected_error = "two Numbers";

        match ctx.instance_and_args(is_number, expected_error)? {
            (Number(a), [Number(b)]) => Ok(Number(*a.min(b))),
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_value("nan", Number(std::f64::NAN.into()));
    result.add_value("negative_infinity", Number(std::f64::NEG_INFINITY.into()));

    bitwise_fn!(or, |);

    result.add_value("pi", Number(std::f64::consts::PI.into()));
    result.add_value("pi_2", Number(std::f64::consts::FRAC_PI_2.into()));
    result.add_value("pi_4", Number(std::f64::consts::FRAC_PI_4.into()));

    result.add_fn("pow", |ctx| {
        let expected_error = "two Numbers";

        match ctx.instance_and_args(is_number, expected_error)? {
            (Number(a), [Number(b)]) => Ok(Number(a.pow(*b))),
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    number_f64_fn!("radians", to_radians);
    number_f64_fn!(recip);
    number_fn!(round);

    bitwise_fn_positive_arg!(shift_left, <<);
    bitwise_fn_positive_arg!(shift_right, >>);

    number_f64_fn!(sin);
    number_f64_fn!(sinh);
    number_f64_fn!(sqrt);
    number_f64_fn!(tan);
    number_f64_fn!(tanh);

    result.add_value("tau", Number(std::f64::consts::TAU.into()));

    result.add_fn("to_float", |ctx| {
        let expected_error = "a Number";

        match ctx.instance_and_args(is_number, expected_error)? {
            (Number(n), []) => Ok(Number(f64::from(n).into())),
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("to_int", |ctx| {
        let expected_error = "a Number";

        match ctx.instance_and_args(is_number, expected_error)? {
            (Number(n), []) => Ok(Number(i64::from(n).into())),
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    bitwise_fn!(xor, ^);

    result
}

fn is_number(value: &Value) -> bool {
    matches!(value, Value::Number(_))
}

fn is_integer(value: &Value) -> bool {
    matches!(value, Value::Number(ValueNumber::I64(_)))
}
