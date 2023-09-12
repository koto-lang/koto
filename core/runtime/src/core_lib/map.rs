//! The `map` core library module

use super::{iterator::adaptors, value_sort::compare_values};
use crate::{prelude::*, Result};
use std::cmp::Ordering;

/// Initializes the `map` core library module
pub fn make_module() -> ValueMap {
    use Value::*;

    let result = ValueMap::with_type("core.map");

    result.add_fn("clear", |ctx| {
        let expected_error = "a Map";

        match map_instance_and_args(ctx, expected_error)? {
            (Map(m), []) => {
                m.data_mut().clear();
                Ok(Map(m.clone()))
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("contains_key", |ctx| {
        let expected_error = "a Map and key";

        match map_instance_and_args(ctx, expected_error)? {
            (Map(m), [key]) => {
                let result = m.data().contains_key(&ValueKey::try_from(key.clone())?);
                Ok(result.into())
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("extend", |ctx| {
        let expected_error = "a Map and an iterable";

        match map_instance_and_args(ctx, expected_error)? {
            (Map(m), [Map(other)]) => {
                m.data_mut().extend(
                    other
                        .data()
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone())),
                );
                Ok(Map(m.clone()))
            }
            (Map(m), [iterable]) if iterable.is_iterable() => {
                let m = m.clone();
                let iterable = iterable.clone();
                let iterator = ctx.vm.make_iterator(iterable)?;

                {
                    let mut map_data = m.data_mut();
                    let (size_hint, _) = iterator.size_hint();
                    map_data.reserve(size_hint);

                    for output in iterator {
                        use ValueIteratorOutput as Output;
                        let (key, value) = match output {
                            Output::ValuePair(key, value) => (key, value),
                            Output::Value(Tuple(t)) if t.len() == 2 => {
                                let key = t[0].clone();
                                let value = t[1].clone();
                                (key, value)
                            }
                            Output::Value(value) => (value, Null),
                            Output::Error(error) => return Err(error),
                        };

                        map_data.insert(ValueKey::try_from(key.clone())?, value);
                    }
                }

                Ok(Map(m))
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("get", |ctx| {
        let (map, key, default) = {
            let expected_error = "a Map and a key, with an optional default value";

            match map_instance_and_args(ctx, expected_error)? {
                (Map(map), [key]) => (map, key, &Null),
                (Map(map), [key, default]) => (map, key, default),
                (_, unexpected) => return type_error_with_slice(expected_error, unexpected),
            }
        };

        match map.data().get(&ValueKey::try_from(key.clone())?) {
            Some(value) => Ok(value.clone()),
            None => Ok(default.clone()),
        }
    });

    result.add_fn("get_index", |ctx| {
        let (map, index, default) = {
            let expected_error = "a Map and a non-negative number";

            match map_instance_and_args(ctx, expected_error)? {
                (Map(map), [Number(n)]) => (map, n, &Null),
                (Map(map), [Number(n), default]) => (map, n, default),
                (_, unexpected) => return type_error_with_slice(expected_error, unexpected),
            }
        };

        match map.data().get_index(index.into()) {
            Some((key, value)) => Ok(Tuple(vec![key.value().clone(), value.clone()].into())),
            None => Ok(default.clone()),
        }
    });

    result.add_fn("get_meta_map", |ctx| {
        let expected_error = "a Map";

        match map_instance_and_args(ctx, expected_error)? {
            (Map(map), []) => {
                if map.meta_map().is_some() {
                    Ok(Map(ValueMap::from_data_and_meta_maps(
                        &ValueMap::default(),
                        map,
                    )))
                } else {
                    Ok(Null)
                }
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("insert", |ctx| {
        let expected_error = "a Map and key (with optional Value to insert)";

        match map_instance_and_args(ctx, expected_error)? {
            (Map(m), [key]) => match m.data_mut().insert(ValueKey::try_from(key.clone())?, Null) {
                Some(old_value) => Ok(old_value),
                None => Ok(Null),
            },
            (Map(m), [key, value]) => {
                match m
                    .data_mut()
                    .insert(ValueKey::try_from(key.clone())?, value.clone())
                {
                    Some(old_value) => Ok(old_value),
                    None => Ok(Null),
                }
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("is_empty", |ctx| {
        let expected_error = "a Map";

        match map_instance_and_args(ctx, expected_error)? {
            (Map(m), []) => Ok(m.is_empty().into()),
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("keys", |ctx| {
        let expected_error = "a Map";

        match map_instance_and_args(ctx, expected_error)? {
            (Map(m), []) => {
                let result = adaptors::PairFirst::new(ValueIterator::with_map(m.clone()));
                Ok(ValueIterator::new(result).into())
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("remove", |ctx| {
        let expected_error = "a Map and key";

        match map_instance_and_args(ctx, expected_error)? {
            (Map(m), [key]) => match m.data_mut().shift_remove(&ValueKey::try_from(key.clone())?) {
                Some(old_value) => Ok(old_value),
                None => Ok(Null),
            },
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("size", |ctx| {
        let expected_error = "a Map";

        match map_instance_and_args(ctx, expected_error)? {
            (Map(m), []) => Ok(Number(m.len().into())),
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("sort", |ctx| {
        let expected_error = "a Map and optional sort key function";

        match map_instance_and_args(ctx, expected_error)? {
            (Map(m), []) => {
                let mut error = None;
                m.data_mut().sort_by(|key_a, _, key_b, _| {
                    if error.is_some() {
                        return Ordering::Equal;
                    }

                    match key_a.partial_cmp(key_b) {
                        Some(ordering) => ordering,
                        None => {
                            // This should never happen, ValueKeys can only be made with sortable values
                            error = Some(runtime_error!("Invalid map key encountered"));
                            Ordering::Equal
                        }
                    }
                });

                if let Some(error) = error {
                    error
                } else {
                    Ok(Map(m.clone()))
                }
            }
            (Map(m), [f]) if f.is_callable() => {
                let m = m.clone();
                let f = f.clone();
                let mut error = None;

                let get_sort_key = |vm: &mut Vm,
                                    cache: &mut DataMap,
                                    key: &ValueKey,
                                    value: &Value|
                 -> Result<Value> {
                    let value = vm.run_function(
                        f.clone(),
                        CallArgs::Separate(&[key.value().clone(), value.clone()]),
                    )?;
                    cache.insert(key.clone(), value.clone());
                    Ok(value)
                };

                let mut cache = DataMap::with_capacity(m.len());
                m.data_mut().sort_by(|key_a, value_a, key_b, value_b| {
                    if error.is_some() {
                        return Ordering::Equal;
                    }

                    let value_a = match cache.get(key_a) {
                        Some(value) => value.clone(),
                        None => match get_sort_key(ctx.vm, &mut cache, key_a, value_a) {
                            Ok(val) => val,
                            Err(e) => {
                                error.get_or_insert(Err(e));
                                Null
                            }
                        },
                    };
                    let value_b = match cache.get(key_b) {
                        Some(value) => value.clone(),
                        None => match get_sort_key(ctx.vm, &mut cache, key_b, value_b) {
                            Ok(val) => val,
                            Err(e) => {
                                error.get_or_insert(Err(e));
                                Null
                            }
                        },
                    };

                    match compare_values(ctx.vm, &value_a, &value_b) {
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
                    Ok(Map(m))
                }
            }
            (_, unexpected) => type_error_with_slice("a Map ", unexpected),
        }
    });

    result.add_fn("update", |ctx| {
        let expected_error = "a Map, key, optional default Value, and update function";

        match map_instance_and_args(ctx, expected_error)? {
            (Map(m), [key, f]) if f.is_callable() => do_map_update(
                m.clone(),
                ValueKey::try_from(key.clone())?,
                Null,
                f.clone(),
                ctx.vm,
            ),
            (Map(m), [key, default, f]) if f.is_callable() => do_map_update(
                m.clone(),
                ValueKey::try_from(key.clone())?,
                default.clone(),
                f.clone(),
                ctx.vm,
            ),
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("values", |ctx| {
        let expected_error = "a Map";

        match map_instance_and_args(ctx, expected_error)? {
            (Map(m), []) => {
                let result = adaptors::PairSecond::new(ValueIterator::with_map(m.clone()));
                Ok(ValueIterator::new(result).into())
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("with_meta_map", |ctx| {
        let expected_error = "two Maps";

        match map_instance_and_args(ctx, expected_error)? {
            (Map(data), [Map(meta)]) => Ok(Map(ValueMap::from_data_and_meta_maps(data, meta))),
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result
}

fn do_map_update(
    map: ValueMap,
    key: ValueKey,
    default: Value,
    f: Value,
    vm: &mut Vm,
) -> Result<Value> {
    if !map.data().contains_key(&key) {
        map.data_mut().insert(key.clone(), default);
    }
    let value = map.data().get(&key).cloned().unwrap();
    match vm.run_function(f, CallArgs::Single(value)) {
        Ok(new_value) => {
            map.data_mut().insert(key, new_value.clone());
            Ok(new_value)
        }
        Err(error) => Err(error),
    }
}

fn map_instance_and_args<'a>(
    ctx: &'a CallContext<'_>,
    expected_error: &str,
) -> Result<(&'a Value, &'a [Value])> {
    use Value::Map;

    // For core.map ops, allow using maps with metamaps when the ops are used as standalone
    // functions.
    match (ctx.instance(), ctx.args()) {
        (Some(instance @ Map(m)), args) if m.meta_map().is_none() => Ok((instance, args)),
        (_, [first @ Map(_), rest @ ..]) => Ok((first, rest)),
        (_, unexpected_args) => type_error_with_slice(expected_error, unexpected_args),
    }
}
