//! The `map` core library module

use super::{iterator::adaptors, value_sort::compare_values};
use crate::{prelude::*, Result};
use std::cmp::Ordering;

/// Initializes the `map` core library module
pub fn make_module() -> KMap {
    let result = KMap::with_type("core.map");

    result.add_fn("clear", |ctx| {
        let expected_error = "|Map|";

        match map_instance_and_args(ctx, expected_error)? {
            (KValue::Map(m), []) => {
                m.data_mut().clear();
                Ok(KValue::Map(m.clone()))
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("contains_key", |ctx| {
        let expected_error = "|Map, Any|";

        match map_instance_and_args(ctx, expected_error)? {
            (KValue::Map(m), [key]) => {
                let result = m.data().contains_key(&ValueKey::try_from(key.clone())?);
                Ok(result.into())
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("extend", |ctx| {
        let expected_error = "|Map, Iterable|";

        match map_instance_and_args(ctx, expected_error)? {
            (KValue::Map(m), [KValue::Map(other)]) => {
                m.data_mut().extend(
                    other
                        .data()
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone())),
                );
                Ok(KValue::Map(m.clone()))
            }
            (KValue::Map(m), [iterable]) if iterable.is_iterable() => {
                let m = m.clone();
                let iterable = iterable.clone();
                let iterator = ctx.vm.make_iterator(iterable)?;

                {
                    let mut map_data = m.data_mut();
                    let (size_hint, _) = iterator.size_hint();
                    map_data.reserve(size_hint);

                    for output in iterator {
                        use KIteratorOutput as Output;
                        let (key, value) = match output {
                            Output::ValuePair(key, value) => (key, value),
                            Output::Value(KValue::Tuple(t)) if t.len() == 2 => {
                                let key = t[0].clone();
                                let value = t[1].clone();
                                (key, value)
                            }
                            Output::Value(value) => (value, KValue::Null),
                            Output::Error(error) => return Err(error),
                        };

                        map_data.insert(ValueKey::try_from(key.clone())?, value);
                    }
                }

                Ok(KValue::Map(m))
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("get", |ctx| {
        let (map, key, default) = {
            let expected_error = "|Map, Any|, or |Map, Any, Any|";

            match map_instance_and_args(ctx, expected_error)? {
                (KValue::Map(map), [key]) => (map, key, &KValue::Null),
                (KValue::Map(map), [key, default]) => (map, key, default),
                (instance, args) => {
                    return unexpected_args_after_instance(expected_error, instance, args)
                }
            }
        };

        let result = map
            .get(&ValueKey::try_from(key.clone())?)
            .unwrap_or_else(|| default.clone());

        Ok(result)
    });

    result.add_fn("get_index", |ctx| {
        let (map, index, default) = {
            let expected_error = "|Map, Number|";

            match map_instance_and_args(ctx, expected_error)? {
                (KValue::Map(map), [KValue::Number(n)]) => (map, n, &KValue::Null),
                (KValue::Map(map), [KValue::Number(n), default]) => (map, n, default),
                (instance, args) => {
                    return unexpected_args_after_instance(expected_error, instance, args)
                }
            }
        };

        match map.data().get_index(index.into()) {
            Some((key, value)) => Ok(KValue::Tuple(
                vec![key.value().clone(), value.clone()].into(),
            )),
            None => Ok(default.clone()),
        }
    });

    result.add_fn("get_meta", |ctx| {
        let expected_error = "|Map|";

        match map_instance_and_args(ctx, expected_error)? {
            (KValue::Map(map), []) => {
                if map.meta_map().is_some() {
                    Ok(KValue::Map(KMap::from_data_and_meta_maps(
                        &KMap::default(),
                        map,
                    )))
                } else {
                    Ok(KValue::Null)
                }
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("insert", |ctx| {
        let expected_error = "|Map, Any|, or |Map, Any, Any|";

        match map_instance_and_args(ctx, expected_error)? {
            (KValue::Map(m), [key]) => match m
                .data_mut()
                .insert(ValueKey::try_from(key.clone())?, KValue::Null)
            {
                Some(old_value) => Ok(old_value),
                None => Ok(KValue::Null),
            },
            (KValue::Map(m), [key, value]) => {
                match m
                    .data_mut()
                    .insert(ValueKey::try_from(key.clone())?, value.clone())
                {
                    Some(old_value) => Ok(old_value),
                    None => Ok(KValue::Null),
                }
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("is_empty", |ctx| {
        let expected_error = "|Map|";

        match map_instance_and_args(ctx, expected_error)? {
            (KValue::Map(m), []) => Ok(m.is_empty().into()),
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("keys", |ctx| {
        let expected_error = "|Map|";

        match map_instance_and_args(ctx, expected_error)? {
            (KValue::Map(m), []) => {
                let result = adaptors::PairFirst::new(KIterator::with_map(m.clone()));
                Ok(KIterator::new(result).into())
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("remove", |ctx| {
        let expected_error = "|Map, Any|";

        match map_instance_and_args(ctx, expected_error)? {
            (KValue::Map(m), [key]) => {
                match m.data_mut().shift_remove(&ValueKey::try_from(key.clone())?) {
                    Some(old_value) => Ok(old_value),
                    None => Ok(KValue::Null),
                }
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("sort", |ctx| {
        let expected_error = "|Map|, or |Map, |Any, Any| -> Any|";

        match map_instance_and_args(ctx, expected_error)? {
            (KValue::Map(m), []) => {
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
                    Ok(KValue::Map(m.clone()))
                }
            }
            (KValue::Map(m), [f]) if f.is_callable() => {
                let m = m.clone();
                let f = f.clone();
                let mut error = None;

                let get_sort_key = |vm: &mut KotoVm,
                                    cache: &mut ValueMap,
                                    key: &ValueKey,
                                    value: &KValue|
                 -> Result<KValue> {
                    let value =
                        vm.call_function(f.clone(), &[key.value().clone(), value.clone()])?;
                    cache.insert(key.clone(), value.clone());
                    Ok(value)
                };

                let mut cache = ValueMap::with_capacity(m.len());
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
                                KValue::Null
                            }
                        },
                    };
                    let value_b = match cache.get(key_b) {
                        Some(value) => value.clone(),
                        None => match get_sort_key(ctx.vm, &mut cache, key_b, value_b) {
                            Ok(val) => val,
                            Err(e) => {
                                error.get_or_insert(Err(e));
                                KValue::Null
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
                    Ok(KValue::Map(m))
                }
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("update", |ctx| {
        let expected_error = "|Map, Any, |Any| -> Any||, or |Map, Any, Any, |Any| -> Any|";

        match map_instance_and_args(ctx, expected_error)? {
            (KValue::Map(m), [key, f]) if f.is_callable() => do_map_update(
                m.clone(),
                ValueKey::try_from(key.clone())?,
                KValue::Null,
                f.clone(),
                ctx.vm,
            ),
            (KValue::Map(m), [key, default, f]) if f.is_callable() => do_map_update(
                m.clone(),
                ValueKey::try_from(key.clone())?,
                default.clone(),
                f.clone(),
                ctx.vm,
            ),
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("values", |ctx| {
        let expected_error = "|Map|";

        match map_instance_and_args(ctx, expected_error)? {
            (KValue::Map(m), []) => {
                let result = adaptors::PairSecond::new(KIterator::with_map(m.clone()));
                Ok(KIterator::new(result).into())
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("with_meta", |ctx| {
        let expected_error = "|Map, Map|";

        match map_instance_and_args(ctx, expected_error)? {
            (KValue::Map(data), [KValue::Map(meta)]) => {
                let mut data = data.clone();
                data.set_meta_map(meta.meta_map().cloned());
                Ok(data.into())
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result
}

fn do_map_update(
    map: KMap,
    key: ValueKey,
    default: KValue,
    f: KValue,
    vm: &mut KotoVm,
) -> Result<KValue> {
    if !map.data().contains_key(&key) {
        map.data_mut().insert(key.clone(), default);
    }
    let value = map.get(&key).unwrap();
    match vm.call_function(f, value) {
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
) -> Result<(&'a KValue, &'a [KValue])> {
    use KValue::Map;

    // For core.map ops, allow using maps with metamaps when the ops are used as standalone
    // functions.
    match (ctx.instance(), ctx.args()) {
        (instance @ Map(m), args) if m.meta_map().is_none() => Ok((instance, args)),
        (_, [first @ Map(_), rest @ ..]) => Ok((first, rest)),
        (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
    }
}
