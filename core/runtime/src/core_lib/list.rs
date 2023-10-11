//! The `list` core library module

use super::{
    iterator::collect_pair,
    value_sort::{compare_values, sort_values},
};
use crate::prelude::*;
use std::{cmp::Ordering, ops::DerefMut};

/// Initializes the `list` core library module
pub fn make_module() -> KMap {
    let result = KMap::with_type("core.list");

    result.add_fn("clear", |ctx| {
        let expected_error = "a List";

        match ctx.instance_and_args(is_list, expected_error)? {
            (Value::List(l), []) => {
                l.data_mut().clear();
                Ok(Value::List(l.clone()))
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("contains", |ctx| {
        let expected_error = "a List and a Value";

        match ctx.instance_and_args(is_list, expected_error)? {
            (Value::List(l), [value]) => {
                let l = l.clone();
                let value = value.clone();
                for candidate in l.data().iter() {
                    match ctx
                        .vm
                        .run_binary_op(BinaryOp::Equal, value.clone(), candidate.clone())
                    {
                        Ok(Value::Bool(false)) => {}
                        Ok(Value::Bool(true)) => return Ok(true.into()),
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
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("extend", |ctx| {
        let expected_error = "a List and iterable";

        match ctx.instance_and_args(is_list, expected_error)? {
            (Value::List(l), [Value::List(other)]) => {
                l.data_mut().extend(other.data().iter().cloned());
                Ok(Value::List(l.clone()))
            }
            (Value::List(l), [Value::Tuple(other)]) => {
                l.data_mut().extend(other.iter().cloned());
                Ok(Value::List(l.clone()))
            }
            (Value::List(l), [iterable]) if iterable.is_iterable() => {
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

                Ok(Value::List(l))
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("fill", |ctx| {
        let expected_error = "a List and a Value";

        match ctx.instance_and_args(is_list, expected_error)? {
            (Value::List(l), [value]) => {
                for v in l.data_mut().iter_mut() {
                    *v = value.clone();
                }
                Ok(Value::List(l.clone()))
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("first", |ctx| {
        let expected_error = "a List";

        match ctx.instance_and_args(is_list, expected_error)? {
            (Value::List(l), []) => match l.data().first() {
                Some(value) => Ok(value.clone()),
                None => Ok(Value::Null),
            },
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("get", |ctx| {
        let (list, index, default) = {
            let expected_error = "a List and a Number (with optional default value)";

            match ctx.instance_and_args(is_list, expected_error)? {
                (Value::List(list), [Value::Number(n)]) => (list, n, &Value::Null),
                (Value::List(list), [Value::Number(n), default]) => (list, n, default),
                (_, unexpected) => return type_error_with_slice(expected_error, unexpected),
            }
        };

        match list.data().get::<usize>(index.into()) {
            Some(value) => Ok(value.clone()),
            None => Ok(default.clone()),
        }
    });

    result.add_fn("insert", |ctx| {
        let expected_error = "a List, a non-negative Number, and a Value";

        match ctx.instance_and_args(is_list, expected_error)? {
            (Value::List(l), [Value::Number(n), value]) if *n >= 0.0 => {
                let index: usize = n.into();
                if index > l.data().len() {
                    return runtime_error!("list.insert: Index out of bounds");
                }

                l.data_mut().insert(index, value.clone());
                Ok(Value::List(l.clone()))
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("is_empty", |ctx| {
        let expected_error = "a List";

        match ctx.instance_and_args(is_list, expected_error)? {
            (Value::List(l), []) => Ok(l.data().is_empty().into()),
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("last", |ctx| {
        let expected_error = "a List";

        match ctx.instance_and_args(is_list, expected_error)? {
            (Value::List(l), []) => match l.data().last() {
                Some(value) => Ok(value.clone()),
                None => Ok(Value::Null),
            },
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("pop", |ctx| {
        let expected_error = "a List";

        match ctx.instance_and_args(is_list, expected_error)? {
            (Value::List(l), []) => match l.data_mut().pop() {
                Some(value) => Ok(value),
                None => Ok(Value::Null),
            },
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("push", |ctx| {
        let expected_error = "a List";

        match ctx.instance_and_args(is_list, expected_error)? {
            (Value::List(l), [value]) => {
                l.data_mut().push(value.clone());
                Ok(Value::List(l.clone()))
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("remove", |ctx| {
        let expected_error = "a List";

        match ctx.instance_and_args(is_list, expected_error)? {
            (Value::List(l), [Value::Number(n)]) if *n >= 0.0 => {
                let index: usize = n.into();
                if index >= l.data().len() {
                    return runtime_error!(
                        "list.remove: Index out of bounds - \
                         the index is {index} but the List only has {} elements",
                        l.data().len(),
                    );
                }

                Ok(l.data_mut().remove(index))
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("resize", |ctx| {
        let expected_error = "a List, a non-negative Number, and an optional Value";

        match ctx.instance_and_args(is_list, expected_error)? {
            (Value::List(l), [Value::Number(n)]) if *n >= 0.0 => {
                l.data_mut().resize(n.into(), Value::Null);
                Ok(Value::List(l.clone()))
            }
            (Value::List(l), [Value::Number(n), value]) if *n >= 0.0 => {
                l.data_mut().resize(n.into(), value.clone());
                Ok(Value::List(l.clone()))
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("resize_with", |ctx| {
        let expected_error = "a List, a non-negative Number, and a function";

        match ctx.instance_and_args(is_list, expected_error)? {
            (Value::List(l), [Value::Number(n), f]) if *n >= 0.0 && f.is_callable() => {
                let new_size = usize::from(n);
                let len = l.len();
                let l = l.clone();
                let f = f.clone();

                match len.cmp(&new_size) {
                    Ordering::Greater => l.data_mut().truncate(new_size),
                    Ordering::Less => {
                        l.data_mut().reserve(new_size);
                        for _ in 0..new_size - len {
                            let new_value = ctx.vm.run_function(f.clone(), CallArgs::None)?;
                            l.data_mut().push(new_value);
                        }
                    }
                    Ordering::Equal => {}
                }

                Ok(Value::List(l))
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("retain", |ctx| {
        let result = {
            let expected_error = "a List, and either a predicate function or matching Value";

            match ctx.instance_and_args(is_list, expected_error)? {
                (Value::List(l), [f]) if f.is_callable() => {
                    let l = l.clone();
                    let f = f.clone();

                    let mut write_index = 0;
                    for read_index in 0..l.len() {
                        let value = l.data()[read_index].clone();
                        match ctx
                            .vm
                            .run_function(f.clone(), CallArgs::Single(value.clone()))
                        {
                            Ok(Value::Bool(result)) => {
                                if result {
                                    l.data_mut()[write_index] = value;
                                    write_index += 1;
                                }
                            }
                            Ok(unexpected) => {
                                return type_error(
                                    "a Bool to returned from the predicate",
                                    &unexpected,
                                );
                            }
                            Err(error) => return Err(error),
                        }
                    }
                    l.data_mut().resize(write_index, Value::Null);
                    l
                }
                (Value::List(l), [value]) => {
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
                            Ok(Value::Bool(true)) => true,
                            Ok(Value::Bool(false)) => false,
                            Ok(unexpected) => {
                                error = Some(type_error_with_slice(
                                    "a Bool from the equality comparison",
                                    &[unexpected],
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
                (_, unexpected) => return type_error_with_slice(expected_error, unexpected),
            }
        };

        Ok(Value::List(result))
    });

    result.add_fn("reverse", |ctx| {
        let expected_error = "a List";

        match ctx.instance_and_args(is_list, expected_error)? {
            (Value::List(l), []) => {
                l.data_mut().reverse();
                Ok(Value::List(l.clone()))
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("size", |ctx| {
        let expected_error = "a List";

        match ctx.instance_and_args(is_list, expected_error)? {
            (Value::List(l), []) => Ok(Value::Number(l.len().into())),
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("sort", |ctx| {
        let expected_error = "a List, and an optional key function";

        match ctx.instance_and_args(is_list, expected_error)? {
            (Value::List(l), []) => {
                let l = l.clone();
                let mut data = l.data_mut();
                sort_values(ctx.vm, &mut data)?;
                Ok(Value::List(l.clone()))
            }
            (Value::List(l), [f]) if f.is_callable() => {
                let l = l.clone();
                let f = f.clone();

                // apply function and construct a vec of (key, value)
                let mut pairs = l
                    .data()
                    .iter()
                    .map(|value| {
                        match ctx
                            .vm
                            .run_function(f.clone(), CallArgs::Single(value.clone()))
                        {
                            Ok(key) => Ok((key, value.clone())),
                            Err(e) => Err(e),
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                let mut error = None;

                // sort array by key (i.e. from [key, value])
                pairs.sort_by(|a, b| {
                    if error.is_some() {
                        return Ordering::Equal;
                    }

                    match compare_values(ctx.vm, &a.0, &b.0) {
                        Ok(ordering) => ordering,
                        Err(e) => {
                            error.get_or_insert(e);
                            Ordering::Equal
                        }
                    }
                });

                if let Some(error) = error {
                    return Err(error);
                }

                // collect values
                *l.data_mut() = pairs
                    .iter()
                    .map(|(_key, value)| value.clone())
                    .collect::<_>();

                Ok(Value::List(l))
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("swap", |ctx| {
        let expected_error = "two Lists";

        match ctx.instance_and_args(is_list, expected_error)? {
            (Value::List(a), [Value::List(b)]) => {
                std::mem::swap(a.data_mut().deref_mut(), b.data_mut().deref_mut());
                Ok(Value::Null)
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("to_tuple", |ctx| {
        let expected_error = "a List";

        match ctx.instance_and_args(is_list, expected_error)? {
            (Value::List(l), []) => Ok(Value::Tuple(l.data().as_slice().into())),
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("transform", |ctx| {
        let expected_error = "a List and a function";

        match ctx.instance_and_args(is_list, expected_error)? {
            (Value::List(l), [f]) if f.is_callable() => {
                let l = l.clone();
                let f = f.clone();

                for value in l.data_mut().iter_mut() {
                    *value = match ctx
                        .vm
                        .run_function(f.clone(), CallArgs::Single(value.clone()))
                    {
                        Ok(result) => result,
                        Err(error) => return Err(error),
                    }
                }

                Ok(Value::List(l))
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result
}

fn is_list(value: &Value) -> bool {
    matches!(value, Value::List(_))
}
