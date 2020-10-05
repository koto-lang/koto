use crate::{
    external_error,
    value_iterator::{ValueIterator, ValueIteratorOutput},
    Value, ValueList, ValueMap, ValueVec,
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

    result.add_fn("take", |_, args| match args {
        [Iterator(i), Number(n)] if *n >= 0.0 => {
            let mut iter = i.clone().take(*n as usize);

            Ok(Iterator(ValueIterator::make_external(move || iter.next())))
        }
        _ => {
            external_error!("iterator.take: Expected iterator and non-negative number as arguments")
        }
    });

    result
}
