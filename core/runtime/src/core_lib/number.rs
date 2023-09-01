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
            result.add_fn($name, |vm, args| match vm.get_args(args) {
                [Number(n)] => Ok(Number(n.$fn())),
                unexpected => type_error_with_slice("a Number as argument", unexpected),
            });
        };
    }

    macro_rules! number_f64_fn {
        ($fn:ident) => {
            number_f64_fn!(stringify!($fn), $fn)
        };
        ($name:expr, $fn:ident) => {
            result.add_fn($name, |vm, args| match vm.get_args(args) {
                [Number(n)] => Ok(Number(f64::from(n).$fn().into())),
                unexpected => type_error_with_slice("a Number as argument", unexpected),
            })
        };
    }

    macro_rules! bitwise_fn {
        ($name:ident, $op:tt) => {
            result.add_fn(stringify!($name), |vm, args| {
                use ValueNumber::I64;
                match vm.get_args(args) {
                    [Number(I64(a)), Number(I64(b))] => Ok(Number((a $op b).into())),
                    unexpected => type_error_with_slice(
                        "two Integers as arguments",
                        unexpected,
                    ),
                }
            })
        };
    }

    macro_rules! bitwise_fn_positive_arg {
        ($name:ident, $op:tt) => {
            result.add_fn(stringify!($name), |vm, args| {
                use ValueNumber::I64;
                match vm.get_args(args) {
                    [Number(I64(a)), Number(I64(b))] if *b >= 0 => Ok(Number((a $op b).into())),
                    unexpected => type_error_with_slice(
                        "two Integers (with non-negative second Integer) as arguments",
                        unexpected,
                    ),
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

    result.add_fn("atan2", |vm, args| match vm.get_args(args) {
        [Number(y), Number(x)] => {
            let result = f64::from(y).atan2(f64::from(x));
            Ok(Number(result.into()))
        }
        unexpected => type_error_with_slice("two Numbers as arguments", unexpected),
    });

    number_fn!(ceil);

    result.add_fn("clamp", |vm, args| match vm.get_args(args) {
        [Number(x), Number(a), Number(b)] => Ok(Number(*a.max(b.min(x)))),
        unexpected => type_error_with_slice("three Numbers as arguments", unexpected),
    });

    number_f64_fn!(cos);
    number_f64_fn!(cosh);
    number_f64_fn!("degrees", to_degrees);

    result.add_value("e", Number(std::f64::consts::E.into()));

    number_f64_fn!(exp);
    number_f64_fn!(exp2);

    result.add_fn("flip_bits", |vm, args| match vm.get_args(args) {
        [Number(ValueNumber::I64(n))] => Ok(Number((!n).into())),
        unexpected => type_error_with_slice("an Integer as argument", unexpected),
    });

    number_fn!(floor);

    result.add_value("infinity", Number(std::f64::INFINITY.into()));

    result.add_fn("is_nan", |vm, args| match vm.get_args(args) {
        [Number(n)] => Ok(n.is_nan().into()),
        unexpected => type_error_with_slice("a Number as argument", unexpected),
    });

    result.add_fn("lerp", |vm, args| match vm.get_args(args) {
        [Number(a), Number(b), Number(t)] => {
            let result = *a + (b - a) * *t;
            Ok(result.into())
        }
        unexpected => type_error_with_slice("two Numbers as arguments", unexpected),
    });

    number_f64_fn!(ln);
    number_f64_fn!(log2);
    number_f64_fn!(log10);

    result.add_fn("max", |vm, args| match vm.get_args(args) {
        [Number(a), Number(b)] => Ok(Number(*a.max(b))),
        unexpected => type_error_with_slice("two Numbers as arguments", unexpected),
    });

    result.add_fn("min", |vm, args| match vm.get_args(args) {
        [Number(a), Number(b)] => Ok(Number(*a.min(b))),
        unexpected => type_error_with_slice("two Numbers as arguments", unexpected),
    });

    result.add_value("nan", Number(std::f64::NAN.into()));
    result.add_value("negative_infinity", Number(std::f64::NEG_INFINITY.into()));

    bitwise_fn!(or, |);

    result.add_value("pi", Number(std::f64::consts::PI.into()));
    result.add_value("pi_2", Number(std::f64::consts::FRAC_PI_2.into()));
    result.add_value("pi_4", Number(std::f64::consts::FRAC_PI_4.into()));

    result.add_fn("pow", |vm, args| match vm.get_args(args) {
        [Number(a), Number(b)] => Ok(Number(a.pow(*b))),
        unexpected => type_error_with_slice("two Numbers as arguments", unexpected),
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

    result.add_fn("to_float", |vm, args| match vm.get_args(args) {
        [Number(n)] => Ok(Number(f64::from(n).into())),
        unexpected => type_error_with_slice("a Number as argument", unexpected),
    });

    result.add_fn("to_int", |vm, args| match vm.get_args(args) {
        [Number(n)] => Ok(Number(i64::from(n).into())),
        unexpected => type_error_with_slice("a Number as argument", unexpected),
    });

    bitwise_fn!(xor, ^);

    result
}
