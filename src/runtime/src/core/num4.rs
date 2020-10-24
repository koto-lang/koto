use crate::{external_error, type_as_string, Value, ValueMap};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("sum", |vm, args| match vm.get_args(args) {
        [Num4(n)] => Ok(Number(
            n[0] as f64 + n[1] as f64 + n[2] as f64 + n[3] as f64,
        )),
        [unexpected] => external_error!(
            "num4.sum: Expected Num4, found '{}'",
            type_as_string(unexpected)
        ),
        _ => external_error!("num4.sum: Expected a Num4 as argument"),
    });

    result
}
