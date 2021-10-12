use crate::{
    make_runtime_error, runtime_error,
    value_iterator::{make_iterator, ValueIterator, ValueIteratorOutput as Output},
    BinaryOp, DataMap, RuntimeResult, Value, ValueList, ValueMap, ValueVec, Vm,
};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("all", |vm, args| match vm.get_args(args) {
        [iterable, f] if iterable.is_iterable() && f.is_callable() => {
            let iter = make_iterator(iterable).unwrap().map(collect_pair);
            let f = f.clone();

            for output in iter {
                match output {
                    Output::Value(value) => match vm.run_function(f.clone(), &[value]) {
                        Ok(Bool(result)) => {
                            if !result {
                                return Ok(Bool(false));
                            }
                        }
                        Ok(unexpected) => {
                            return runtime_error!(
                                "iterator.all: Predicate should return a bool, found '{}'",
                                unexpected.type_as_string()
                            )
                        }
                        Err(error) => return Err(error.with_prefix("iterator.all")),
                    },
                    Output::Error(error) => return Err(error),
                    _ => unreachable!(),
                }
            }

            Ok(Bool(true))
        }
        _ => runtime_error!("iterator.all: Expected iterable and function as arguments"),
    });

    result.add_fn("any", |vm, args| match vm.get_args(args) {
        [iterable, f] if iterable.is_iterable() && f.is_callable() => {
            let iter = make_iterator(iterable).unwrap().map(collect_pair);
            let f = f.clone();

            for output in iter {
                match output {
                    Output::Value(value) => match vm.run_function(f.clone(), &[value]) {
                        Ok(Bool(result)) => {
                            if result {
                                return Ok(Bool(true));
                            }
                        }
                        Ok(unexpected) => {
                            return runtime_error!(
                                "iterator.any: Predicate should return a bool, found '{}'",
                                unexpected.type_as_string()
                            )
                        }
                        Err(error) => return Err(error.with_prefix("iterator.any")),
                    },
                    Output::Error(error) => return Err(error),
                    _ => unreachable!(),
                }
            }

            Ok(Bool(false))
        }
        _ => runtime_error!("iterator.any: Expected iterable and function as arguments"),
    });

    result.add_fn("chain", |vm, args| match vm.get_args(args) {
        [iterable_a, iterable_b] if iterable_a.is_iterable() && iterable_b.is_iterable() => {
            let iter_a = make_iterator(iterable_a).unwrap();
            let iter_b = make_iterator(iterable_b).unwrap();

            let mut iter = iter_a.chain(iter_b);

            Ok(Iterator(ValueIterator::make_external(move || iter.next())))
        }
        _ => runtime_error!("iterator.chain: Expected two iterables as arguments"),
    });

    result.add_fn("consume", |vm, args| match vm.get_args(args) {
        [iterable] if iterable.is_iterable() => {
            for output in make_iterator(iterable).unwrap() {
                if let Output::Error(error) = output {
                    return Err(error);
                }
            }
            Ok(Empty)
        }
        _ => runtime_error!("iterator.consume: Expected iterable as argument"),
    });

    result.add_fn("count", |vm, args| match vm.get_args(args) {
        [iterable] if iterable.is_iterable() => {
            let mut result = 0;
            for output in make_iterator(iterable).unwrap() {
                if let Output::Error(error) = output {
                    return Err(error);
                }
                result += 1;
            }
            Ok(Number(result.into()))
        }
        _ => runtime_error!("iterator.count: Expected iterable as argument"),
    });

    result.add_fn("each", |vm, args| match vm.get_args(args) {
        [iterable, f] if iterable.is_iterable() && f.is_callable() => {
            let iter = make_iterator(iterable).unwrap().map(collect_pair);
            let f = f.clone();
            let mut vm = vm.spawn_shared_vm();

            let mut iter = iter.map(move |iter_output| match iter_output {
                Output::Value(value) => match vm.run_function(f.clone(), &[value]) {
                    Ok(result) => Output::Value(result),
                    Err(error) => Output::Error(error.with_prefix("iterator.each")),
                },
                Output::Error(error) => Output::Error(error),
                _ => unreachable!(),
            });

            Ok(Iterator(ValueIterator::make_external(move || iter.next())))
        }
        _ => runtime_error!("iterator.each: Expected iterable and function as arguments"),
    });

    result.add_fn("enumerate", |vm, args| match vm.get_args(args) {
        [iterable] if iterable.is_iterable() => {
            let mut iter = make_iterator(iterable)
                .unwrap()
                .enumerate()
                .map(|(i, iter_output)| match collect_pair(iter_output) {
                    Output::Value(value) => Output::ValuePair(Number(i.into()), value),
                    Output::Error(error) => Output::Error(error),
                    _ => unreachable!(),
                });

            Ok(Iterator(ValueIterator::make_external(move || iter.next())))
        }
        _ => runtime_error!("iterator.enumerate: Expected iterable as argument"),
    });

    result.add_fn("fold", |vm, args| {
        match vm.get_args(args) {
            [iterable, result, f] if iterable.is_iterable() && f.is_callable() => {
                let result = result.clone();
                let f = f.clone();
                let mut iter = make_iterator(iterable).unwrap();

                match iter
                    .lock_internals(|iterator| {
                        let mut fold_result = result.clone();
                        for value in iterator.map(collect_pair) {
                            match value {
                                Output::Value(value) => {
                                    match vm.run_function(f.clone(), &[fold_result, value]) {
                                        Ok(result) => fold_result = result,
                                        Err(error) => {
                                            return Some(Output::Error(
                                                error.with_prefix("iterator.fold"),
                                            ))
                                        }
                                    }
                                }
                                Output::Error(error) => return Some(Output::Error(error)),
                                _ => unreachable!(),
                            }
                        }

                        Some(Output::Value(fold_result))
                    })
                    // None is never returned from the closure
                    .unwrap()
                {
                    Output::Value(result) => Ok(result),
                    Output::Error(error) => Err(error),
                    _ => unreachable!(),
                }
            }
            _ => runtime_error!(
                "iterator.fold: Expected iterable, initial value, and function as arguments"
            ),
        }
    });

    result.add_fn("intersperse", |vm, args| match vm.get_args(args) {
        [iterable, separator_fn] if iterable.is_iterable() && separator_fn.is_callable() => {
            let mut iter = make_iterator(iterable).unwrap().peekable();
            let mut intersperse = false;
            let separator_fn = separator_fn.clone();
            let mut vm = vm.spawn_shared_vm();

            let result = move || {
                if iter.peek().is_some() {
                    let result = if intersperse {
                        match vm.run_function(separator_fn.clone(), &[]) {
                            Ok(result) => Output::Value(result),
                            Err(error) => Output::Error(error.with_prefix("iterator.intersperse")),
                        }
                    } else {
                        iter.next().unwrap()
                    };
                    intersperse = !intersperse;
                    Some(result)
                } else {
                    None
                }
            };

            Ok(Iterator(ValueIterator::make_external(result)))
        }
        [iterable, separator] if iterable.is_iterable() => {
            let mut iter = make_iterator(iterable).unwrap().peekable();
            let mut intersperse = false;
            let separator = separator.clone();

            let result = move || {
                if iter.peek().is_some() {
                    let result = if intersperse {
                        Output::Value(separator.clone())
                    } else {
                        iter.next().unwrap()
                    };
                    intersperse = !intersperse;
                    Some(result)
                } else {
                    None
                }
            };

            Ok(Iterator(ValueIterator::make_external(result)))
        }
        _ => runtime_error!("iterator.intersperse: Expected iterable as argument"),
    });

    result.add_fn("keep", |vm, args| match vm.get_args(args) {
        [iterable, f] if iterable.is_iterable() && f.is_callable() => {
            let mut iter = make_iterator(iterable).unwrap().map(collect_pair);
            let f = f.clone();
            let mut vm = vm.spawn_shared_vm();

            Ok(Iterator(ValueIterator::make_external(move || {
                for output in &mut iter {
                    match output {
                        Output::Value(value) => {
                            match vm.run_function(f.clone(), &[value.clone()]) {
                                Ok(Bool(result)) => {
                                    if result {
                                        return Some(Output::Value(value));
                                    } else {
                                        continue;
                                    }
                                }
                                Ok(unexpected) => {
                                    return Some(Output::Error(make_runtime_error!(format!(
                                        "iterator.keep: Expected a Bool to be returned from the \
                                         predicate, found '{}'",
                                        unexpected.type_as_string(),
                                    ))))
                                }
                                Err(error) => {
                                    return Some(Output::Error(error.with_prefix("iterator.keep")))
                                }
                            }
                        }
                        error @ Output::Error(_) => return Some(error),
                        _ => unreachable!(),
                    }
                }
                None
            })))
        }
        _ => runtime_error!("iterator.keep: Expected iterable and function as arguments"),
    });

    result.add_fn("last", |vm, args| match vm.get_args(args) {
        [iterable] if iterable.is_iterable() => {
            let mut result = Empty;

            let mut iter = make_iterator(iterable).unwrap().map(collect_pair);
            for output in &mut iter {
                match output {
                    Output::Value(value) => result = value,
                    Output::Error(error) => return Err(error),
                    _ => unreachable!(),
                }
            }

            Ok(result)
        }
        _ => runtime_error!("iterator.keep: Expected iterable and function as arguments"),
    });

    result.add_fn("max", |vm, args| match vm.get_args(args) {
        [iterable] if iterable.is_iterable() => {
            let iterable = iterable.clone();
            let mut result: Option<Value> = None;

            for iter_output in make_iterator(&iterable).unwrap().map(collect_pair) {
                match iter_output {
                    Output::Value(value) => {
                        result = Some(match result {
                            Some(result) => match vm.run_binary_op(
                                BinaryOp::Less,
                                result.clone(),
                                value.clone(),
                            ) {
                                Ok(Bool(true)) => value,
                                Ok(Bool(false)) => result,
                                Ok(unexpected) => {
                                    return runtime_error!(
                                        "iterator.max: \
                                         Expected Bool from < comparison, found '{}'",
                                        unexpected.type_as_string()
                                    );
                                }
                                Err(error) => return Err(error.with_prefix("iterator.max")),
                            },
                            None => value,
                        })
                    }
                    Output::Error(error) => return Err(error.with_prefix("iterator.max")),
                    _ => unreachable!(),
                }
            }

            Ok(result.unwrap_or(Empty))
        }
        _ => runtime_error!("iterator.max: Expected iterable as argument"),
    });

    result.add_fn("min", |vm, args| match vm.get_args(args) {
        [iterable] if iterable.is_iterable() => {
            let iterable = iterable.clone();
            let mut result: Option<Value> = None;

            for iter_output in make_iterator(&iterable).unwrap().map(collect_pair) {
                match iter_output {
                    Output::Value(value) => {
                        result = Some(match result {
                            Some(result) => match vm.run_binary_op(
                                BinaryOp::Less,
                                result.clone(),
                                value.clone(),
                            ) {
                                Ok(Bool(true)) => result,
                                Ok(Bool(false)) => value,
                                Ok(unexpected) => {
                                    return runtime_error!(
                                        "iterator.min: \
                                         Expected Bool from < comparison, found '{}'",
                                        unexpected.type_as_string()
                                    );
                                }
                                Err(error) => return Err(error.with_prefix("iterator.min")),
                            },
                            None => value,
                        })
                    }
                    Output::Error(error) => return Err(error.with_prefix("iterator.min")),
                    _ => unreachable!(),
                }
            }

            Ok(result.unwrap_or(Empty))
        }
        _ => runtime_error!("iterator.min: Expected iterable as argument"),
    });

    result.add_fn("min_max", |vm, args| match vm.get_args(args) {
        [iterable] if iterable.is_iterable() => {
            let iterable = iterable.clone();
            let mut result = None;

            let compare_values = |vm: &mut Vm, op, a: Value, b: Value| -> RuntimeResult {
                match vm.run_binary_op(op, a.clone(), b.clone()) {
                    Ok(Bool(true)) => Ok(a),
                    Ok(Bool(false)) => Ok(b),
                    Ok(unexpected) => {
                        return runtime_error!(
                            "iterator.min_max: \
                             Expected Bool from {} comparison, found '{}'",
                            op,
                            unexpected.type_as_string()
                        );
                    }
                    Err(error) => Err(error.with_prefix("iterator.min_max")),
                }
            };

            for iter_output in make_iterator(&iterable).unwrap().map(collect_pair) {
                match iter_output {
                    Output::Value(value) => {
                        result = Some(match result {
                            Some((min, max)) => (
                                compare_values(vm, BinaryOp::Less, min, value.clone())?,
                                compare_values(vm, BinaryOp::Greater, max, value)?,
                            ),
                            None => (value.clone(), value),
                        })
                    }
                    Output::Error(error) => return Err(error),
                    _ => unreachable!(),
                }
            }

            Ok(result.map_or(Empty, |(min, max)| Tuple(vec![min, max].into())))
        }
        _ => runtime_error!("iterator.min_max: Expected iterable as argument"),
    });

    result.add_fn("next", |vm, args| match vm.get_args(args) {
        [Iterator(i)] => match i.clone().next().map(collect_pair) {
            Some(Output::Value(value)) => Ok(value),
            Some(Output::Error(error)) => Err(error),
            None => Ok(Value::Empty),
            _ => unreachable!(),
        },
        _ => runtime_error!("iterator.next: Expected iterator as argument"),
    });

    result.add_fn("position", |vm, args| match vm.get_args(args) {
        [iterable, f] if iterable.is_iterable() && f.is_callable() => {
            let iter = make_iterator(iterable).unwrap().map(collect_pair);
            let f = f.clone();

            for (i, output) in iter.enumerate() {
                match output {
                    Output::Value(value) => match vm.run_function(f.clone(), &[value.clone()]) {
                        Ok(Bool(result)) => {
                            if result {
                                return Ok(Number(i.into()));
                            }
                        }
                        Ok(unexpected) => {
                            return runtime_error!(
                                "iterator.position expects a Bool to be returned from the \
                                     predicate, found '{}'",
                                unexpected.type_as_string(),
                            )
                        }
                        Err(error) => return Err(error.with_prefix("iterator.position")),
                    },
                    Output::Error(error) => return Err(error),
                    _ => unreachable!(),
                }
            }

            Ok(Empty)
        }
        _ => runtime_error!("iterator.position: Expected iterable and function as arguments"),
    });

    result.add_fn("product", |vm, args| {
        let (iterable, initial_value) = match vm.get_args(args) {
            [iterable] if iterable.is_iterable() => (iterable.clone(), Value::Number(1.into())),
            [iterable, initial_value] if iterable.is_iterable() => {
                (iterable.clone(), initial_value.clone())
            }
            _ => return runtime_error!("iterator.product: Expected iterable as argument"),
        };

        fold_with_operator(vm, iterable, initial_value, BinaryOp::Multiply)
            .map_err(|e| e.with_prefix("iterator.product"))
    });

    result.add_fn("skip", |vm, args| match vm.get_args(args) {
        [iterable, Number(n)] if iterable.is_iterable() && *n >= 0.0 => {
            let mut iter = make_iterator(iterable).unwrap();

            for _ in 0..n.into() {
                if let Some(Output::Error(error)) = iter.next() {
                    return Err(error);
                }
            }

            Ok(Iterator(ValueIterator::make_external(move || iter.next())))
        }
        _ => {
            runtime_error!("iterator.skip: Expected iterable and non-negative number as arguments")
        }
    });

    result.add_fn("sum", |vm, args| {
        let (iterable, initial_value) = match vm.get_args(args) {
            [iterable] if iterable.is_iterable() => (iterable.clone(), Value::Number(0.into())),
            [iterable, initial_value] if iterable.is_iterable() => {
                (iterable.clone(), initial_value.clone())
            }
            _ => return runtime_error!("iterator.sum: Expected iterable as argument"),
        };

        fold_with_operator(vm, iterable, initial_value, BinaryOp::Add)
            .map_err(|e| e.with_prefix("iterator.sum"))
    });

    result.add_fn("take", |vm, args| match vm.get_args(args) {
        [iterable, Number(n)] if iterable.is_iterable() && *n >= 0.0 => {
            let mut iter = make_iterator(iterable).unwrap().take(n.into());

            Ok(Iterator(ValueIterator::make_external(move || iter.next())))
        }
        _ => {
            runtime_error!("iterator.take: Expected iterable and non-negative number as arguments")
        }
    });

    result.add_fn("to_list", |vm, args| match vm.get_args(args) {
        [iterable] if iterable.is_iterable() => {
            let mut result = ValueVec::new();

            for output in make_iterator(iterable).unwrap().map(collect_pair) {
                match output {
                    Output::Value(value) => result.push(value),
                    Output::Error(error) => return Err(error),
                    _ => unreachable!(),
                }
            }

            Ok(List(ValueList::with_data(result)))
        }
        _ => runtime_error!("iterator.to_list: Expected iterable as argument"),
    });

    result.add_fn("to_map", |vm, args| match vm.get_args(args) {
        [iterable] if iterable.is_iterable() => {
            let mut result = DataMap::new();

            for output in make_iterator(iterable).unwrap() {
                match output {
                    Output::Value(Tuple(t)) if t.data().len() == 2 => {
                        let key = t.data()[0].clone();
                        let value = t.data()[1].clone();
                        result.insert(key.into(), value);
                    }
                    Output::Value(value) => {
                        result.insert(value.into(), Value::Empty);
                    }
                    Output::ValuePair(key, value) => {
                        result.insert(key.into(), value);
                    }
                    Output::Error(error) => return Err(error),
                }
            }

            Ok(Map(ValueMap::with_data(result)))
        }
        _ => runtime_error!("iterator.to_map: Expected iterator as argument"),
    });

    result.add_fn("to_string", |vm, args| match vm.get_args(args) {
        [iterable] if iterable.is_iterable() => {
            let mut result = String::new();

            for output in make_iterator(iterable).unwrap().map(collect_pair) {
                match output {
                    Output::Value(Str(s)) => result.push_str(&s),
                    Output::Value(value) => result.push_str(&value.to_string()),
                    Output::Error(error) => return Err(error),
                    _ => unreachable!(),
                }
            }

            Ok(Str(result.into()))
        }
        _ => return runtime_error!("iterator.to_string: Expected iterable as argument"),
    });

    result.add_fn("to_tuple", |vm, args| match vm.get_args(args) {
        [iterable] if iterable.is_iterable() => {
            let mut result = Vec::new();

            for output in make_iterator(iterable).unwrap().map(collect_pair) {
                match output {
                    Output::Value(value) => result.push(value),
                    Output::Error(error) => return Err(error),
                    _ => unreachable!(),
                }
            }

            Ok(Tuple(result.into()))
        }
        _ => runtime_error!("iterator.to_tuple: Expected iterable as argument"),
    });

    result.add_fn("zip", |vm, args| match vm.get_args(args) {
        [iterable_a, iterable_b] if iterable_a.is_iterable() && iterable_b.is_iterable() => {
            let iter_a = make_iterator(iterable_a).unwrap().map(collect_pair);
            let iter_b = make_iterator(iterable_b).unwrap().map(collect_pair);

            let mut iter = iter_a.zip(iter_b).map(|(a, b)| match (a, b) {
                (Output::Value(output_a), Output::Value(output_b)) => {
                    Output::ValuePair(output_a, output_b)
                }
                (Output::Error(e), _) | (_, Output::Error(e)) => Output::Error(e),
                _ => unreachable!(),
            });

            Ok(Iterator(ValueIterator::make_external(move || iter.next())))
        }
        _ => runtime_error!("iterator.zip: Expected two iterables as arguments"),
    });

    result
}

fn collect_pair(iterator_output: Output) -> Output {
    match iterator_output {
        Output::ValuePair(first, second) => Output::Value(Value::Tuple(vec![first, second].into())),
        _ => iterator_output,
    }
}

fn fold_with_operator(
    vm: &mut Vm,
    iterable: Value,
    initial_value: Value,
    operator: BinaryOp,
) -> RuntimeResult {
    let mut result = initial_value;

    for output in make_iterator(&iterable).unwrap().map(collect_pair) {
        match output {
            Output::Value(rhs_value) => {
                result = vm.run_binary_op(operator, result, rhs_value)?;
            }
            Output::Error(error) => return Err(error),
            _ => unreachable!(),
        }
    }

    Ok(result)
}
