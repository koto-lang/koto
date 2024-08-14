//! The `list` core library module

use super::{
    iterator::collect_pair,
    value_sort::{sort_by_key, sort_values},
};
use crate::prelude::*;
use std::{cmp::Ordering, ops::DerefMut};

/// Initializes the `list` core library module
pub fn make_module() -> KMap {
    let result = KMap::with_type("core.list");

    result.add_fn("clear", |ctx| {
        let expected_error = "|List|";

        match ctx.instance_and_args(is_list, expected_error)? {
            (KValue::List(l), []) => {
                l.data_mut().clear();
                Ok(KValue::List(l.clone()))
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("contains", |ctx| {
        let expected_error = "|List|";

        match ctx.instance_and_args(is_list, expected_error)? {
            (KValue::List(l), [value]) => {
                let l = l.clone();
                let value = value.clone();
                for candidate in l.data().iter() {
                    match ctx
                        .vm
                        .run_binary_op(BinaryOp::Equal, value.clone(), candidate.clone())
                    {
                        Ok(KValue::Bool(false)) => {}
                        Ok(KValue::Bool(true)) => return Ok(true.into()),
                        Ok(unexpected) => {
                            return runtime_error!(
                                "list.contains: Expected Bool from comparison, found '{}'",
                                unexpected.type_as_string()
                            )
                        }
                        Err(e) => return Err(e),
                    }
                }
                Ok(false.into())
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("extend", |ctx| {
        let expected_error = "|List, Iterable|";

        match ctx.instance_and_args(is_list, expected_error)? {
            (KValue::List(l), [KValue::List(other)]) => {
                l.data_mut().extend(other.data().iter().cloned());
                Ok(KValue::List(l.clone()))
            }
            (KValue::List(l), [KValue::Tuple(other)]) => {
                l.data_mut().extend(other.iter().cloned());
                Ok(KValue::List(l.clone()))
            }
            (KValue::List(l), [iterable]) if iterable.is_iterable() => {
                let l = l.clone();
                let iterable = iterable.clone();
                let iterator = ctx.vm.make_iterator(iterable)?;

                {
                    let mut list_data = l.data_mut();
                    let (size_hint, _) = iterator.size_hint();
                    list_data.reserve(size_hint);

                    for value in iterator.map(collect_pair) {
                        match value {
                            KIteratorOutput::Value(value) => list_data.push(value.clone()),
                            KIteratorOutput::Error(error) => return Err(error),
                            _ => unreachable!(),
                        }
                    }
                }

                Ok(KValue::List(l))
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("fill", |ctx| {
        let expected_error = "|List, Any|";

        match ctx.instance_and_args(is_list, expected_error)? {
            (KValue::List(l), [value]) => {
                for v in l.data_mut().iter_mut() {
                    *v = value.clone();
                }
                Ok(KValue::List(l.clone()))
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("first", |ctx| {
        let expected_error = "|List|";

        match ctx.instance_and_args(is_list, expected_error)? {
            (KValue::List(l), []) => match l.data().first() {
                Some(value) => Ok(value.clone()),
                None => Ok(KValue::Null),
            },
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("get", |ctx| {
        let (list, index, default) = {
            let expected_error = "|List, Number|, or |List, Number, Any|";

            match ctx.instance_and_args(is_list, expected_error)? {
                (KValue::List(list), [KValue::Number(n)]) => (list, n, &KValue::Null),
                (KValue::List(list), [KValue::Number(n), default]) => (list, n, default),
                (instance, args) => {
                    return unexpected_args_after_instance(expected_error, instance, args)
                }
            }
        };

        match list.data().get::<usize>(index.into()) {
            Some(value) => Ok(value.clone()),
            None => Ok(default.clone()),
        }
    });

    result.add_fn("insert", |ctx| {
        let expected_error = "|List, Number, Any|";

        match ctx.instance_and_args(is_list, expected_error)? {
            (KValue::List(l), [KValue::Number(n), value]) => {
                let index: usize = n.into();
                if *n < 0.0 || index > l.data().len() {
                    return runtime_error!("Index out of bounds");
                }

                l.data_mut().insert(index, value.clone());
                Ok(KValue::List(l.clone()))
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("is_empty", |ctx| {
        let expected_error = "|List|";

        match ctx.instance_and_args(is_list, expected_error)? {
            (KValue::List(l), []) => Ok(l.data().is_empty().into()),
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("last", |ctx| {
        let expected_error = "|List|";

        match ctx.instance_and_args(is_list, expected_error)? {
            (KValue::List(l), []) => match l.data().last() {
                Some(value) => Ok(value.clone()),
                None => Ok(KValue::Null),
            },
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("pop", |ctx| {
        let expected_error = "|List|";

        match ctx.instance_and_args(is_list, expected_error)? {
            (KValue::List(l), []) => match l.data_mut().pop() {
                Some(value) => Ok(value),
                None => Ok(KValue::Null),
            },
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("push", |ctx| {
        let expected_error = "|List|";

        match ctx.instance_and_args(is_list, expected_error)? {
            (KValue::List(l), [value]) => {
                l.data_mut().push(value.clone());
                Ok(KValue::List(l.clone()))
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("remove", |ctx| {
        let expected_error = "|List|";

        match ctx.instance_and_args(is_list, expected_error)? {
            (KValue::List(l), [KValue::Number(n)]) => {
                let index: usize = n.into();
                if *n < 0.0 || index >= l.data().len() {
                    return runtime_error!("Index out of bounds");
                }

                Ok(l.data_mut().remove(index))
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("resize", |ctx| {
        let expected_error = "|List, Number|, or |List, Number, Any|";

        match ctx.instance_and_args(is_list, expected_error)? {
            (_, [KValue::Number(n), ..]) if *n < 0.0 => {
                runtime_error!("Expected a non-negative size")
            }
            (KValue::List(l), [KValue::Number(n)]) => {
                l.data_mut().resize(n.into(), KValue::Null);
                Ok(KValue::List(l.clone()))
            }
            (KValue::List(l), [KValue::Number(n), value]) => {
                l.data_mut().resize(n.into(), value.clone());
                Ok(KValue::List(l.clone()))
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("resize_with", |ctx| {
        let expected_error = "|List, Number, || -> Any|";

        match ctx.instance_and_args(is_list, expected_error)? {
            (KValue::List(l), [KValue::Number(n), f]) if f.is_callable() => {
                if *n < 0.0 {
                    return runtime_error!("Expected a non-negative size");
                }

                let new_size = usize::from(n);
                let len = l.len();
                let l = l.clone();
                let f = f.clone();

                match len.cmp(&new_size) {
                    Ordering::Greater => l.data_mut().truncate(new_size),
                    Ordering::Less => {
                        l.data_mut().reserve(new_size);
                        for _ in 0..new_size - len {
                            let new_value = ctx.vm.call_function(f.clone(), &[])?;
                            l.data_mut().push(new_value);
                        }
                    }
                    Ordering::Equal => {}
                }

                Ok(KValue::List(l))
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("retain", |ctx| {
        let result = {
            let expected_error = "|List, Any|";

            match ctx.instance_and_args(is_list, expected_error)? {
                (KValue::List(l), [f]) if f.is_callable() => {
                    let l = l.clone();
                    let f = f.clone();

                    let mut write_index = 0;
                    for read_index in 0..l.len() {
                        let value = l.data()[read_index].clone();
                        match ctx.vm.call_function(f.clone(), value.clone()) {
                            Ok(KValue::Bool(result)) => {
                                if result {
                                    l.data_mut()[write_index] = value;
                                    write_index += 1;
                                }
                            }
                            Ok(unexpected) => {
                                return unexpected_type(
                                    "a Bool to returned from the predicate",
                                    &unexpected,
                                );
                            }
                            Err(error) => return Err(error),
                        }
                    }
                    l.data_mut().resize(write_index, KValue::Null);
                    l
                }
                (KValue::List(l), [value]) => {
                    let l = l.clone();
                    let value = value.clone();

                    let mut error = None;
                    l.data_mut().retain(|x| {
                        if error.is_some() {
                            return true;
                        }
                        match ctx
                            .vm
                            .run_binary_op(BinaryOp::Equal, x.clone(), value.clone())
                        {
                            Ok(KValue::Bool(true)) => true,
                            Ok(KValue::Bool(false)) => false,
                            Ok(unexpected) => {
                                error = Some(unexpected_type(
                                    "a Bool from the equality comparison",
                                    &unexpected,
                                ));
                                true
                            }
                            Err(e) => {
                                error = Some(Err(e));
                                true
                            }
                        }
                    });
                    if let Some(error) = error {
                        return error;
                    }
                    l
                }
                (instance, args) => {
                    return unexpected_args_after_instance(expected_error, instance, args)
                }
            }
        };

        Ok(KValue::List(result))
    });

    result.add_fn("reverse", |ctx| {
        let expected_error = "|List|";

        match ctx.instance_and_args(is_list, expected_error)? {
            (KValue::List(l), []) => {
                l.data_mut().reverse();
                Ok(KValue::List(l.clone()))
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("sort", |ctx| {
        let expected_error = "|List|, or |List, |Any| -> Any|";

        match ctx.instance_and_args(is_list, expected_error)? {
            (KValue::List(l), []) => {
                let l = l.clone();
                let mut data = l.data_mut();
                sort_values(ctx.vm, &mut data)?;
                Ok(KValue::List(l.clone()))
            }
            (KValue::List(l), [f]) if f.is_callable() => {
                let l = l.clone();

                let sorted = sort_by_key(ctx.vm, l.data().as_ref(), f.clone())?;

                for (target_value, (_key, source_value)) in
                    l.data_mut().iter_mut().zip(sorted.into_iter())
                {
                    *target_value = source_value;
                }

                Ok(KValue::List(l))
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("swap", |ctx| {
        let expected_error = "|List, List|";

        match ctx.instance_and_args(is_list, expected_error)? {
            (KValue::List(a), [KValue::List(b)]) => {
                std::mem::swap(a.data_mut().deref_mut(), b.data_mut().deref_mut());
                Ok(KValue::Null)
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("to_tuple", |ctx| {
        let expected_error = "|List|";

        match ctx.instance_and_args(is_list, expected_error)? {
            (KValue::List(l), []) => Ok(KValue::Tuple(l.data().as_slice().into())),
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("transform", |ctx| {
        let expected_error = "|List, |Any| -> Any|";

        match ctx.instance_and_args(is_list, expected_error)? {
            (KValue::List(l), [f]) if f.is_callable() => {
                let l = l.clone();
                let f = f.clone();

                for value in l.data_mut().iter_mut() {
                    *value = match ctx.vm.call_function(f.clone(), value.clone()) {
                        Ok(result) => result,
                        Err(error) => return Err(error),
                    }
                }

                Ok(KValue::List(l))
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result
}

fn is_list(value: &KValue) -> bool {
    matches!(value, KValue::List(_))
}
