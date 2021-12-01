use crate::{runtime_error, RuntimeResult, Value, ValueMap};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("length", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number(n.length().into())),
        _ => num2_error("length"),
    });

    result.add_fn("max", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number((n.0.max(n.1)).into())),
        _ => num2_error("max"),
    });

    result.add_fn("min", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number((n.0.min(n.1)).into())),
        _ => num2_error("min"),
    });

    result.add_fn("normalize", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Num2(n.normalize())),
        _ => num2_error("normalize"),
    });

    result.add_fn("product", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number((n.0 * n.1).into())),
        _ => num2_error("product"),
    });

    result.add_fn("sum", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number((n.0 + n.1).into())),
        _ => num2_error("sum"),
    });

    result
}

fn num2_error(name: &str) -> RuntimeResult {
    runtime_error!("num2.{}: Expected a Num2 as argument", name)
}
