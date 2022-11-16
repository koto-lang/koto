use crate::{prelude::*, value_sort::sort_values};

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
                            "a Bool from the equality comparison",
                            &[unexpected],
                        )
                    }
                    Err(e) => return Err(e),
                }
            }
            Ok(false.into())
        }
        unexpected => type_error_with_slice("a Tuple and Value as arguments", unexpected),
    });

    result.add_fn("deep_copy", |vm, args| match vm.get_args(args) {
        [value @ Tuple(_)] => Ok(value.deep_copy()),
        unexpected => expected_tuple_error(unexpected),
    });

    result.add_fn("first", |vm, args| match vm.get_args(args) {
        [Tuple(t)] => match t.first() {
            Some(value) => Ok(value.clone()),
            None => Ok(Null),
        },
        unexpected => expected_tuple_error(unexpected),
    });

    result.add_fn("get", |vm, args| {
        let (tuple, index, default) = match vm.get_args(args) {
            [Tuple(tuple), Number(n)] => (tuple, n, &Null),
            [Tuple(tuple), Number(n), default] => (tuple, n, default),
            unexpected => {
                return type_error_with_slice(
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
        unexpected => expected_tuple_error(unexpected),
    });

    result.add_fn("size", |vm, args| match vm.get_args(args) {
        [Tuple(t)] => Ok(Number(t.len().into())),
        unexpected => expected_tuple_error(unexpected),
    });

    result.add_fn("sort_copy", |vm, args| match vm.get_args(args) {
        [Tuple(t)] => {
            let mut result = t.to_vec();

            sort_values(vm, &mut result)?;

            Ok(Tuple(result.into()))
        }
        unexpected => expected_tuple_error(unexpected),
    });

    result.add_fn("to_list", |vm, args| match vm.get_args(args) {
        [Tuple(t)] => Ok(List(ValueList::from_slice(t))),
        unexpected => expected_tuple_error(unexpected),
    });

    result
}

fn expected_tuple_error(unexpected: &[Value]) -> RuntimeResult {
    type_error_with_slice("a Tuple as argument", unexpected)
}
