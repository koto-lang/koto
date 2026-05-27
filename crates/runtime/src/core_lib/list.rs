//! The `list` core library module

use super::{
    iterator::collect_pair,
    value_sort::{sort_by_key_async, sort_values_async},
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

    result.add_vm_fn("contains", |ctx| {
        let expected_error = "|List|";

        match ctx.instance_and_args(is_list, expected_error)? {
            (KValue::List(l), [value]) => {
                let candidates = l.data().iter().cloned().collect::<Vec<_>>();
                let value = value.clone();

                ctx.run_with_vm(|mut vm| async move {
                    for candidate in candidates {
                        match vm
                            .run_binary_op(BinaryOp::Equal, value.clone(), candidate)
                            .await?
                        {
                            KValue::Bool(false) => {}
                            KValue::Bool(true) => return Ok(true.into()),
                            unexpected => {
                                return runtime_error!(
                                    "list.contains: Expected Bool from comparison, found '{}'",
                                    unexpected.type_as_string()
                                );
                            }
                        }
                    }

                    Ok(false.into())
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("extend", |ctx| {
        let expected_error = "|List, Iterable|";

        match ctx.instance_and_args(is_list, expected_error)? {
            (KValue::List(l), [KValue::List(other)]) => {
                l.data_mut().extend(other.data().iter().cloned());
                Ok(KValue::List(l.clone()).into())
            }
            (KValue::List(l), [KValue::Tuple(other)]) => {
                l.data_mut().extend(other.iter().cloned());
                Ok(KValue::List(l.clone()).into())
            }
            (KValue::List(l), [iterable]) if iterable.is_iterable() => {
                let l = l.clone();
                let iterable = iterable.clone();

                ctx.run_with_vm(|mut vm| async move {
                    let mut iterator = vm.make_iterator(iterable).await?;
                    let (size_hint, _) = iterator.size_hint();
                    l.data_mut().reserve(size_hint);

                    while let Some(output) = vm.next(&mut iterator).await? {
                        match collect_pair(output) {
                            KIteratorOutput::Value(value) => l.data_mut().push(value),
                            KIteratorOutput::Error(error) => return Err(error),
                            _ => unreachable!(),
                        }
                    }

                    Ok(KValue::List(l))
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
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
            use KValue::{List, Null, Number};
            let expected_error = "|List, Number|, or |List, Number, Any|";

            match ctx.instance_and_args(is_list, expected_error)? {
                (List(list), [Number(n)]) => (list, n, Null),
                (List(list), [Number(n), default]) => (list, n, default.clone()),
                (instance, args) => {
                    return unexpected_args_after_instance(expected_error, instance, args);
                }
            }
        };

        if *index >= 0 {
            match list.data().get(usize::from(index)) {
                Some(value) => Ok(value.clone()),
                None => Ok(default),
            }
        } else {
            Ok(default)
        }
    });

    result.add_fn("insert", |ctx| {
        let expected_error = "|List, Number, Any|";

        match ctx.instance_and_args(is_list, expected_error)? {
            (KValue::List(l), [KValue::Number(n), value]) => {
                let index: usize = n.into();
                if *n < 0.0 || index > l.data().len() {
                    return runtime_error!("index out of bounds");
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
                    return runtime_error!("index out of bounds");
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
                runtime_error!("expected a non-negative size")
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

    result.add_vm_fn("resize_with", |ctx| {
        let expected_error = "|List, Number, || -> Any|";

        match ctx.instance_and_args(is_list, expected_error)? {
            (KValue::List(l), [KValue::Number(n), f]) if f.is_callable() => {
                if *n < 0.0 {
                    return runtime_error!("expected a non-negative size");
                }

                let new_size = usize::from(n);
                let len = l.len();
                let l = l.clone();
                let f = f.clone();

                match len.cmp(&new_size) {
                    Ordering::Greater => l.data_mut().truncate(new_size),
                    Ordering::Less => {
                        return ctx.run_with_vm(|mut vm| async move {
                            l.data_mut().reserve(new_size);
                            for _ in 0..new_size - len {
                                let new_value =
                                    vm.call_function_with_args(f.clone(), Vec::new()).await?;
                                l.data_mut().push(new_value);
                            }

                            Ok(KValue::List(l))
                        });
                    }
                    Ordering::Equal => {}
                }

                Ok(KValue::List(l).into())
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_vm_fn("retain", |ctx| {
        let expected_error = "|List, Any|";

        match ctx.instance_and_args(is_list, expected_error)? {
            (KValue::List(l), [f]) if f.is_callable() => {
                let l = l.clone();
                let f = f.clone();
                let values = l.data().iter().cloned().collect::<Vec<_>>();

                ctx.run_with_vm(|mut vm| async move {
                    let mut result = ValueVec::with_capacity(values.len());

                    for value in values {
                        match vm.call_function_with_arg(f.clone(), value.clone()).await? {
                            KValue::Bool(keep) => {
                                if keep {
                                    result.push(value);
                                }
                            }
                            unexpected => {
                                return unexpected_type(
                                    "a Bool to returned from the predicate",
                                    &unexpected,
                                );
                            }
                        }
                    }

                    *l.data_mut() = result;
                    Ok(KValue::List(l))
                })
            }
            (KValue::List(l), [value]) => {
                let l = l.clone();
                let value = value.clone();
                let values = l.data().iter().cloned().collect::<Vec<_>>();

                ctx.run_with_vm(|mut vm| async move {
                    let mut result = ValueVec::with_capacity(values.len());

                    for candidate in values {
                        match vm
                            .run_binary_op(BinaryOp::Equal, candidate.clone(), value.clone())
                            .await?
                        {
                            KValue::Bool(keep) => {
                                if keep {
                                    result.push(candidate);
                                }
                            }
                            unexpected => {
                                return unexpected_type(
                                    "a Bool from the equality comparison",
                                    &unexpected,
                                );
                            }
                        }
                    }

                    *l.data_mut() = result;
                    Ok(KValue::List(l))
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
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

    result.add_vm_fn("sort", |ctx| {
        let expected_error = "|List|, or |List, |Any| -> Any|";

        match ctx.instance_and_args(is_list, expected_error)? {
            (KValue::List(l), []) => {
                let l = l.clone();
                let mut values = l.data().iter().cloned().collect::<Vec<_>>();

                ctx.run_with_vm(|mut vm| async move {
                    sort_values_async(&mut vm, &mut values).await?;
                    *l.data_mut() = values.into_iter().collect();
                    Ok(KValue::List(l))
                })
            }
            (KValue::List(l), [f]) if f.is_callable() => {
                let l = l.clone();
                let f = f.clone();
                let values = l.data().iter().cloned().collect::<Vec<_>>();

                ctx.run_with_vm(|mut vm| async move {
                    let sorted = sort_by_key_async(&mut vm, &values, f).await?;

                    *l.data_mut() = sorted
                        .into_iter()
                        .map(|(_key, value)| value)
                        .collect::<ValueVec>();

                    Ok(KValue::List(l))
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
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

    result.add_vm_fn("transform", |ctx| {
        let expected_error = "|List, |Any| -> Any|";

        match ctx.instance_and_args(is_list, expected_error)? {
            (KValue::List(l), [f]) if f.is_callable() => {
                let l = l.clone();
                let f = f.clone();
                let values = l.data().iter().cloned().collect::<Vec<_>>();

                ctx.run_with_vm(|mut vm| async move {
                    let mut result = ValueVec::with_capacity(values.len());

                    for value in values {
                        result.push(vm.call_function_with_arg(f.clone(), value).await?);
                    }

                    *l.data_mut() = result;
                    Ok(KValue::List(l))
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result
}

fn is_list(value: &KValue) -> bool {
    matches!(value, KValue::List(_))
}
