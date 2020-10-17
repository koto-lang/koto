use crate::{external_error, value_iterator::ValueIterator, Value, ValueMap};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("iter", |vm, args| match vm.get_args(args) {
        [Tuple(t)] => Ok(Iterator(ValueIterator::with_tuple(t.clone()))),
        _ => external_error!("tuple.iter: Expected tuple as argument"),
    });

    result
}
