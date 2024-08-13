//! The `number` core library module

use crate::prelude::*;

/// Initializes the `number` core library module
pub fn make_module() -> KMap {
    use KValue::Number;

    let result = KMap::with_type("core.number");

    macro_rules! number_fn {
        ($fn:ident) => {
            number_fn!(stringify!($fn), $fn)
        };
        ($name:expr, $fn:ident) => {
            result.add_fn($name, |ctx| {
                let expected_error = "|Number|";

                match ctx.instance_and_args(is_number, expected_error)? {
                    (Number(n), []) => Ok(Number(n.$fn())),
                    (instance, args) => {
                        unexpected_args_after_instance(expected_error, instance, args)
                    }
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
                let expected_error = "|Number|";

                match ctx.instance_and_args(is_number, expected_error)? {
                    (Number(n), []) => Ok(Number(f64::from(n).$fn().into())),
                    (instance, args) => {
                        unexpected_args_after_instance(expected_error, instance, args)
                    }
                }
            });
        };
    }

    macro_rules! bitwise_fn {
        ($name:ident, $op:tt) => {
            result.add_fn(stringify!($name), |ctx| {
                let expected_error = "|Number, Number|";

                match ctx.instance_and_args(is_number, expected_error)? {
                    (Number(a), [Number(b)]) => Ok((i64::from(a) $op i64::from(b)).into()),
                    (instance, args) => {
                        unexpected_args_after_instance(expected_error, instance, args)
                    }
                }
            })
        };
    }

    macro_rules! bitwise_fn_positive_arg {
        ($name:ident, $op:tt) => {
            result.add_fn(stringify!($name), |ctx| {
                let expected_error = "|Number, Number|";

                match ctx.instance_and_args(is_number, expected_error)? {
                    (Number(a), [Number(b)]) if *b >= 0 => {
                        Ok((i64::from(a) $op i64::from(b)).into())
                    }
                    (instance, args) => {
                        unexpected_args_after_instance(expected_error, instance, args)
                    }
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
        let expected_error = "|Number, Number|";

        match ctx.instance_and_args(is_number, expected_error)? {
            (Number(y), [Number(x)]) => Ok(f64::from(y).atan2(f64::from(x)).into()),
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    number_fn!(ceil);

    result.add_fn("clamp", |ctx| {
        let expected_error = "|Number, Number, Number|";

        match ctx.instance_and_args(is_number, expected_error)? {
            (Number(x), [Number(a), Number(b)]) => Ok(Number(*a.max(b.min(x)))),
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    number_f64_fn!(cos);
    number_f64_fn!(cosh);
    number_f64_fn!("degrees", to_degrees);

    result.insert("e", std::f64::consts::E);

    number_f64_fn!(exp);
    number_f64_fn!(exp2);

    result.add_fn("flip_bits", |ctx| {
        let expected_error = "|Number|";

        match ctx.instance_and_args(is_number, expected_error)? {
            (Number(n), []) => Ok((!n.as_i64()).into()),
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    number_fn!(floor);

    result.insert("infinity", Number(f64::INFINITY.into()));

    result.add_fn("is_nan", |ctx| {
        let expected_error = "|Number|";

        match ctx.instance_and_args(is_number, expected_error)? {
            (Number(n), []) => Ok(n.is_nan().into()),
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("lerp", |ctx| {
        let expected_error = "|Number, Number, Number|";

        match ctx.instance_and_args(is_number, expected_error)? {
            (Number(a), [Number(b), Number(t)]) => {
                let result = *a + (b - a) * *t;
                Ok(result.into())
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    number_f64_fn!(ln);
    number_f64_fn!(log2);
    number_f64_fn!(log10);

    result.add_fn("max", |ctx| {
        let expected_error = "|Number, Number|";

        match ctx.instance_and_args(is_number, expected_error)? {
            (Number(a), [Number(b)]) => Ok(Number(*a.max(b))),
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("min", |ctx| {
        let expected_error = "|Number, Number|";

        match ctx.instance_and_args(is_number, expected_error)? {
            (Number(a), [Number(b)]) => Ok(Number(*a.min(b))),
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.insert("nan", f64::NAN);
    result.insert("negative_infinity", f64::NEG_INFINITY);

    bitwise_fn!(or, |);

    result.insert("pi", std::f64::consts::PI);
    result.insert("pi_2", std::f64::consts::FRAC_PI_2);
    result.insert("pi_4", std::f64::consts::FRAC_PI_4);

    result.add_fn("pow", |ctx| {
        let expected_error = "|Number, Number|";

        match ctx.instance_and_args(is_number, expected_error)? {
            (Number(a), [Number(b)]) => Ok(Number(a.pow(*b))),
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
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

    result.insert("tau", std::f64::consts::TAU);

    result.add_fn("to_int", |ctx| {
        let expected_error = "|Number|";

        match ctx.instance_and_args(is_number, expected_error)? {
            (Number(n), []) => Ok(i64::from(n).into()),
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    bitwise_fn!(xor, ^);

    result
}

fn is_number(value: &KValue) -> bool {
    matches!(value, KValue::Number(_))
}
