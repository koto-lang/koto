use crate::{
    external_error, value_iterator::ValueIterator, value_sort::sort_values, BinaryOp, Value,
    ValueList, ValueMap,
};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("contains", |vm, args| match vm.get_args(args) {
        [Tuple(t), value] => {
            let t = t.clone();
            let value = value.clone();
            let vm = vm.child_vm();
            for candidate in t.data().iter() {
                match vm.run_binary_op(BinaryOp::Equal, value.clone(), candidate.clone()) {
                    Ok(Bool(false)) => {}
                    Ok(Bool(true)) => return Ok(true.into()),
                    Ok(unexpected) => {
                        return external_error!(
                            "tuple.contains: Expected Bool from comparison, found '{}'",
                            unexpected.type_as_string()
                        )
                    }
                    Err(e) => return Err(e.with_prefix("tuple.contains")),
                }
            }
            Ok(false.into())
        }
        _ => external_error!("tuple.contains: Expected tuple and value as arguments"),
    });

    result.add_fn("deep_copy", |vm, args| match vm.get_args(args) {
        [value @ Tuple(_)] => Ok(value.deep_copy()),
        _ => external_error!("tuple.deep_copy: Expected tuple as argument"),
    });

    result.add_fn("first", |vm, args| match vm.get_args(args) {
        [Tuple(t)] => match t.data().first() {
            Some(value) => Ok(value.clone()),
            None => Ok(Value::Empty),
        },
        _ => external_error!("tuple.first: Expected tuple as argument"),
    });

    result.add_fn("get", |vm, args| match vm.get_args(args) {
        [Tuple(t), Number(n)] => {
            if *n < 0.0 {
                return external_error!("tuple.get: Negative indices aren't allowed");
            }
            let index: usize = n.into();
            match t.data().get(index) {
                Some(value) => Ok(value.clone()),
                None => Ok(Value::Empty),
            }
        }
        _ => external_error!("tuple.get: Expected tuple and number as arguments"),
    });

    result.add_fn("iter", |vm, args| match vm.get_args(args) {
        [Tuple(t)] => Ok(Iterator(ValueIterator::with_tuple(t.clone()))),
        _ => external_error!("tuple.iter: Expected tuple as argument"),
    });

    result.add_fn("last", |vm, args| match vm.get_args(args) {
        [Tuple(t)] => match t.data().last() {
            Some(value) => Ok(value.clone()),
            None => Ok(Value::Empty),
        },
        _ => external_error!("tuple.last: Expected tuple as argument"),
    });

    result.add_fn("size", |vm, args| match vm.get_args(args) {
        [Tuple(t)] => Ok(Number(t.data().len().into())),
        _ => external_error!("tuple.size: Expected tuple as argument"),
    });

    result.add_fn("sort_copy", |vm, args| match vm.get_args(args) {
        [Tuple(t)] => {
            let mut result = t.data().to_vec();
            let vm = vm.child_vm();

            sort_values(vm, &mut result)?;

            Ok(Tuple(result.into()))
        }
        _ => external_error!("tuple.sort_copy: Expected tuple as argument"),
    });

    result.add_fn("to_list", |vm, args| match vm.get_args(args) {
        [Tuple(t)] => Ok(List(ValueList::from_slice(t.data()))),
        _ => external_error!("tuple.to_list: Expected tuple as argument"),
    });

    result
}
