use crate::single_arg_fn;
use koto_runtime::{external_error, type_as_string, value, Value, ValueMap};

pub fn register(prelude: &mut ValueMap) {
    use Value::*;

    let mut math = ValueMap::new();

    macro_rules! math_fn_1 {
        ($fn:ident) => {
            math_fn_1!(stringify!($fn), $fn)
        };
        ($name:expr, $fn:ident) => {
            single_arg_fn!(math, $name, Number, n, { Ok(Number(n.$fn())) });
        };
    }

    math_fn_1!(abs);
    math_fn_1!(acos);
    math_fn_1!(asin);
    math_fn_1!(atan);
    math_fn_1!(ceil);

    math.add_fn("clamp", |vm, args| match vm.get_args(args) {
        [Number(x), Number(a), Number(b)] => Ok(Number(a.max(b.min(*x)))),
        _ => external_error!("math.clamp: Expected three numbers as arguments"),
    });

    math_fn_1!(cos);
    math_fn_1!(cosh);
    math_fn_1!("degrees", to_degrees);
    math_fn_1!(exp);
    math_fn_1!(exp2);
    math_fn_1!(floor);
    math_fn_1!(log10);
    math_fn_1!(log2);
    math_fn_1!(ln);

    math.add_fn("max", |vm, args| match vm.get_args(args) {
        [Number(a), Number(b)] => Ok(Number(a.max(*b))),
        _ => external_error!("math.max: Expected two numbers as arguments"),
    });

    math.add_fn("min", |vm, args| match vm.get_args(args) {
        [Number(a), Number(b)] => Ok(Number(a.min(*b))),
        _ => external_error!("math.min: Expected two numbers as arguments"),
    });

    math.add_value("pi", Number(std::f64::consts::PI));
    math.add_fn("pow", |vm, args| match vm.get_args(args) {
        [Number(a), Number(b)] => Ok(Number(a.powf(*b))),
        _ => external_error!("math.pow: Expected two numbers as arguments"),
    });

    math_fn_1!("radians", to_radians);
    math_fn_1!(recip);
    math_fn_1!(sin);
    math_fn_1!(sinh);
    math_fn_1!(sqrt);

    math.add_fn("sum", |vm, args| match vm.get_args(args) {
        [] => external_error!("sum: Missing argument"),
        [Num2(n)] => Ok(Number(n[0] + n[1])),
        [Num4(n)] => Ok(Number((n[0] + n[1] + n[2] + n[3]) as f64)),
        [unexpected] => external_error!(
            "math.sum: Expected Num2 or Num4, found '{}'",
            type_as_string(unexpected)
        ),
        _ => external_error!("math.sum: Expected a single Num2 or Num4 argument"),
    });

    math_fn_1!(tan);
    math_fn_1!(tanh);

    math.add_value("tau", Number(std::f64::consts::PI * 2.0));

    prelude.add_map("math", math);
}
