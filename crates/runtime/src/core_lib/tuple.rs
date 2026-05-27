//! The `tuple` core library module

use super::value_sort::{sort_by_key_async, sort_values_async};
use crate::prelude::*;

/// Initializes the `tuple` core library module
pub fn make_module() -> KMap {
    let result = KMap::with_type("core.tuple");

    result.add_vm_fn("contains", |ctx| {
        let expected_error = "|Tuple, Any|";

        match ctx.instance_and_args(is_tuple, expected_error)? {
            (KValue::Tuple(t), [value]) => {
                let candidates = t.iter().cloned().collect::<Vec<_>>();
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
                                return unexpected_type(
                                    "a Bool from the equality comparison",
                                    &unexpected,
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

    result.add_fn("first", |ctx| {
        let expected_error = "|Tuple|";

        match ctx.instance_and_args(is_tuple, expected_error)? {
            (KValue::Tuple(t), []) => match t.first() {
                Some(value) => Ok(value.clone()),
                None => Ok(KValue::Null),
            },
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("get", |ctx| {
        use KValue::{Null, Number, Tuple};
        let expected_error = "|Tuple, Number|, or |Tuple, Number, Any|";

        let (tuple, index, default) = match ctx.instance_and_args(is_tuple, expected_error)? {
            (Tuple(tuple), [Number(n)]) => (tuple, n, Null),
            (Tuple(tuple), [Number(n), default]) => (tuple, n, default.clone()),
            (instance, args) => {
                return unexpected_args_after_instance(expected_error, instance, args);
            }
        };

        if *index >= 0 {
            match tuple.get(usize::from(index)) {
                Some(value) => Ok(value.clone()),
                None => Ok(default),
            }
        } else {
            Ok(default)
        }
    });

    result.add_fn("last", |ctx| {
        let expected_error = "|Tuple|";

        match ctx.instance_and_args(is_tuple, expected_error)? {
            (KValue::Tuple(t), []) => match t.last() {
                Some(value) => Ok(value.clone()),
                None => Ok(KValue::Null),
            },
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("is_empty", |ctx| {
        let expected_error = "|Tuple|";

        match ctx.instance_and_args(is_tuple, expected_error)? {
            (KValue::Tuple(t), []) => Ok(t.is_empty().into()),
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_vm_fn("sort_copy", |ctx| {
        let expected_error = "|Tuple|, or |Tuple, |Any| -> Any|";

        match ctx.instance_and_args(is_tuple, expected_error)? {
            (KValue::Tuple(t), []) => {
                let mut result = t.to_vec();

                ctx.run_with_vm(|mut vm| async move {
                    sort_values_async(&mut vm, &mut result).await?;
                    Ok(KValue::Tuple(result.into()))
                })
            }
            (KValue::Tuple(t), [f]) if f.is_callable() => {
                let t = t.clone();
                let f = f.clone();

                ctx.run_with_vm(|mut vm| async move {
                    let sorted = sort_by_key_async(&mut vm, &t, f).await?;
                    let result: Vec<_> = sorted.into_iter().map(|(_key, value)| value).collect();
                    Ok(KValue::Tuple(result.into()))
                })
            }
            (instance, args) => {
                unexpected_args_after_instance::<KValue>(expected_error, instance, args)
                    .map(FunctionOutput::Ready)
            }
        }
    });

    result.add_fn("to_list", |ctx| {
        let expected_error = "|Tuple|";

        match ctx.instance_and_args(is_tuple, expected_error)? {
            (KValue::Tuple(t), []) => Ok(KValue::List(KList::from_slice(t))),
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result
}

fn is_tuple(value: &KValue) -> bool {
    matches!(value, KValue::Tuple(_))
}
