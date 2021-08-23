use crate::{runtime_error, Value, ValueIterator, ValueMap};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("iter", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Iterator(ValueIterator::with_num2(*n))),
        _ => runtime_error!("num2.iter: Expected a Num2 as argument"),
    });

    result.add_fn("sum", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number((n[0] + n[1]).into())),
        [unexpected] => runtime_error!(
            "num2.sum: Expected Num2, found '{}'",
            unexpected.type_as_string()
        ),
        _ => runtime_error!("num2.sum: Expected a Num2 as argument"),
    });

    result
}
