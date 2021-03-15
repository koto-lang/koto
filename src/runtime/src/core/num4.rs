use crate::{external_error, Value, ValueMap};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("sum", |vm, args| match vm.get_args(args) {
        [Num4(n)] => Ok(Number(
            (n[0] as f64 + n[1] as f64 + n[2] as f64 + n[3] as f64).into(),
        )),
        [unexpected] => external_error!(
            "num4.sum: Expected Num4, found '{}'",
            unexpected.type_as_string()
        ),
        _ => external_error!("num4.sum: Expected a Num4 as argument"),
    });

    result
}
