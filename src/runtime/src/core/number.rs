use crate::{external_error, type_as_string, Value, ValueMap};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    macro_rules! number_fn_1 {
        ($fn:ident) => {
            number_fn_1!(stringify!($fn), $fn)
        };
        ($name:expr, $fn:ident) => {
            result.add_fn($name, |vm, args| match vm.get_args(args) {
                [Number(n)] => Ok(Number(n.$fn())),
                [other] => external_error!(
                    "number.{} expects a Number as argument, found {}",
                    $name,
                    type_as_string(other),
                ),
                _ => external_error!("number.{} expects a Number as argument", $name),
            })
        };
    }

    number_fn_1!(abs);
    number_fn_1!(acos);
    number_fn_1!(asin);
    number_fn_1!(atan);
    number_fn_1!(ceil);

    result.add_fn("clamp", |vm, args| match vm.get_args(args) {
        [Number(x), Number(a), Number(b)] => Ok(Number(a.max(b.min(*x)))),
        _ => external_error!("number.clamp: Expected three numbers as arguments"),
    });

    number_fn_1!(cos);
    number_fn_1!(cosh);
    number_fn_1!("degrees", to_degrees);
    number_fn_1!(exp);
    number_fn_1!(exp2);
    number_fn_1!(floor);
    number_fn_1!(log10);
    number_fn_1!(log2);
    number_fn_1!(ln);

    result.add_fn("max", |vm, args| match vm.get_args(args) {
        [Number(a), Number(b)] => Ok(Number(a.max(*b))),
        _ => external_error!("number.max: Expected two numbers as arguments"),
    });

    result.add_fn("min", |vm, args| match vm.get_args(args) {
        [Number(a), Number(b)] => Ok(Number(a.min(*b))),
        _ => external_error!("number.min: Expected two numbers as arguments"),
    });

    result.add_value("pi", Number(std::f64::consts::PI));
    result.add_fn("pow", |vm, args| match vm.get_args(args) {
        [Number(a), Number(b)] => Ok(Number(a.powf(*b))),
        _ => external_error!("number.pow: Expected two numbers as arguments"),
    });

    number_fn_1!("radians", to_radians);
    number_fn_1!(recip);
    number_fn_1!(sin);
    number_fn_1!(sinh);
    number_fn_1!(sqrt);

    result.add_fn("sum", |vm, args| match vm.get_args(args) {
        [] => external_error!("sum: Missing argument"),
        [Num2(n)] => Ok(Number(n[0] + n[1])),
        [Num4(n)] => Ok(Number((n[0] + n[1] + n[2] + n[3]) as f64)),
        [unexpected] => external_error!(
            "number.sum: Expected Num2 or Num4, found '{}'",
            type_as_string(unexpected)
        ),
        _ => external_error!("number.sum: Expected a single Num2 or Num4 argument"),
    });

    number_fn_1!(tan);
    number_fn_1!(tanh);

    result.add_value("tau", Number(std::f64::consts::TAU));

    result
}
