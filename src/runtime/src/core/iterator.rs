use crate::{
    runtime_error,
    value_iterator::{
        make_iterator, ValueIterator, ValueIteratorOutput as Output, ValueIteratorResult,
    },
    BinaryOp, DataMap, RuntimeResult, Value, ValueList, ValueMap, ValueVec, Vm,
};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("all", |vm, args| match vm.get_args(args) {
        [iterable, f] if iterable.is_iterable() && f.is_callable() => {
            let f = f.clone();
            let iter = make_iterator(iterable).unwrap().map(collect_pair);
            let vm = vm.child_vm();

            for iter_output in iter {
                match iter_output {
                    Ok(Output::Value(value)) => match vm.run_function(f.clone(), &[value]) {
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
                    Err(error) => return Err(error),
                    _ => unreachable!(),
                }
            }

            Ok(Bool(true))
        }
        _ => runtime_error!("iterator.all: Expected iterable and function as arguments"),
    });

    result.add_fn("any", |vm, args| match vm.get_args(args) {
        [iterable, f] if iterable.is_iterable() && f.is_callable() => {
            let f = f.clone();
            let iter = make_iterator(iterable).unwrap().map(collect_pair);
            let vm = vm.child_vm();

            for iter_output in iter {
                match iter_output {
                    Ok(Output::Value(value)) => match vm.run_function(f.clone(), &[value]) {
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
                    Err(error) => return Err(error),
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
            let iter = make_iterator(iterable).unwrap();
            for output in iter {
                if let Err(error) = output {
                    return Err(error);
                }
            }
            Ok(Empty)
        }
        _ => runtime_error!("iterator.consume: Expected iterable as argument"),
    });

    result.add_fn("count", |vm, args| match vm.get_args(args) {
        [iterable] if iterable.is_iterable() => {
            let iter = make_iterator(iterable).unwrap();
            let mut result = 0;
            for output in iter {
                if let Err(error) = output {
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
                Ok(Output::Value(value)) => match vm.run_function(f.clone(), &[value]) {
                    Ok(result) => Ok(Output::Value(result)),
                    Err(error) => Err(error.with_prefix("iterator.each")),
                },
                Err(error) => Err(error),
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
                    Ok(Output::Value(value)) => Ok(Output::ValuePair(Number(i.into()), value)),
                    other => other,
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
                let vm = vm.child_vm();

                match iter
                    .lock_internals(|iterator| {
                        let mut fold_result = result.clone();
                        for value in iterator.map(collect_pair) {
                            match value {
                                Ok(Output::Value(value)) => {
                                    match vm.run_function(f.clone(), &[fold_result, value]) {
                                        Ok(result) => fold_result = result,
                                        Err(error) => {
                                            return Some(Err(error.with_prefix("iterator.fold")))
                                        }
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
                            Ok(result) => Ok(Output::Value(result.clone())),
                            Err(error) => Err(error.with_prefix("iterator.intersperse")),
                        }
                    } else {
                        match iter.next().unwrap() {
                            Ok(value) => Ok(value),
                            Err(error) => Err(error),
                        }
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
                        Ok(Output::Value(separator.clone()))
                    } else {
                        match iter.next().unwrap() {
                            Ok(value) => Ok(value),
                            Err(error) => Err(error),
                        }
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
                        Ok(Output::Value(value)) => {
                            match vm.run_function(f.clone(), &[value.clone()]) {
                                Ok(Bool(result)) => {
                                    if result {
                                        return Some(Ok(Output::Value(value)));
                                    } else {
                                        continue;
                                    }
                                }
                                Ok(unexpected) => {
                                    return Some(runtime_error!(
                                        "iterator.keep expects a Bool to be returned from the \
                                         predicate, found '{}'",
                                        unexpected.type_as_string(),
                                    ))
                                }
                                Err(error) => return Some(Err(error.with_prefix("iterator.keep"))),
                            }
                        }
                        Err(error) => return Some(Err(error)),
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
                    Ok(Output::Value(value)) => result = value,
                    Err(error) => return Err(error),
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
            let vm = vm.child_vm();
            let mut result: Option<Value> = None;

            for iter_output in make_iterator(&iterable).unwrap().map(collect_pair) {
                match iter_output {
                    Ok(Output::Value(value)) => {
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
                    Err(error) => return Err(error.with_prefix("iterator.max")),
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
            let vm = vm.child_vm();
            let mut result: Option<Value> = None;

            for iter_output in make_iterator(&iterable).unwrap().map(collect_pair) {
                match iter_output {
                    Ok(Output::Value(value)) => {
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
                    Err(error) => return Err(error.with_prefix("iterator.min")),
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
            let vm = vm.child_vm();
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
                    Ok(Output::Value(value)) => {
                        result = Some(match result {
                            Some((min, max)) => (
                                compare_values(vm, BinaryOp::Less, min, value.clone())?,
                                compare_values(vm, BinaryOp::Greater, max, value)?,
                            ),
                            None => (value.clone(), value),
                        })
                    }
                    Err(error) => return Err(error),
                    _ => unreachable!(),
                }
            }

            Ok(result.map_or(Empty, |(min, max)| Tuple(vec![min, max].into())))
        }
        _ => runtime_error!("iterator.min_max: Expected iterable as argument"),
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
        _ => runtime_error!("iterator.next: Expected iterator as argument"),
    });

    result.add_fn("position", |vm, args| match vm.get_args(args) {
        [iterable, f] if iterable.is_iterable() && f.is_callable() => {
            let iter = make_iterator(iterable).unwrap().map(collect_pair);
            let f = f.clone();
            let vm = vm.child_vm();

            for (i, output) in iter.enumerate() {
                match output {
                    Ok(Output::Value(value)) => {
                        match vm.run_function(f.clone(), &[value.clone()]) {
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
                        }
                    }
                    Err(error) => return Err(error),
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
                if let Some(Err(error)) = iter.next() {
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
        _ => runtime_error!("iterator.to_list: Expected iterable as argument"),
    });

    result.add_fn("to_map", |vm, args| match vm.get_args(args) {
        [iterable] if iterable.is_iterable() => {
            let mut iterator = make_iterator(iterable).unwrap();
            let mut result = DataMap::new();

            loop {
                match iterator.next() {
                    Some(Ok(Output::Value(Tuple(t)))) if t.data().len() == 2 => {
                        let key = t.data()[0].clone();
                        let value = t.data()[1].clone();
                        result.insert(key.into(), value);
                    }
                    Some(Ok(Output::Value(value))) => {
                        result.insert(value.into(), Value::Empty);
                    }
                    Some(Ok(Output::ValuePair(key, value))) => {
                        result.insert(key.into(), value);
                    }
                    Some(Err(error)) => return Err(error),
                    None => break,
                }
            }

            Ok(Map(ValueMap::with_data(result)))
        }
        _ => runtime_error!("iterator.to_map: Expected iterator as argument"),
    });

    result.add_fn("to_string", |vm, args| match vm.get_args(args) {
        [iterable] if iterable.is_iterable() => {
            let iterator = make_iterator(iterable).unwrap();
            let mut result = String::new();

            for output in iterator.map(collect_pair) {
                match output {
                    Ok(Output::Value(Str(s))) => result.push_str(&s),
                    Ok(Output::Value(value)) => result.push_str(&value.to_string()),
                    Err(error) => return Err(error),
                    _ => unreachable!(),
                }
            }

            Ok(Str(result.into()))
        }
        _ => return runtime_error!("iterator.to_string: Expected iterable as argument"),
    });

    result.add_fn("to_tuple", |vm, args| match vm.get_args(args) {
        [iterable] if iterable.is_iterable() => {
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
        _ => runtime_error!("iterator.to_tuple: Expected iterable as argument"),
    });

    result.add_fn("zip", |vm, args| match vm.get_args(args) {
        [iterable_a, iterable_b] if iterable_a.is_iterable() && iterable_b.is_iterable() => {
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
        _ => runtime_error!("iterator.zip: Expected two iterables as arguments"),
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

fn fold_with_operator(
    vm: &mut Vm,
    iterable: Value,
    initial_value: Value,
    operator: BinaryOp,
) -> RuntimeResult {
    let vm = vm.child_vm();
    let mut result = initial_value;

    for output in make_iterator(&iterable).unwrap().map(collect_pair) {
        match output {
            Ok(Output::Value(rhs_value)) => {
                result = vm.run_binary_op(operator, result, rhs_value)?;
            }
            Err(error) => return Err(error),
            _ => unreachable!(),
        }
    }

    Ok(result)
}
