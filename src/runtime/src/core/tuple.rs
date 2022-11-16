use crate::{
    type_error_with_slice, value_sort::sort_values, BinaryOp, RuntimeResult, Value, ValueList,
    ValueMap,
};

pub fn make_module() -> ValueMap {
    use Value::*;

    let result = ValueMap::new();

    result.add_fn("contains", |vm, args| match vm.get_args(args) {
        [Tuple(t), value] => {
            let t = t.clone();
            let value = value.clone();
            for candidate in t.iter() {
                match vm.run_binary_op(BinaryOp::Equal, value.clone(), candidate.clone()) {
                    Ok(Bool(false)) => {}
                    Ok(Bool(true)) => return Ok(true.into()),
                    Ok(unexpected) => {
                        return type_error_with_slice(
                            "tuple.contains",
                            "a Bool from the equality comparison",
                            &[unexpected],
                        )
                    }
                    Err(e) => return Err(e.with_prefix("tuple.contains")),
                }
            }
            Ok(false.into())
        }
        unexpected => type_error_with_slice(
            "tuple.contains",
            "a Tuple and Value as arguments",
            unexpected,
        ),
    });

    result.add_fn("deep_copy", |vm, args| match vm.get_args(args) {
        [value @ Tuple(_)] => Ok(value.deep_copy()),
        unexpected => expected_tuple_error("deep_copy", unexpected),
    });

    result.add_fn("first", |vm, args| match vm.get_args(args) {
        [Tuple(t)] => match t.first() {
            Some(value) => Ok(value.clone()),
            None => Ok(Null),
        },
        unexpected => expected_tuple_error("first", unexpected),
    });

    result.add_fn("get", |vm, args| {
        let (tuple, index, default) = match vm.get_args(args) {
            [Tuple(tuple), Number(n)] => (tuple, n, &Null),
            [Tuple(tuple), Number(n), default] => (tuple, n, default),
            unexpected => {
                return type_error_with_slice(
                    "tuple.get",
                    "a Tuple and Number (with optional default Value) as arguments",
                    unexpected,
                )
            }
        };

        match tuple.get::<usize>(index.into()) {
            Some(value) => Ok(value.clone()),
            None => Ok(default.clone()),
        }
    });

    result.add_fn("last", |vm, args| match vm.get_args(args) {
        [Tuple(t)] => match t.last() {
            Some(value) => Ok(value.clone()),
            None => Ok(Null),
        },
        unexpected => expected_tuple_error("last", unexpected),
    });

    result.add_fn("size", |vm, args| match vm.get_args(args) {
        [Tuple(t)] => Ok(Number(t.len().into())),
        unexpected => expected_tuple_error("size", unexpected),
    });

    result.add_fn("sort_copy", |vm, args| match vm.get_args(args) {
        [Tuple(t)] => {
            let mut result = t.to_vec();

            sort_values(vm, &mut result)?;

            Ok(Tuple(result.into()))
        }
        unexpected => expected_tuple_error("sort_copy", unexpected),
    });

    result.add_fn("to_list", |vm, args| match vm.get_args(args) {
        [Tuple(t)] => Ok(List(ValueList::from_slice(t))),
        unexpected => expected_tuple_error("to_list", unexpected),
    });

    result
}

fn expected_tuple_error(name: &str, unexpected: &[Value]) -> RuntimeResult {
    type_error_with_slice(&format!("tuple.{name}"), "a Tuple as argument", unexpected)
}
