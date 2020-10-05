use {
    crate::{
        external_error, value_iterator::ValueIteratorOutput, Value, ValueList, ValueMap, ValueVec,
    },
};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("collect", |_, args| match args {
        [Iterator(i)] => {
            let mut iterator = i.clone();
            let mut result = ValueVec::new();

            loop {
                match iterator.next() {
                    Some(Ok(ValueIteratorOutput::Value(value))) => result.push(value),
                    Some(Ok(ValueIteratorOutput::ValuePair(first, second))) => {
                        result.push(List(ValueList::from_slice(&[first, second])))
                    }
                    Some(Err(error)) => return Err(error),
                    None => break,
                }
            }

            Ok(List(ValueList::with_data(result)))
        }
        _ => external_error!("iterator.collect: Expected iterator as argument"),
    });

    result
}
