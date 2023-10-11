//! The `tuple` core library module

use super::value_sort::sort_values;
use crate::prelude::*;

/// Initializes the `tuple` core library module
pub fn make_module() -> KMap {
    let result = KMap::with_type("core.tuple");

    result.add_fn("contains", |ctx| {
        let expected_error = "a Tuple and a Value";

        match ctx.instance_and_args(is_tuple, expected_error)? {
            (Value::Tuple(t), [value]) => {
                let t = t.clone();
                let value = value.clone();
                for candidate in t.iter() {
                    match ctx
                        .vm
                        .run_binary_op(BinaryOp::Equal, value.clone(), candidate.clone())
                    {
                        Ok(Value::Bool(false)) => {}
                        Ok(Value::Bool(true)) => return Ok(true.into()),
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
            (Value::Tuple(t), []) => match t.first() {
                Some(value) => Ok(value.clone()),
                None => Ok(Value::Null),
            },
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("get", |ctx| {
        let (tuple, index, default) = {
            let expected_error = "a Tuple and Number (with optional default Value)";

            match ctx.instance_and_args(is_tuple, expected_error)? {
                (Value::Tuple(tuple), [Value::Number(n)]) => (tuple, n, &Value::Null),
                (Value::Tuple(tuple), [Value::Number(n), default]) => (tuple, n, default),
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
            (Value::Tuple(t), []) => match t.last() {
                Some(value) => Ok(value.clone()),
                None => Ok(Value::Null),
            },
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("size", |ctx| {
        let expected_error = "a Tuple";

        match ctx.instance_and_args(is_tuple, expected_error)? {
            (Value::Tuple(t), []) => Ok(Value::Number(t.len().into())),
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("sort_copy", |ctx| {
        let expected_error = "a Tuple";

        match ctx.instance_and_args(is_tuple, expected_error)? {
            (Value::Tuple(t), []) => {
                let mut result = t.to_vec();

                sort_values(ctx.vm, &mut result)?;

                Ok(Value::Tuple(result.into()))
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("to_list", |ctx| {
        let expected_error = "a Tuple";

        match ctx.instance_and_args(is_tuple, expected_error)? {
            (Value::Tuple(t), []) => Ok(Value::List(ValueList::from_slice(t))),
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result
}

fn is_tuple(value: &Value) -> bool {
    matches!(value, Value::Tuple(_))
}
