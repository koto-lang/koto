use crate::{
    external_error,
    value_iterator::{ValueIterator, ValueIteratorOutput as Output, ValueIteratorResult},
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
                match iterator.next().map(|maybe_pair| collect_pair(maybe_pair)) {
                    Some(Ok(Output::Value(value))) => result.push(value),
                    Some(Err(error)) => return Err(error),
                    Some(_) => unreachable!(),
                    None => break,
                }
            }

            Ok(List(ValueList::with_data(result)))
        }
        _ => external_error!("iterator.collect: Expected iterator as argument"),
    });

    result.add_fn("enumerate", |_, args| match args {
        [Iterator(i)] => {
            let mut iter =
                i.clone()
                    .enumerate()
                    .map(|(i, maybe_pair)| match collect_pair(maybe_pair) {
                        Ok(Output::Value(value)) => Ok(Output::ValuePair(Number(i as f64), value)),
                        other => other,
                    });

            Ok(Iterator(ValueIterator::make_external(move || iter.next())))
        }
        _ => external_error!("iterator.enumerate: Expected iterator as argument"),
    });

    result.add_fn("next", |_, args| match args {
        [Iterator(i)] => {
            let result = match i.clone().next().map(|maybe_pair| collect_pair(maybe_pair)) {
                Some(Ok(Output::Value(value))) => value,
                Some(Err(error)) => return Err(error),
                None => Value::Empty,
                _ => unreachable!(),
            };
            Ok(result)
        }
        _ => external_error!("iterator.next: Expected iterator as argument"),
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

    result.add_fn("transform", |runtime, args| match args {
        [Iterator(i), Function(f)] => {
            let f = f.clone();
            let mut runtime = runtime.spawn_shared_vm();

            let mut iter = i.clone().map(move |iter_output| match iter_output {
                Ok(Output::Value(value)) => match runtime.run_function(&f, &[value.clone()]) {
                    Ok(result) => Ok(Output::Value(result)),
                    Err(error) => Err(error),
                },
                Ok(Output::ValuePair(first, second)) => {
                    match runtime.run_function(&f, &[first, second]) {
                        Ok(result) => Ok(Output::Value(result)),
                        Err(error) => Err(error),
                    }
                }
                Err(error) => Err(error),
            });

            Ok(Iterator(ValueIterator::make_external(move || iter.next())))
        }
        _ => external_error!("iterator.transform: Expected iterator and function as arguments"),
    });

    result
}

fn collect_pair(iterator_output: ValueIteratorResult) -> ValueIteratorResult {
    match iterator_output {
        Ok(Output::ValuePair(first, second)) => {
            Ok(Output::Value(Value::List(ValueList::from_slice(&[
                first, second,
            ]))))
        }
        _ => iterator_output,
    }
}
