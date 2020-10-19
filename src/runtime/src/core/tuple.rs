use crate::{external_error, value_iterator::ValueIterator, Value, ValueMap};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("get", |vm, args| match vm.get_args(args) {
        [Tuple(t), Number(n)] => {
            if *n < 0.0 {
                return external_error!("tuple.get: Negative indices aren't allowed");
            }
            let index = *n as usize;
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

    result.add_fn("size", |vm, args| match vm.get_args(args) {
        [Tuple(t)] => Ok(Number(t.data().len() as f64)),
        _ => external_error!("tuple.size: Expected tuple as argument"),
    });

    result
}
