pub mod adaptors;

use crate::{
    runtime_error,
    value_iterator::{make_iterator, ValueIterator, ValueIteratorOutput as Output},
    BinaryOp, CallArgs, DataMap, RuntimeError, RuntimeResult, Value, ValueList, ValueMap, ValueVec,
    Vm,
};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("all", |vm, args| match vm.get_args(args) {
        [iterable, predicate] if iterable.is_iterable() && predicate.is_callable() => {
            let predicate = predicate.clone();

            for output in make_iterator(iterable).unwrap() {
                let predicate_result = match output {
                    Output::Value(value) => {
                        vm.run_function(predicate.clone(), CallArgs::Single(value))
                    }
                    Output::ValuePair(a, b) => {
                        vm.run_function(predicate.clone(), CallArgs::AsTuple(&[a, b]))
                    }
                    Output::Error(error) => return Err(error),
                };

                match predicate_result {
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
                }
            }

            Ok(Bool(true))
        }
        _ => runtime_error!("iterator.all: Expected iterable and function as arguments"),
    });

    result.add_fn("any", |vm, args| match vm.get_args(args) {
        [iterable, predicate] if iterable.is_iterable() && predicate.is_callable() => {
            let predicate = predicate.clone();

            for output in make_iterator(iterable).unwrap() {
                let predicate_result = match output {
                    Output::Value(value) => {
                        vm.run_function(predicate.clone(), CallArgs::Single(value))
                    }
                    Output::ValuePair(a, b) => {
                        vm.run_function(predicate.clone(), CallArgs::AsTuple(&[a, b]))
                    }
                    Output::Error(error) => return Err(error),
                };

                match predicate_result {
                    Ok(Bool(result)) => {
                        if result {
                            return Ok(Bool(true));
                        }
                    }
                    Ok(unexpected) => {
                        return runtime_error!(
                            "iterator.all: Predicate should return a bool, found '{}'",
                            unexpected.type_as_string()
                        )
                    }
                    Err(error) => return Err(error.with_prefix("iterator.all")),
                }
            }

            Ok(Bool(false))
        }
        _ => runtime_error!("iterator.any: Expected iterable and function as arguments"),
    });

    result.add_fn("chain", |vm, args| match vm.get_args(args) {
        [iterable_a, iterable_b] if iterable_a.is_iterable() && iterable_b.is_iterable() => {
            let result = ValueIterator::make_external(adaptors::Chain::new(
                make_iterator(iterable_a).unwrap(),
                make_iterator(iterable_b).unwrap(),
            ));

            Ok(Iterator(result))
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

    result.add_fn("copy", |vm, args| match vm.get_args(args) {
        [Iterator(iter)] => Ok(Iterator(iter.make_copy())),
        _ => runtime_error!("iterator.copy: Expected iterator as argument"),
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
            let result = adaptors::Each::new(
                make_iterator(iterable).unwrap(),
                f.clone(),
                vm.spawn_shared_vm(),
            );

            Ok(Iterator(ValueIterator::make_external(result)))
        }
        _ => runtime_error!("iterator.each: Expected iterable and function as arguments"),
    });

    result.add_fn("cycle", |vm, args| match vm.get_args(args) {
        [iterable] if iterable.is_iterable() => {
            let result = adaptors::Cycle::new(make_iterator(iterable).unwrap());

            Ok(Iterator(ValueIterator::make_external(result)))
        }
        _ => runtime_error!("iterator.cycle: Expected iterable as argument"),
    });

    result.add_fn("enumerate", |vm, args| match vm.get_args(args) {
        [iterable] if iterable.is_iterable() => {
            let result = adaptors::Enumerate::new(make_iterator(iterable).unwrap());
            Ok(Iterator(ValueIterator::make_external(result)))
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
                    .borrow_internals(|iterator| {
                        let mut fold_result = result.clone();
                        for value in iterator.map(collect_pair) {
                            match value {
                                Output::Value(value) => {
                                    match vm.run_function(
                                        f.clone(),
                                        CallArgs::Separate(&[fold_result, value]),
                                    ) {
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
            let result = adaptors::IntersperseWith::new(
                make_iterator(iterable).unwrap(),
                separator_fn.clone(),
                vm.spawn_shared_vm(),
            );

            Ok(Iterator(ValueIterator::make_external(result)))
        }
        [iterable, separator] if iterable.is_iterable() => {
            let result =
                adaptors::Intersperse::new(make_iterator(iterable).unwrap(), separator.clone());

            Ok(Iterator(ValueIterator::make_external(result)))
        }
        _ => runtime_error!("iterator.intersperse: Expected iterable as argument"),
    });

    result.add_fn("keep", |vm, args| match vm.get_args(args) {
        [iterable, predicate] if iterable.is_iterable() && predicate.is_callable() => {
            let result = adaptors::Keep::new(
                make_iterator(iterable).unwrap(),
                predicate.clone(),
                vm.spawn_shared_vm(),
            );
            Ok(Iterator(ValueIterator::make_external(result)))
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
            run_iterator_comparison(vm, iterable, InvertResult::Yes)
                .map_err(|e| e.with_prefix("iterator.max"))
        }
        [iterable, key_fn] if iterable.is_iterable() && key_fn.is_callable() => {
            let iterable = iterable.clone();
            let key_fn = key_fn.clone();
            run_iterator_comparison_by_key(vm, iterable, key_fn, InvertResult::Yes)
                .map_err(|e| e.with_prefix("iterator.max"))
        }
        _ => runtime_error!("iterator.max: Expected iterable as argument"),
    });

    result.add_fn("min", |vm, args| match vm.get_args(args) {
        [iterable] if iterable.is_iterable() => {
            let iterable = iterable.clone();
            run_iterator_comparison(vm, iterable, InvertResult::No)
                .map_err(|e| e.with_prefix("iterator.min"))
        }
        [iterable, key_fn] if iterable.is_iterable() && key_fn.is_callable() => {
            let iterable = iterable.clone();
            let key_fn = key_fn.clone();
            run_iterator_comparison_by_key(vm, iterable, key_fn, InvertResult::No)
                .map_err(|e| e.with_prefix("iterator.min"))
        }
        _ => runtime_error!("iterator.min: Expected iterable as argument"),
    });

    result.add_fn("min_max", |vm, args| match vm.get_args(args) {
        [iterable] if iterable.is_iterable() => {
            let iterable = iterable.clone();
            let mut result = None;

            for iter_output in make_iterator(&iterable).unwrap().map(collect_pair) {
                match iter_output {
                    Output::Value(value) => {
                        result = Some(match result {
                            Some((min, max)) => (
                                compare_values(vm, min, value.clone(), InvertResult::No)
                                    .map_err(|e| e.with_prefix("iterator.min_max"))?,
                                compare_values(vm, max, value, InvertResult::Yes)
                                    .map_err(|e| e.with_prefix("iterator.min_max"))?,
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
        [iterable, key_fn] if iterable.is_iterable() && key_fn.is_callable() => {
            let key_fn = key_fn.clone();
            let mut result = None;

            for iter_output in make_iterator(iterable).unwrap().map(collect_pair) {
                match iter_output {
                    Output::Value(value) => {
                        let key =
                            vm.run_function(key_fn.clone(), CallArgs::Single(value.clone()))?;
                        let value_and_key = (value, key);

                        result = Some(match result {
                            Some((min_and_key, max_and_key)) => (
                                compare_values_with_key(
                                    vm,
                                    min_and_key,
                                    value_and_key.clone(),
                                    InvertResult::No,
                                )
                                .map_err(|e| e.with_prefix("iterator.min_max"))?,
                                compare_values_with_key(
                                    vm,
                                    max_and_key,
                                    value_and_key,
                                    InvertResult::Yes,
                                )
                                .map_err(|e| e.with_prefix("iterator.min_max"))?,
                            ),
                            None => (value_and_key.clone(), value_and_key),
                        })
                    }
                    Output::Error(error) => return Err(error),
                    _ => unreachable!(), // value pairs have been collected in collect_pair
                }
            }

            Ok(result.map_or(Empty, |((min, _), (max, _))| Tuple(vec![min, max].into())))
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
        [iterable, predicate] if iterable.is_iterable() && predicate.is_callable() => {
            let predicate = predicate.clone();

            for (i, output) in make_iterator(iterable).unwrap().enumerate() {
                let predicate_result = match output {
                    Output::Value(value) => {
                        vm.run_function(predicate.clone(), CallArgs::Single(value))
                    }
                    Output::ValuePair(a, b) => {
                        vm.run_function(predicate.clone(), CallArgs::AsTuple(&[a, b]))
                    }
                    Output::Error(error) => return Err(error),
                };

                match predicate_result {
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

            Ok(Iterator(iter))
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
            let result = adaptors::Take::new(make_iterator(iterable).unwrap(), n.into());
            Ok(Iterator(ValueIterator::make_external(result)))
        }
        _ => {
            runtime_error!("iterator.take: Expected iterable and non-negative number as arguments")
        }
    });

    result.add_fn("to_list", |vm, args| match vm.get_args(args) {
        [iterable] if iterable.is_iterable() => {
            let iterator = make_iterator(iterable).unwrap();
            let (size_hint, _) = iterator.size_hint();
            let mut result = ValueVec::with_capacity(size_hint);

            for output in iterator.map(collect_pair) {
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
            let iterator = make_iterator(iterable).unwrap();
            let (size_hint, _) = iterator.size_hint();
            let mut result = DataMap::with_capacity(size_hint);

            for output in iterator {
                match output {
                    Output::ValuePair(key, value) => {
                        result.insert(key.into(), value);
                    }
                    Output::Value(Tuple(t)) if t.data().len() == 2 => {
                        let key = t.data()[0].clone();
                        let value = t.data()[1].clone();
                        result.insert(key.into(), value);
                    }
                    Output::Value(value) => {
                        result.insert(value.into(), Value::Empty);
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
            let iterator = make_iterator(iterable).unwrap();
            let (size_hint, _) = iterator.size_hint();
            let mut result = String::with_capacity(size_hint);

            for output in iterator.map(collect_pair) {
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
            let iterator = make_iterator(iterable).unwrap();
            let (size_hint, _) = iterator.size_hint();
            let mut result = Vec::with_capacity(size_hint);

            for output in iterator.map(collect_pair) {
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
            let result = adaptors::Zip::new(
                make_iterator(iterable_a).unwrap(),
                make_iterator(iterable_b).unwrap(),
            );
            Ok(Iterator(ValueIterator::make_external(result)))
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

fn run_iterator_comparison(
    vm: &mut Vm,
    iterable: Value,
    invert_result: InvertResult,
) -> RuntimeResult {
    let mut result: Option<Value> = None;

    for iter_output in make_iterator(&iterable).unwrap().map(collect_pair) {
        match iter_output {
            Output::Value(value) => {
                result = Some(match result {
                    Some(result) => {
                        compare_values(vm, result.clone(), value.clone(), invert_result)?
                    }
                    None => value,
                })
            }
            Output::Error(error) => return Err(error),
            _ => unreachable!(),
        }
    }

    Ok(result.unwrap_or_default())
}

fn run_iterator_comparison_by_key(
    vm: &mut Vm,
    iterable: Value,
    key_fn: Value,
    invert_result: InvertResult,
) -> RuntimeResult {
    let mut result_and_key: Option<(Value, Value)> = None;

    for iter_output in make_iterator(&iterable).unwrap().map(collect_pair) {
        match iter_output {
            Output::Value(value) => {
                let key = vm.run_function(key_fn.clone(), CallArgs::Single(value.clone()))?;
                let value_and_key = (value, key);

                result_and_key = Some(match result_and_key {
                    Some(result_and_key) => {
                        compare_values_with_key(vm, result_and_key, value_and_key, invert_result)?
                    }
                    None => value_and_key,
                });
            }
            Output::Error(error) => return Err(error),
            _ => unreachable!(),
        }
    }

    Ok(result_and_key.map_or(Value::Empty, |(value, _)| value))
}

// Compares two values using BinaryOp::Less
//
// Returns the lesser of the two values, unless `invert_result` is set to Yes
fn compare_values(vm: &mut Vm, a: Value, b: Value, invert_result: InvertResult) -> RuntimeResult {
    use {InvertResult::*, Value::Bool};

    let comparison_result = vm.run_binary_op(BinaryOp::Less, a.clone(), b.clone())?;

    match (comparison_result, invert_result) {
        (Bool(true), No) => Ok(a),
        (Bool(false), No) => Ok(b),
        (Bool(true), Yes) => Ok(b),
        (Bool(false), Yes) => Ok(a),
        (other, _) => runtime_error!(
            "Expected Bool from '<' comparison, found '{}'",
            other.type_as_string()
        ),
    }
}

// Compares two values using BinaryOp::Less
//
// Returns the lesser of the two values, unless `invert_result` is set to Yes
fn compare_values_with_key(
    vm: &mut Vm,
    a_and_key: (Value, Value),
    b_and_key: (Value, Value),
    invert_result: InvertResult,
) -> Result<(Value, Value), RuntimeError> {
    use {InvertResult::*, Value::Bool};

    let comparison_result =
        vm.run_binary_op(BinaryOp::Less, a_and_key.1.clone(), b_and_key.1.clone())?;

    match (comparison_result, invert_result) {
        (Bool(true), No) => Ok(a_and_key),
        (Bool(false), No) => Ok(b_and_key),
        (Bool(true), Yes) => Ok(b_and_key),
        (Bool(false), Yes) => Ok(a_and_key),
        (other, _) => runtime_error!(
            "Expected Bool from '<' comparison, found '{}'",
            other.type_as_string()
        ),
    }
}

#[derive(Clone, Copy)]
enum InvertResult {
    Yes,
    No,
}
