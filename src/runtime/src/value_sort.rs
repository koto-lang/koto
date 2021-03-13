//! As operators can be overloaded, we can't treat values specifically (i.e. use `PartialOrd` and
//! `Ord` for example). So we always need to call operators to compare them. This module contains
//! helpers for comparing and sorting [Value].

use std::cmp::Ordering;

use crate::{external_error, type_as_string, BinaryOp, RuntimeError, Value, Vm};

/// Sorts values in a slice using Koto operators for comparison.
pub fn sort_values(vm: &mut Vm, arr: &mut [Value]) -> Result<(), RuntimeError> {
    let mut error = None;

    arr.sort_by(|a, b| {
        if error.is_some() {
            return Ordering::Equal;
        }

        match compare_values(vm, a, b) {
            Ok(ordering) => ordering,
            Err(e) => {
                error.get_or_insert(e);
                Ordering::Equal
            }
        }
    });

    if let Some(err) = error {
        return Err(err);
    }

    Ok(())
}

/// Compares values using Koto operators.
pub fn compare_values(vm: &mut Vm, a: &Value, b: &Value) -> Result<Ordering, RuntimeError> {
    match vm.run_binary_op(BinaryOp::Less, a.clone(), b.clone())? {
        Value::Bool(true) => Ok(Ordering::Less),
        Value::Bool(false) => match vm.run_binary_op(BinaryOp::Greater, a.clone(), b.clone())? {
            Value::Bool(true) => Ok(Ordering::Greater),
            Value::Bool(false) => Ok(Ordering::Equal),
            unexpected => external_error!(
                "Expected Bool from > comparison, found '{}'",
                type_as_string(&unexpected)
            ),
        },
        unexpected => {
            external_error!(
                "Expected Bool from < comparison, found '{}'",
                type_as_string(&unexpected)
            )
        }
    }
}
