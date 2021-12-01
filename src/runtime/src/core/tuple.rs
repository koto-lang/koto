use crate::{runtime_error, value_sort::sort_values, BinaryOp, Value, ValueList, ValueMap};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("contains", |vm, args| match vm.get_args(args) {
        [Tuple(t), value] => {
            let t = t.clone();
            let value = value.clone();
            for candidate in t.data().iter() {
                match vm.run_binary_op(BinaryOp::Equal, value.clone(), candidate.clone()) {
                    Ok(Bool(false)) => {}
                    Ok(Bool(true)) => return Ok(true.into()),
                    Ok(unexpected) => {
                        return runtime_error!(
                            "tuple.contains: Expected Bool from comparison, found '{}'",
                            unexpected.type_as_string()
                        )
                    }
                    Err(e) => return Err(e.with_prefix("tuple.contains")),
                }
            }
            Ok(false.into())
        }
        _ => runtime_error!("tuple.contains: Expected tuple and value as arguments"),
    });

    result.add_fn("deep_copy", |vm, args| match vm.get_args(args) {
        [value @ Tuple(_)] => Ok(value.deep_copy()),
        _ => runtime_error!("tuple.deep_copy: Expected tuple as argument"),
    });

    result.add_fn("first", |vm, args| match vm.get_args(args) {
        [Tuple(t)] => match t.data().first() {
            Some(value) => Ok(value.clone()),
            None => Ok(Value::Empty),
        },
        _ => runtime_error!("tuple.first: Expected tuple as argument"),
    });

    result.add_fn("get", |vm, args| {
        let (tuple, index, default) = match vm.get_args(args) {
            [Tuple(tuple), Number(n)] => (tuple, n, &Empty),
            [Tuple(tuple), Number(n), default] => (tuple, n, default),
            _ => return runtime_error!("tuple.get: Expected tuple and number as arguments"),
        };

        if *index < 0.0 {
            return runtime_error!("tuple.get: Negative indices aren't allowed");
        }
        match tuple.data().get::<usize>(index.into()) {
            Some(value) => Ok(value.clone()),
            None => Ok(default.clone()),
        }
    });

    result.add_fn("last", |vm, args| match vm.get_args(args) {
        [Tuple(t)] => match t.data().last() {
            Some(value) => Ok(value.clone()),
            None => Ok(Value::Empty),
        },
        _ => runtime_error!("tuple.last: Expected tuple as argument"),
    });

    result.add_fn("size", |vm, args| match vm.get_args(args) {
        [Tuple(t)] => Ok(Number(t.data().len().into())),
        _ => runtime_error!("tuple.size: Expected tuple as argument"),
    });

    result.add_fn("sort_copy", |vm, args| match vm.get_args(args) {
        [Tuple(t)] => {
            let mut result = t.data().to_vec();

            sort_values(vm, &mut result)?;

            Ok(Tuple(result.into()))
        }
        _ => runtime_error!("tuple.sort_copy: Expected tuple as argument"),
    });

    result.add_fn("to_list", |vm, args| match vm.get_args(args) {
        [Tuple(t)] => Ok(List(ValueList::from_slice(t.data()))),
        _ => runtime_error!("tuple.to_list: Expected tuple as argument"),
    });

    result
}
