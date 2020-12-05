use crate::{
    external_error, type_as_string, value,
    value_iterator::{
        is_iterable, make_iterator, ValueIterator, ValueIteratorOutput as Output,
        ValueIteratorResult,
    },
    Value, ValueHashMap, ValueList, ValueMap, ValueVec,
};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("all", |vm, args| match vm.get_args(args) {
        [iterable, Function(f)] if is_iterable(iterable) => {
            let f = f.clone();
            let iter = make_iterator(iterable).unwrap().map(collect_pair);
            let mut vm = vm.spawn_shared_vm();

            for iter_output in iter {
                match iter_output {
                    Ok(Output::Value(value)) => match vm.run_function(&f, &[value]) {
                        Ok(Bool(result)) => {
                            if !result {
                                return Ok(Bool(false));
                            }
                        }
                        Ok(unexpected) => {
                            return external_error!(
                                "iterator.all: Predicate should return a bool, found '{}'",
                                type_as_string(&unexpected)
                            )
                        }
                        Err(error) => return Err(error),
                    },
                    Err(error) => return Err(error),
                    _ => unreachable!(),
                }
            }

            Ok(Bool(true))
        }
        _ => external_error!("iterator.all: Expected iterable and function as arguments"),
    });

    result.add_fn("any", |vm, args| match vm.get_args(args) {
        [iterable, Function(f)] if is_iterable(iterable) => {
            let f = f.clone();
            let iter = make_iterator(iterable).unwrap().map(collect_pair);
            let mut vm = vm.spawn_shared_vm();

            for iter_output in iter {
                match iter_output {
                    Ok(Output::Value(value)) => match vm.run_function(&f, &[value]) {
                        Ok(Bool(result)) => {
                            if result {
                                return Ok(Bool(true));
                            }
                        }
                        Ok(unexpected) => {
                            return external_error!(
                                "iterator.any: Predicate should return a bool, found '{}'",
                                type_as_string(&unexpected)
                            )
                        }
                        Err(error) => return Err(error),
                    },
                    Err(error) => return Err(error),
                    _ => unreachable!(),
                }
            }

            Ok(Bool(false))
        }
        _ => external_error!("iterator.any: Expected iterable and function as arguments"),
    });

    result.add_fn("consume", |vm, args| match vm.get_args(args) {
        [iterable] if is_iterable(iterable) => {
            let iter = make_iterator(iterable).unwrap();
            iter.for_each(drop);
            Ok(Empty)
        }
        _ => external_error!("iterator.consume: Expected iterable as argument"),
    });

    result.add_fn("count", |vm, args| match vm.get_args(args) {
        [iterable] if is_iterable(iterable) => {
            let iter = make_iterator(iterable).unwrap();
            Ok(Number(iter.count() as f64))
        }
        _ => external_error!("iterator.count: Expected iterable as argument"),
    });

    result.add_fn("each", |vm, args| match vm.get_args(args) {
        [iterable, Function(f)] if is_iterable(iterable) => {
            let iter = make_iterator(iterable).unwrap().map(collect_pair);
            let f = f.clone();
            let mut vm = vm.spawn_shared_vm();

            let mut iter = iter.map(move |iter_output| match iter_output {
                Ok(Output::Value(value)) => match vm.run_function(&f, &[value]) {
                    Ok(result) => Ok(Output::Value(result)),
                    Err(error) => Err(error),
                },
                Err(error) => Err(error),
                _ => unreachable!(),
            });

            Ok(Iterator(ValueIterator::make_external(move || iter.next())))
        }
        _ => external_error!("iterator.each: Expected iterable and function as arguments"),
    });

    result.add_fn("enumerate", |vm, args| match vm.get_args(args) {
        [iterable] if is_iterable(iterable) => {
            let mut iter = make_iterator(iterable)
                .unwrap()
                .enumerate()
                .map(|(i, iter_output)| match collect_pair(iter_output) {
                    Ok(Output::Value(value)) => Ok(Output::ValuePair(Number(i as f64), value)),
                    other => other,
                });

            Ok(Iterator(ValueIterator::make_external(move || iter.next())))
        }
        _ => external_error!("iterator.enumerate: Expected iterable as argument"),
    });

    result.add_fn("fold", |vm, args| {
        match vm.get_args(args) {
            [iterable, result, Function(f)] if is_iterable(iterable) => {
                let result = result.clone();
                let f = f.clone();
                let mut iter = make_iterator(iterable).unwrap();
                let mut vm = vm.spawn_shared_vm();

                match iter
                    .lock_internals(|iterator| {
                        let mut fold_result = result.clone();
                        for value in iterator.map(collect_pair) {
                            match value {
                                Ok(Output::Value(value)) => {
                                    match vm.run_function(&f, &[fold_result, value]) {
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
                    // None is never returned from the closure
                    .unwrap()
                {
                    Ok(Output::Value(result)) => Ok(result),
                    Err(error) => Err(error),
                    _ => unreachable!(),
                }
            }
            _ => external_error!(
                "iterator.fold: Expected iterable, initial value, and function as arguments"
            ),
        }
    });

    result.add_fn("keep", |vm, args| match vm.get_args(args) {
        [iterable, Function(f)] if is_iterable(iterable) => {
            let mut iter = make_iterator(iterable).unwrap().map(collect_pair);
            let f = f.clone();
            let mut vm = vm.spawn_shared_vm();

            Ok(Iterator(ValueIterator::make_external(move || {
                for output in &mut iter {
                    match output {
                        Ok(Output::Value(value)) => match vm.run_function(&f, &[value.clone()]) {
                            Ok(Bool(result)) => {
                                if result {
                                    return Some(Ok(Output::Value(value)));
                                } else {
                                    continue;
                                }
                            }
                            Ok(unexpected) => {
                                return Some(external_error!(
                                    "iterator.keep expects a Bool to be returned from the \
                                         predicate, found '{}'",
                                    value::type_as_string(&unexpected),
                                ))
                            }
                            Err(error) => return Some(Err(error)),
                        },
                        Err(error) => return Some(Err(error)),
                        _ => unreachable!(),
                    }
                }
                None
            })))
        }
        _ => external_error!("iterator.keep: Expected iterable and function as arguments"),
    });

    result.add_fn("max", |vm, args| match vm.get_args(args) {
        [iterable] if is_iterable(iterable) => {
            let mut result = None;

            for iter_output in make_iterator(iterable).unwrap().map(collect_pair) {
                match iter_output {
                    Ok(Output::Value(value)) => {
                        result = Some(match result {
                            Some(result) => std::cmp::max(result, value),
                            None => value,
                        })
                    }
                    Err(error) => return Err(error),
                    _ => unreachable!(),
                }
            }

            Ok(result.unwrap_or(Empty))
        }
        _ => external_error!("iterator.min: Expected iterable as argument"),
    });

    result.add_fn("min", |vm, args| match vm.get_args(args) {
        [iterable] if is_iterable(iterable) => {
            let mut result = None;

            for iter_output in make_iterator(iterable).unwrap().map(collect_pair) {
                match iter_output {
                    Ok(Output::Value(value)) => {
                        result = Some(match result {
                            Some(result) => std::cmp::min(result, value),
                            None => value,
                        })
                    }
                    Err(error) => return Err(error),
                    _ => unreachable!(),
                }
            }

            Ok(result.unwrap_or(Empty))
        }
        _ => external_error!("iterator.min: Expected iterable as argument"),
    });

    result.add_fn("next", |vm, args| match vm.get_args(args) {
        [Iterator(i)] => {
            let result = match i.clone().next().map(collect_pair) {
                Some(Ok(Output::Value(value))) => value,
                Some(Err(error)) => return Err(error),
                None => Value::Empty,
                _ => unreachable!(),
            };
            Ok(result)
        }
        _ => external_error!("iterator.next: Expected iterator as argument"),
    });

    result.add_fn("position", |vm, args| match vm.get_args(args) {
        [iterable, Function(f)] if is_iterable(iterable) => {
            let iter = make_iterator(iterable).unwrap().map(collect_pair);
            let f = f.clone();
            let mut vm = vm.spawn_shared_vm();

            for (i, output) in iter.enumerate() {
                match output {
                    Ok(Output::Value(value)) => match vm.run_function(&f, &[value.clone()]) {
                        Ok(Bool(result)) => {
                            if result {
                                return Ok(Number(i as f64));
                            }
                        }
                        Ok(unexpected) => {
                            return external_error!(
                                "iterator.position expects a Bool to be returned from the \
                                 predicate, found '{}'",
                                value::type_as_string(&unexpected),
                            )
                        }
                        Err(error) => return Err(error),
                    },
                    Err(error) => return Err(error),
                    _ => unreachable!(),
                }
            }

            Ok(Empty)
        }
        _ => external_error!("iterator.position: Expected iterable and function as arguments"),
    });

    result.add_fn("skip", |vm, args| match vm.get_args(args) {
        [iterable, Number(n)] if is_iterable(iterable) && *n >= 0.0 => {
            let mut iter = make_iterator(iterable).unwrap();

            for _ in 0..*n as usize {
                if let Some(Err(error)) = iter.next() {
                    return Err(error);
                }
            }

            Ok(Iterator(ValueIterator::make_external(move || iter.next())))
        }
        _ => {
            external_error!("iterator.skip: Expected iterable and non-negative number as arguments")
        }
    });

    result.add_fn("take", |vm, args| match vm.get_args(args) {
        [iterable, Number(n)] if is_iterable(iterable) && *n >= 0.0 => {
            let mut iter = make_iterator(iterable).unwrap().take(*n as usize);

            Ok(Iterator(ValueIterator::make_external(move || iter.next())))
        }
        _ => {
            external_error!("iterator.take: Expected iterable and non-negative number as arguments")
        }
    });

    result.add_fn("to_list", |vm, args| match vm.get_args(args) {
        [iterable] if is_iterable(iterable) => {
            let mut iterator = make_iterator(iterable).unwrap();
            let mut result = ValueVec::new();

            loop {
                match iterator.next().map(collect_pair) {
                    Some(Ok(Output::Value(value))) => result.push(value),
                    Some(Err(error)) => return Err(error),
                    Some(_) => unreachable!(),
                    None => break,
                }
            }

            Ok(List(ValueList::with_data(result)))
        }
        _ => external_error!("iterator.to_list: Expected iterable as argument"),
    });

    result.add_fn("to_map", |vm, args| match vm.get_args(args) {
        [iterable] if is_iterable(iterable) => {
            let mut iterator = make_iterator(iterable).unwrap();
            let mut result = ValueHashMap::new();

            loop {
                match iterator.next() {
                    Some(Ok(Output::Value(Tuple(t)))) if t.data().len() == 2 => {
                        let key = t.data()[0].clone();
                        let value = t.data()[1].clone();
                        result.insert(key, value);
                    }
                    Some(Ok(Output::Value(value))) => {
                        result.insert(value, Value::Empty);
                    }
                    Some(Ok(Output::ValuePair(key, value))) => {
                        result.insert(key, value);
                    }
                    Some(Err(error)) => return Err(error),
                    None => break,
                }
            }

            Ok(Map(ValueMap::with_data(result)))
        }
        _ => external_error!("iterator.to_map: Expected iterator as argument"),
    });

    result.add_fn("to_tuple", |vm, args| match vm.get_args(args) {
        [iterable] if is_iterable(iterable) => {
            let mut iterator = make_iterator(iterable).unwrap();
            let mut result = Vec::new();

            loop {
                match iterator.next().map(collect_pair) {
                    Some(Ok(Output::Value(value))) => result.push(value),
                    Some(Err(error)) => return Err(error),
                    Some(_) => unreachable!(),
                    None => break,
                }
            }

            Ok(Tuple(result.into()))
        }
        _ => external_error!("iterator.to_tuple: Expected iterable as argument"),
    });

    result.add_fn("zip", |vm, args| match vm.get_args(args) {
        [iterable_a, iterable_b] if is_iterable(iterable_a) && is_iterable(iterable_b) => {
            let iter_a = make_iterator(iterable_a).unwrap().map(collect_pair);
            let iter_b = make_iterator(iterable_b).unwrap().map(collect_pair);

            let mut iter = iter_a.zip(iter_b).map(|(a, b)| match (a, b) {
                (Ok(Output::Value(output_a)), Ok(Output::Value(output_b))) => {
                    Ok(Output::ValuePair(output_a, output_b))
                }
                (Err(e), _) | (_, Err(e)) => Err(e),
                _ => unreachable!(),
            });

            Ok(Iterator(ValueIterator::make_external(move || iter.next())))
        }
        _ => external_error!("iterator.zip: Expected two iterables as arguments"),
    });

    result
}

fn collect_pair(iterator_output: ValueIteratorResult) -> ValueIteratorResult {
    match iterator_output {
        Ok(Output::ValuePair(first, second)) => {
            Ok(Output::Value(Value::Tuple(vec![first, second].into())))
        }
        _ => iterator_output,
    }
}
