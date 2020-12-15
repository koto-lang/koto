use crate::{external_error, type_as_string, Value, ValueMap};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("sum", |vm, args| match vm.get_args(args) {
        [Num2(n)] => Ok(Number((n[0] + n[1]).into())),
        [unexpected] => external_error!(
            "num2.sum: Expected Num2, found '{}'",
            type_as_string(unexpected)
        ),
        _ => external_error!("num2.sum: Expected a Num2 as argument"),
    });

    result
}
