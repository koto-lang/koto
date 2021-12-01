use crate::{unexpected_type_error_with_slice, RuntimeResult, Value, ValueMap};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("length", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number(n.length().into())),
        unexpected => num2_error("length", unexpected),
    });

    result.add_fn("max", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number((n.0.max(n.1)).into())),
        unexpected => num2_error("max", unexpected),
    });

    result.add_fn("min", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number((n.0.min(n.1)).into())),
        unexpected => num2_error("min", unexpected),
    });

    result.add_fn("normalize", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Num2(n.normalize())),
        unexpected => num2_error("normalize", unexpected),
    });

    result.add_fn("product", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number((n.0 * n.1).into())),
        unexpected => num2_error("product", unexpected),
    });

    result.add_fn("sum", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number((n.0 + n.1).into())),
        unexpected => num2_error("sum", unexpected),
    });

    result
}

fn num2_error(name: &str, unexpected: &[Value]) -> RuntimeResult {
    unexpected_type_error_with_slice(&format!("num2.{}", name), "a Num2 as argument", unexpected)
}
