use {
    crate::{
        runtime_error, value_iterator::ValueIteratorOutput as Output, value_sort::compare_values,
        DataMap, RuntimeResult, Value, ValueIterator, ValueKey, ValueMap, Vm,
    },
    std::{cmp::Ordering, ops::Deref},
};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("clear", |vm, args| match vm.get_args(args) {
        [Map(m)] => {
            m.data_mut().clear();
            Ok(Empty)
        }
        _ => runtime_error!("map.clear: Expected map as argument"),
    });

    result.add_fn("contains_key", |vm, args| match vm.get_args(args) {
        [Map(m), key] if key.is_immutable() => {
            Ok(Bool(m.data().contains_key(&ValueKey::from(key.clone()))))
        }
        [other_a, other_b, ..] => runtime_error!(
            "map.contains_key: Expected map and key as arguments, found '{}' and '{}'",
            other_a.type_as_string(),
            other_b.type_as_string()
        ),
        _ => runtime_error!("map.contains_key: Expected map and key as arguments"),
    });

    result.add_fn("copy", |vm, args| match vm.get_args(args) {
        [Map(m)] => Ok(Map(ValueMap::with_data(m.data().clone()))),
        _ => runtime_error!("map.copy: Expected map as argument"),
    });

    result.add_fn("deep_copy", |vm, args| match vm.get_args(args) {
        [value @ Map(_)] => Ok(value.deep_copy()),
        _ => runtime_error!("map.deep_copy: Expected map as argument"),
    });

    result.add_fn("get", |vm, args| match vm.get_args(args) {
        [Map(m), key] if key.is_immutable() => match m.data().get(&ValueKey::from(key.clone())) {
            Some(value) => Ok(value.clone()),
            None => Ok(Empty),
        },
        [other_a, other_b, ..] => runtime_error!(
            "map.get: Expected map and key as arguments, found '{}' and '{}'",
            other_a.type_as_string(),
            other_b.type_as_string()
        ),
        _ => runtime_error!("map.get: Expected map and key as arguments"),
    });

    result.add_fn("get_index", |vm, args| match vm.get_args(args) {
        [Map(m), Number(n)] => {
            if *n < 0.0 {
                return runtime_error!("map.get_index: Negative indices aren't allowed");
            }
            match m.data().get_index(n.into()) {
                Some((key, value)) => Ok(Tuple(vec![key.deref().clone(), value.clone()].into())),
                None => Ok(Empty),
            }
        }
        _ => runtime_error!("map.get_index: Expected map and index as arguments"),
    });

    result.add_fn("insert", |vm, args| match vm.get_args(args) {
        [Map(m), key] if key.is_immutable() => {
            match m.data_mut().insert(key.clone().into(), Empty) {
                Some(old_value) => Ok(old_value),
                None => Ok(Empty),
            }
        }
        [Map(m), key, value] if key.is_immutable() => {
            match m.data_mut().insert(key.clone().into(), value.clone()) {
                Some(old_value) => Ok(old_value),
                None => Ok(Empty),
            }
        }
        [other_a, other_b, ..] => runtime_error!(
            "map.insert: Expected map and key as arguments, found '{}' and '{}'",
            other_a.type_as_string(),
            other_b.type_as_string()
        ),
        _ => runtime_error!("map.insert: Expected map and key as arguments"),
    });

    result.add_fn("is_empty", |vm, args| match vm.get_args(args) {
        [Map(m)] => Ok(Bool(m.data().is_empty())),
        [other, ..] => runtime_error!(
            "map.is_empty: Expected map as argument, found '{}'",
            other.type_as_string(),
        ),
        _ => runtime_error!("map.is_empty: Expected map and key as arguments"),
    });

    result.add_fn("iter", |vm, args| match vm.get_args(args) {
        [Map(m)] => Ok(Iterator(ValueIterator::with_map(m.clone()))),
        [other, ..] => runtime_error!(
            "map.iter: Expected map as argument, found '{}'",
            other.type_as_string(),
        ),
        _ => runtime_error!("map.iter: Expected map as argument"),
    });

    result.add_fn("keys", |vm, args| match vm.get_args(args) {
        [Map(m)] => {
            let mut iter = ValueIterator::with_map(m.clone()).map(|output| match output {
                Output::ValuePair(key, _) => Output::Value(key),
                error @ Output::Error(_) => error,
                _ => unreachable!(),
            });

            Ok(Iterator(ValueIterator::make_external(move || iter.next())))
        }
        [other, ..] => runtime_error!(
            "map.keys: Expected map as argument, found '{}'",
            other.type_as_string(),
        ),
        _ => runtime_error!("map.keys: Expected map as argument"),
    });

    result.add_fn("remove", |vm, args| match vm.get_args(args) {
        [Map(m), key] if key.is_immutable() => {
            match m.data_mut().shift_remove(&ValueKey::from(key.clone())) {
                Some(old_value) => Ok(old_value),
                None => Ok(Empty),
            }
        }
        [other_a, other_b, ..] => runtime_error!(
            "map.remove: Expected map and key as arguments, found '{}' and '{}'",
            other_a.type_as_string(),
            other_b.type_as_string()
        ),
        _ => runtime_error!("map.remove: Expected map and key as arguments"),
    });

    result.add_fn("size", |vm, args| match vm.get_args(args) {
        [Map(m)] => Ok(Number(m.len().into())),
        [other, ..] => runtime_error!(
            "map.size: Expected map as argument, found '{}'",
            other.type_as_string(),
        ),
        _ => runtime_error!("map.size: Expected map and key as arguments"),
    });

    result.add_fn("sort", |vm, args| match vm.get_args(args) {
        [Map(m)] => {
            m.data_mut().sort_keys();
            Ok(Empty)
        }
        [Map(l), f] if f.is_callable() => {
            let m = l.clone();
            let f = f.clone();
            let vm = vm.child_vm();
            let mut error = None;

            let get_sort_key =
                |vm: &mut Vm, cache: &mut DataMap, key: &Value, value: &Value| -> RuntimeResult {
                    let value = vm.run_function(f.clone(), &[key.clone(), value.clone()])?;
                    cache.insert(key.clone().into(), value.clone());
                    Ok(value)
                };

            let mut cache = DataMap::with_capacity(m.len());
            m.data_mut().sort_by(|key_a, value_a, key_b, value_b| {
                if error.is_some() {
                    return Ordering::Equal;
                }

                let value_a = match cache.get(key_a) {
                    Some(value) => value.clone(),
                    None => match get_sort_key(vm, &mut cache, key_a, value_a) {
                        Ok(val) => val,
                        Err(e) => {
                            error.get_or_insert(Err(e.with_prefix("map.sort")));
                            Empty
                        }
                    },
                };
                let value_b = match cache.get(key_b) {
                    Some(value) => value.clone(),
                    None => match get_sort_key(vm, &mut cache, key_b, value_b) {
                        Ok(val) => val,
                        Err(e) => {
                            error.get_or_insert(Err(e.with_prefix("map.sort")));
                            Empty
                        }
                    },
                };

                match compare_values(vm, &value_a, &value_b) {
                    Ok(ordering) => ordering,
                    Err(e) => {
                        error.get_or_insert(Err(e));
                        Ordering::Equal
                    }
                }
            });

            if let Some(error) = error {
                error
            } else {
                Ok(Empty)
            }
        }
        _ => runtime_error!("map.sort: Expected map as argument"),
    });

    result.add_fn("update", |vm, args| match vm.get_args(args) {
        [Map(m), key, f] if key.is_immutable() && f.is_callable() => do_map_update(
            m.clone(),
            key.clone().into(),
            Empty,
            f.clone(),
            vm.child_vm(),
        ),
        [Map(m), key, default, f] if key.is_immutable() && f.is_callable() => do_map_update(
            m.clone(),
            key.clone().into(),
            default.clone(),
            f.clone(),
            vm.child_vm(),
        ),
        _ => runtime_error!("map.update: Expected map, key, and function as arguments"),
    });

    result.add_fn("values", |vm, args| match vm.get_args(args) {
        [Map(m)] => {
            let mut iter = ValueIterator::with_map(m.clone()).map(|output| match output {
                Output::ValuePair(_, value) => Output::Value(value),
                error @ Output::Error(_) => error,
                _ => unreachable!(),
            });

            Ok(Iterator(ValueIterator::make_external(move || iter.next())))
        }
        [other, ..] => runtime_error!(
            "map.values: Expected map as argument, found '{}'",
            other.type_as_string(),
        ),
        _ => runtime_error!("map.values: Expected map as argument"),
    });

    result
}

fn do_map_update(
    map: ValueMap,
    key: ValueKey,
    default: Value,
    f: Value,
    vm: &mut Vm,
) -> RuntimeResult {
    if !map.data().contains_key(&key) {
        map.data_mut().insert(key.clone(), default);
    }
    let value = map.data().get(&key).cloned().unwrap();
    match vm.run_function(f, &[value]) {
        Ok(new_value) => {
            map.data_mut().insert(key, new_value.clone());
            Ok(new_value)
        }
        Err(error) => Err(error.with_prefix("map.update")),
    }
}
