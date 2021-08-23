use crate::{runtime_error, Value, ValueIterator, ValueMap};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("iter", |vm, args| match vm.get_args(args) {
        [Num4(n)] => Ok(Iterator(ValueIterator::with_num4(*n))),
        _ => runtime_error!("num4.iter: Expected a Num4 as argument"),
    });

    result.add_fn("sum", |vm, args| match vm.get_args(args) {
        [Num4(n)] => Ok(Number(
            (n[0] as f64 + n[1] as f64 + n[2] as f64 + n[3] as f64).into(),
        )),
        [unexpected] => runtime_error!(
            "num4.sum: Expected Num4, found '{}'",
            unexpected.type_as_string()
        ),
        _ => runtime_error!("num4.sum: Expected a Num4 as argument"),
    });

    result
}
