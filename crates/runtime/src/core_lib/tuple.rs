//! The `tuple` core library module

use super::value_sort::{sort_by_key, sort_values};
use crate::prelude::*;

/// Initializes the `tuple` core library module
pub fn make_module() -> KMap {
    let result = KMap::with_type("core.tuple");

    result.add_fn("contains", |ctx| {
        let expected_error = "a Tuple and a Value";

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
                            return type_error_with_slice(
                                "a Bool from the equality comparison",
                                &[unexpected],
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

    result.add_fn("first", |ctx| {
        let expected_error = "a Tuple";

        match ctx.instance_and_args(is_tuple, expected_error)? {
            (KValue::Tuple(t), []) => match t.first() {
                Some(value) => Ok(value.clone()),
                None => Ok(KValue::Null),
            },
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("get", |ctx| {
        let (tuple, index, default) = {
            let expected_error = "a Tuple and Number (with optional default Value)";

            match ctx.instance_and_args(is_tuple, expected_error)? {
                (KValue::Tuple(tuple), [KValue::Number(n)]) => (tuple, n, &KValue::Null),
                (KValue::Tuple(tuple), [KValue::Number(n), default]) => (tuple, n, default),
                (_, unexpected) => return type_error_with_slice(expected_error, unexpected),
            }
        };

        match tuple.get::<usize>(index.into()) {
            Some(value) => Ok(value.clone()),
            None => Ok(default.clone()),
        }
    });

    result.add_fn("last", |ctx| {
        let expected_error = "a Tuple";

        match ctx.instance_and_args(is_tuple, expected_error)? {
            (KValue::Tuple(t), []) => match t.last() {
                Some(value) => Ok(value.clone()),
                None => Ok(KValue::Null),
            },
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("sort_copy", |ctx| {
        let expected_error = "a Tuple, and an optional key function";

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
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("to_list", |ctx| {
        let expected_error = "a Tuple";

        match ctx.instance_and_args(is_tuple, expected_error)? {
            (KValue::Tuple(t), []) => Ok(KValue::List(KList::from_slice(t))),
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result
}

fn is_tuple(value: &KValue) -> bool {
    matches!(value, KValue::Tuple(_))
}
