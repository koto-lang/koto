use crate::{
    external_error, value,
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

    result.add_fn("filter", |runtime, args| match args {
        [Iterator(i), Function(f)] => {
            let mut iter = i.clone();
            let f = f.clone();
            let mut runtime = runtime.spawn_shared_vm();

            Ok(Iterator(ValueIterator::make_external(move || {
                while let Some(output) = iter.next() {
                    match output {
                        Ok(Output::Value(value)) => {
                            match runtime.run_function(&f, &[value.clone()]) {
                                Ok(Bool(result)) => {
                                    if result {
                                        return Some(Ok(Output::Value(value)));
                                    } else {
                                        continue;
                                    }
                                }
                                Ok(unexpected) => {
                                    return Some(external_error!(
                                        "iterator.filter expects a Bool to be returned from the \
                                         predicate, found '{}'",
                                        value::type_as_string(&unexpected),
                                    ))
                                }
                                Err(error) => return Some(Err(error)),
                            }
                        }
                        Ok(Output::ValuePair(first, second)) => {
                            match runtime.run_function(&f, &[first.clone(), second.clone()]) {
                                Ok(Bool(result)) => {
                                    if result {
                                        return Some(Ok(Output::ValuePair(first, second)));
                                    } else {
                                        continue;
                                    }
                                }
                                Ok(unexpected) => {
                                    return Some(external_error!(
                                        "iterator.filter expects a Bool to be returned from the \
                                         predicate, found '{}'",
                                        value::type_as_string(&unexpected),
                                    ))
                                }
                                Err(error) => return Some(Err(error)),
                            }
                        }
                        Err(error) => return Some(Err(error)),
                    }
                }
                None
            })))
        }
        _ => external_error!("iterator.filter: Expected iterator and function as arguments"),
    });

    result.add_fn("fold", |runtime, args| match args {
        [Iterator(iterator), result, Function(f)] => {
            if f.arg_count != 2 {
                return external_error!(
                    "iterator.fold: The fold function must have two or three arguments, found '{}'",
                    f.arg_count,
                );
            }

            match iterator
                .clone()
                .lock_internals(|iterator| {
                    let mut fold_result = result.clone();
                    for value in iterator {
                        match collect_pair(value) {
                            Ok(Output::Value(value)) => {
                                match runtime.run_function(&f, &[fold_result, value]) {
                                    Ok(result) => fold_result = result,
                                    Err(error) => return Some(Err(error)),
                                }
                            }
                            Err(error) => return Some(Err(error)),
                            _ => unreachable!(),
                        }
                    }

                    Some(Ok(Output::Value(fold_result)))
                })
                .unwrap() // None is never returned from the closure
            {
                Ok(Output::Value(result)) => Ok(result),
                Err(error) => Err(error),
                _ => unreachable!(),
            }
        }
        [Iterator(_), _, unexpected] => external_error!(
            "iterator.fold: Expected Function as third argument, found '{}'",
            value::type_as_string(&unexpected),
        ),
        _ => external_error!("iterator.fold: Expected initial value and function as arguments"),
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
