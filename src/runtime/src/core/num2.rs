use crate::{runtime_error, Value, ValueIterator, ValueMap};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("iter", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Iterator(ValueIterator::with_num2(*n))),
        _ => runtime_error!("num2.iter: Expected a Num2 as argument"),
    });

    result.add_fn("max", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number((n.0.max(n.1)).into())),
        _ => runtime_error!("num2.max: Expected a Num2 as argument"),
    });

    result.add_fn("min", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number((n.0.min(n.1)).into())),
        _ => runtime_error!("num2.min: Expected a Num2 as argument"),
    });

    result.add_fn("product", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number((n.0 * n.1).into())),
        _ => runtime_error!("num2.product: Expected a Num2 as argument"),
    });

    result.add_fn("sum", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number((n.0 + n.1).into())),
        _ => runtime_error!("num2.sum: Expected a Num2 as argument"),
    });

    result
}
