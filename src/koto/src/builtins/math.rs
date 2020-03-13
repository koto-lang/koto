use crate::{single_arg_fn, value, value::deref_value, Error, Value, ValueMap};

pub fn register(global: &mut ValueMap) {
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
    math_fn_1!(cos);
    math_fn_1!(cosh);
    math_fn_1!("degrees", to_degrees);
    math_fn_1!(exp);
    math_fn_1!(exp2);
    math_fn_1!(floor);
    math_fn_1!(log10);
    math_fn_1!(log2);
    math_fn_1!(ln);
    math_fn_1!("radians", to_radians);
    math_fn_1!(recip);
    math_fn_1!(sin);
    math_fn_1!(sinh);
    math_fn_1!(sqrt);
    math_fn_1!(tan);
    math_fn_1!(tanh);

    single_arg_fn!(math, "sum", Vec4, v, {
        Ok(Number((v[0] + v[1] + v[2] + v[3]) as f64))
    });

    math.add_value("pi", Number(std::f64::consts::PI));

    global.add_map("math", math);
}
