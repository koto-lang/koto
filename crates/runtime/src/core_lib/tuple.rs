//! The `tuple` core library module

use super::value_sort::{sort_by_key, sort_values};
use crate::prelude::*;

/// Initializes the `tuple` core library module
pub fn make_module() -> KMap {
    let result = KMap::with_type("core.tuple");

    result.add_fn("contains", |ctx| {
        let expected_error = "|Tuple, Any|";

        match ctx.instance_and_args(is_tuple, expected_error)? {
            (KValue::Tuple(t), [value]) => {
                let t = t.clone();
                let value = value.clone();
                for candidate in t.iter() {
                    match ctx
                        .vm
                        .run_binary_op(BinaryOp::Equal, value.clone(), candidate.clone())
                    {
                        Ok(KValue::Bool(false)) => {}
                        Ok(KValue::Bool(true)) => return Ok(true.into()),
                        Ok(unexpected) => {
                            return unexpected_type(
                                "a Bool from the equality comparison",
                                &unexpected,
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
        let (tuple, index, default) = {
            let expected_error = "|Tuple, Number|, or |Tuple, Number, Any|";

            match ctx.instance_and_args(is_tuple, expected_error)? {
                (KValue::Tuple(tuple), [KValue::Number(n)]) => (tuple, n, &KValue::Null),
                (KValue::Tuple(tuple), [KValue::Number(n), default]) => (tuple, n, default),
                (instance, args) => {
                    return unexpected_args_after_instance(expected_error, instance, args)
                }
            }
        };

        match tuple.get::<usize>(index.into()) {
            Some(value) => Ok(value.clone()),
            None => Ok(default.clone()),
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

    result.add_fn("sort_copy", |ctx| {
        let expected_error = "|Tuple|, or |Tuple, |Any| -> Any|";

        match ctx.instance_and_args(is_tuple, expected_error)? {
            (KValue::Tuple(t), []) => {
                let mut result = t.to_vec();

                sort_values(ctx.vm, &mut result)?;

                Ok(KValue::Tuple(result.into()))
            }
            (KValue::Tuple(t), [f]) if f.is_callable() => {
                let t = t.clone();
                let sorted = sort_by_key(ctx.vm, &t, f.clone())?;
                let result: Vec<_> = sorted.into_iter().map(|(_key, value)| value).collect();
                Ok(KValue::Tuple(result.into()))
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
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
