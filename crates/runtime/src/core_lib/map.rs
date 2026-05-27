//! The `map` core library module

use super::{iterator::adaptors, value_sort::compare_values_async};
use crate::{Result, prelude::*};
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

    result.add_vm_fn("extend", |ctx| {
        let expected_error = "|Map, Iterable|";

        match vm_map_instance_and_args(ctx, expected_error)? {
            (KValue::Map(m), [KValue::Map(other)]) => {
                m.data_mut().extend(
                    other
                        .data()
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone())),
                );
                Ok(KValue::Map(m.clone()).into())
            }
            (KValue::Map(m), [iterable]) if iterable.is_iterable() => {
                let m = m.clone();
                let iterable = iterable.clone();

                ctx.run_with_vm(|mut vm| async move {
                    let mut iterator = vm.make_iterator(iterable).await?;
                    let (size_hint, _) = iterator.size_hint();
                    m.data_mut().reserve(size_hint);

                    while let Some(output) = vm.next(&mut iterator).await? {
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

                        m.data_mut().insert(ValueKey::try_from(key.clone())?, value);
                    }

                    Ok(KValue::Map(m))
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_fn("get", |ctx| {
        let (map, key, default) = {
            let expected_error = "|Map, Any|, or |Map, Any, Any|";

            match map_instance_and_args(ctx, expected_error)? {
                (KValue::Map(map), [key]) => (map, key, &KValue::Null),
                (KValue::Map(map), [key, default]) => (map, key, default),
                (instance, args) => {
                    return unexpected_args_after_instance(expected_error, instance, args);
                }
            }
        };

        let result = map
            .get(&ValueKey::try_from(key.clone())?)
            .unwrap_or_else(|| default.clone());

        Ok(result)
    });

    result.add_fn("get_index", |ctx| {
        use KValue::{Map, Null, Number};
        let expected_error = "|Map, Number|";

        let (map, index, default) = match map_instance_and_args(ctx, expected_error)? {
            (Map(map), [Number(n)]) => (map, n, Null),
            (Map(map), [Number(n), default]) => (map, n, default.clone()),
            (instance, args) => {
                return unexpected_args_after_instance(expected_error, instance, args);
            }
        };

        if *index >= 0 {
            match map.data().get_index(usize::from(index)) {
                Some((key, value)) => Ok(KValue::Tuple(
                    vec![key.value().clone(), value.clone()].into(),
                )),
                None => Ok(default),
            }
        } else {
            Ok(default)
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

    result.add_vm_fn("sort", |ctx| {
        let expected_error = "|Map|, or |Map, |Any, Any| -> Any|";

        match vm_map_instance_and_args(ctx, expected_error)? {
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
                            error = Some(runtime_error!("invalid map key encountered"));
                            Ordering::Equal
                        }
                    }
                });

                if let Some(error) = error {
                    error.map(FunctionOutput::Ready)
                } else {
                    Ok(KValue::Map(m.clone()).into())
                }
            }
            (KValue::Map(m), [f]) if f.is_callable() => {
                let m = m.clone();
                let f = f.clone();
                let entries = m
                    .data()
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect::<Vec<_>>();

                ctx.run_with_vm(|mut vm| async move {
                    let mut keyed_entries = Vec::with_capacity(entries.len());

                    for (key, value) in entries {
                        let sort_key = vm
                            .call_function_with_args(
                                f.clone(),
                                vec![key.value().clone(), value.clone()],
                            )
                            .await?;
                        keyed_entries.push((sort_key, key, value));
                    }

                    sort_map_entries_by_key(&mut vm, &mut keyed_entries).await?;

                    *m.data_mut() = keyed_entries
                        .into_iter()
                        .map(|(_sort_key, key, value)| (key, value))
                        .collect::<ValueMap>();

                    Ok(KValue::Map(m))
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("update", |ctx| {
        let expected_error = "|Map, Any, |Any| -> Any||, or |Map, Any, Any, |Any| -> Any|";

        match vm_map_instance_and_args(ctx, expected_error)? {
            (KValue::Map(m), [key, f]) if f.is_callable() => do_map_update(
                m.clone(),
                ValueKey::try_from(key.clone())?,
                KValue::Null,
                f.clone(),
                ctx,
            ),
            (KValue::Map(m), [key, default, f]) if f.is_callable() => do_map_update(
                m.clone(),
                ValueKey::try_from(key.clone())?,
                default.clone(),
                f.clone(),
                ctx,
            ),
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
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
    ctx: &mut VmCallContext<'_>,
) -> Result<FunctionOutput> {
    if !map.data().contains_key(&key) {
        map.data_mut().insert(key.clone(), default);
    }
    let value = map.get(&key).unwrap();

    ctx.run_with_vm(|mut vm| async move {
        let new_value = vm.call_function_with_arg(f, value).await?;
        map.data_mut().insert(key, new_value.clone());
        Ok(new_value)
    })
}

async fn sort_map_entries_by_key(
    vm: &mut AsyncKotoVm,
    entries: &mut [(KValue, ValueKey, KValue)],
) -> Result<()> {
    for i in 1..entries.len() {
        let mut j = i;

        while j > 0
            && compare_values_async(vm, &entries[j].0, &entries[j - 1].0).await? == Ordering::Less
        {
            entries.swap(j, j - 1);
            j -= 1;
        }
    }

    Ok(())
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

fn vm_map_instance_and_args<'a>(
    ctx: &'a VmCallContext<'_>,
    expected_error: &str,
) -> Result<(&'a KValue, &'a [KValue])> {
    use KValue::Map;

    match (ctx.instance(), ctx.args()) {
        (instance @ Map(m), args) if m.meta_map().is_none() => Ok((instance, args)),
        (_, [first @ Map(_), rest @ ..]) => Ok((first, rest)),
        (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
    }
}
