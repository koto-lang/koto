use crate::{unexpected_type_error_with_slice, RuntimeResult, Value, ValueMap};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("length", |vm, args| match vm.get_args(args) {
        [Num4(n)] => Ok(Number(n.length().into())),
        unexpected => num4_error("length", unexpected),
    });

    result.add_fn("max", |vm, args| match vm.get_args(args) {
        [Num4(n)] => Ok(Number((n.0.max(n.1).max(n.2).max(n.3)).into())),
        unexpected => num4_error("max", unexpected),
    });

    result.add_fn("min", |vm, args| match vm.get_args(args) {
        [Num4(n)] => Ok(Number((n.0.min(n.1).min(n.2).min(n.3)).into())),
        unexpected => num4_error("min", unexpected),
    });

    result.add_fn("normalize", |vm, args| match vm.get_args(args) {
        [Num4(n)] => Ok(Num4(n.normalize())),
        unexpected => num4_error("normalize", unexpected),
    });

    result.add_fn("product", |vm, args| match vm.get_args(args) {
        [Num4(n)] => Ok(Number(
            (n.0 as f64 * n.1 as f64 * n.2 as f64 * n.3 as f64).into(),
        )),
        unexpected => num4_error("product", unexpected),
    });

    result.add_fn("sum", |vm, args| match vm.get_args(args) {
        [Num4(n)] => Ok(Number(
            (n.0 as f64 + n.1 as f64 + n.2 as f64 + n.3 as f64).into(),
        )),
        unexpected => num4_error("sum", unexpected),
    });

    result
}

fn num4_error(name: &str, unexpected: &[Value]) -> RuntimeResult {
    unexpected_type_error_with_slice(&format!("num4.{}", name), "a Num4 as argument", unexpected)
}
