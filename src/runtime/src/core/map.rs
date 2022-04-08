use {
    super::iterator::adaptors,
    crate::{
        unexpected_type_error_with_slice, value_sort::compare_values, CallArgs, DataMap,
        RuntimeResult, Value, ValueIterator, ValueKey, ValueMap, Vm,
    },
    std::{cmp::Ordering, ops::Deref},
};

pub fn make_module() -> ValueMap {
    use Value::*;

    let result = ValueMap::new();

    result.add_fn("clear", |vm, args| match vm.get_args(args) {
        [Map(m)] => {
            m.data_mut().clear();
            Ok(Null)
        }
        unexpected => {
            unexpected_type_error_with_slice("map.clear", "a Map as argument", unexpected)
        }
    });

    result.add_fn("contains_key", |vm, args| match vm.get_args(args) {
        [Map(m), key] if key.is_immutable() => {
            Ok(Bool(m.data().contains_key(&ValueKey::from(key.clone()))))
        }
        unexpected => unexpected_type_error_with_slice(
            "map.contains_key",
            "a Map and key as arguments",
            unexpected,
        ),
    });

    result.add_fn("copy", |vm, args| match vm.get_args(args) {
        [Map(m)] => Ok(Map(ValueMap::with_data(m.data().clone()))),
        unexpected => unexpected_type_error_with_slice("map.copy", "a Map as argument", unexpected),
    });

    result.add_fn("deep_copy", |vm, args| match vm.get_args(args) {
        [value @ Map(_)] => Ok(value.deep_copy()),
        unexpected => {
            unexpected_type_error_with_slice("map.deep_copy", "a Map as argument", unexpected)
        }
    });

    result.add_fn("get", |vm, args| {
        let (map, key, default) = match vm.get_args(args) {
            [Map(map), key] if key.is_immutable() => (map, key, &Null),
            [Map(map), key, default] if key.is_immutable() => (map, key, default),
            unexpected => {
                return unexpected_type_error_with_slice(
                    "map.get",
                    "a Map and key as arguments",
                    unexpected,
                )
            }
        };

        match map.data().get(&ValueKey::from(key.clone())) {
            Some(value) => Ok(value.clone()),
            None => Ok(default.clone()),
        }
    });

    result.add_fn("get_index", |vm, args| {
        let (map, index, default) = match vm.get_args(args) {
            [Map(map), Number(n)] => (map, n, &Null),
            [Map(map), Number(n), default] => (map, n, default),
            unexpected => {
                return unexpected_type_error_with_slice(
                    "map.get_index",
                    "a Map and Number as arguments",
                    unexpected,
                )
            }
        };

        match map.data().get_index(index.into()) {
            Some((key, value)) => Ok(Tuple(vec![key.deref().clone(), value.clone()].into())),
            None => Ok(default.clone()),
        }
    });

    result.add_fn("get_meta_map", |vm, args| match vm.get_args(args) {
        [Map(map)] => {
            if map.meta_map().is_some() {
                Ok(Map(ValueMap::from_data_and_meta_maps(
                    &ValueMap::default(),
                    map,
                )))
            } else {
                Ok(Null)
            }
        }
        unexpected => unexpected_type_error_with_slice("map.get_meta_map", "a Map", unexpected),
    });

    result.add_fn("insert", |vm, args| match vm.get_args(args) {
        [Map(m), key] if key.is_immutable() => {
            match m.data_mut().insert(key.clone().into(), Null) {
                Some(old_value) => Ok(old_value),
                None => Ok(Null),
            }
        }
        [Map(m), key, value] if key.is_immutable() => {
            match m.data_mut().insert(key.clone().into(), value.clone()) {
                Some(old_value) => Ok(old_value),
                None => Ok(Null),
            }
        }
        unexpected => unexpected_type_error_with_slice(
            "map.insert",
            "a Map and key (with optional Value to insert) as arguments",
            unexpected,
        ),
    });

    result.add_fn("is_empty", |vm, args| match vm.get_args(args) {
        [Map(m)] => Ok(Bool(m.is_empty())),
        unexpected => {
            unexpected_type_error_with_slice("map.is_empty", "a Map as argument", unexpected)
        }
    });

    result.add_fn("keys", |vm, args| match vm.get_args(args) {
        [Map(m)] => {
            let result = adaptors::PairFirst::new(ValueIterator::with_map(m.clone()));
            Ok(Iterator(ValueIterator::new(result)))
        }
        unexpected => unexpected_type_error_with_slice("map.keys", "a Map as argument", unexpected),
    });

    result.add_fn("remove", |vm, args| match vm.get_args(args) {
        [Map(m), key] if key.is_immutable() => {
            match m.data_mut().shift_remove(&ValueKey::from(key.clone())) {
                Some(old_value) => Ok(old_value),
                None => Ok(Null),
            }
        }
        unexpected => {
            unexpected_type_error_with_slice("map.remove", "a Map and key as arguments", unexpected)
        }
    });

    result.add_fn("size", |vm, args| match vm.get_args(args) {
        [Map(m)] => Ok(Number(m.len().into())),
        unexpected => unexpected_type_error_with_slice("map.size", "a Map as argument", unexpected),
    });

    result.add_fn("sort", |vm, args| match vm.get_args(args) {
        [Map(m)] => {
            m.data_mut().sort_keys();
            Ok(Null)
        }
        [Map(l), f] if f.is_callable() => {
            let m = l.clone();
            let f = f.clone();
            let mut error = None;

            let get_sort_key =
                |vm: &mut Vm, cache: &mut DataMap, key: &Value, value: &Value| -> RuntimeResult {
                    let value = vm.run_function(
                        f.clone(),
                        CallArgs::Separate(&[key.clone(), value.clone()]),
                    )?;
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
                            Null
                        }
                    },
                };
                let value_b = match cache.get(key_b) {
                    Some(value) => value.clone(),
                    None => match get_sort_key(vm, &mut cache, key_b, value_b) {
                        Ok(val) => val,
                        Err(e) => {
                            error.get_or_insert(Err(e.with_prefix("map.sort")));
                            Null
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
                Ok(Null)
            }
        }
        unexpected => unexpected_type_error_with_slice(
            "map.sort",
            "a Map and optional sort key Function as arguments",
            unexpected,
        ),
    });

    result.add_fn("update", |vm, args| match vm.get_args(args) {
        [Map(m), key, f] if key.is_immutable() && f.is_callable() => {
            do_map_update(m.clone(), key.clone().into(), Null, f.clone(), vm)
        }
        [Map(m), key, default, f] if key.is_immutable() && f.is_callable() => do_map_update(
            m.clone(),
            key.clone().into(),
            default.clone(),
            f.clone(),
            vm,
        ),
        unexpected => unexpected_type_error_with_slice(
            "map.update",
            "a Map, key, optional default Value, and update Function as arguments",
            unexpected,
        ),
    });

    result.add_fn("values", |vm, args| match vm.get_args(args) {
        [Map(m)] => {
            let result = adaptors::PairSecond::new(ValueIterator::with_map(m.clone()));
            Ok(Iterator(ValueIterator::new(result)))
        }
        unexpected => {
            unexpected_type_error_with_slice("map.values", "a Map as argument", unexpected)
        }
    });

    result.add_fn("with_meta_map", |vm, args| match vm.get_args(args) {
        [Map(data), Map(meta)] => Ok(Map(ValueMap::from_data_and_meta_maps(data, meta))),
        unexpected => unexpected_type_error_with_slice(
            "map.with_meta_map",
            "two Maps as arguments",
            unexpected,
        ),
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
    match vm.run_function(f, CallArgs::Single(value)) {
        Ok(new_value) => {
            map.data_mut().insert(key, new_value.clone());
            Ok(new_value)
        }
        Err(error) => Err(error.with_prefix("map.update")),
    }
}
