use crate::{runtime_error, RuntimeResult, Value, ValueIterator, ValueMap};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("iter", |vm, args| match vm.get_args(args) {
        [Num4(n)] => Ok(Iterator(ValueIterator::with_num4(*n))),
        _ => num4_error("iter"),
    });

    result.add_fn("length", |vm, args| match vm.get_args(args) {
        [Num4(n)] => Ok(Number(n.length().into())),
        _ => num4_error("length"),
    });

    result.add_fn("max", |vm, args| match vm.get_args(args) {
        [Num4(n)] => Ok(Number((n.0.max(n.1).max(n.2).max(n.3)).into())),
        _ => num4_error("max"),
    });

    result.add_fn("min", |vm, args| match vm.get_args(args) {
        [Num4(n)] => Ok(Number((n.0.min(n.1).min(n.2).min(n.3)).into())),
        _ => num4_error("min"),
    });

    result.add_fn("normalize", |vm, args| match vm.get_args(args) {
        [Num4(n)] => Ok(Num4(n.normalize())),
        _ => num4_error("normalize"),
    });

    result.add_fn("product", |vm, args| match vm.get_args(args) {
        [Num4(n)] => Ok(Number(
            (n.0 as f64 * n.1 as f64 * n.2 as f64 * n.3 as f64).into(),
        )),
        _ => num4_error("product"),
    });

    result.add_fn("sum", |vm, args| match vm.get_args(args) {
        [Num4(n)] => Ok(Number(
            (n.0 as f64 + n.1 as f64 + n.2 as f64 + n.3 as f64).into(),
        )),
        _ => num4_error("sum"),
    });

    result
}

fn num4_error(name: &str) -> RuntimeResult {
    runtime_error!("num4.{}: Expected a Num4 as argument", name)
}
