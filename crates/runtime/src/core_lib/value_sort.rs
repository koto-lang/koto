//! As operators can be overloaded, we can't treat values specifically (i.e. use `PartialOrd` and
//! `Ord` for example). So we always need to call operators to compare them. This module contains
//! helpers for comparing and sorting [Value].

use std::cmp::Ordering;

use crate::{runtime_error, BinaryOp, Error, KValue, Vm};

/// Sorts values in a slice using Koto operators for comparison.
pub fn sort_values(vm: &mut Vm, arr: &mut [KValue]) -> Result<(), Error> {
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
pub fn compare_values(vm: &mut Vm, a: &KValue, b: &KValue) -> Result<Ordering, Error> {
    use KValue::Bool;

    match vm.run_binary_op(BinaryOp::Less, a.clone(), b.clone())? {
        Bool(true) => Ok(Ordering::Less),
        Bool(false) => match vm.run_binary_op(BinaryOp::Greater, a.clone(), b.clone())? {
            Bool(true) => Ok(Ordering::Greater),
            Bool(false) => Ok(Ordering::Equal),
            unexpected => runtime_error!(
                "Expected Bool from > comparison, found '{}'",
                unexpected.type_as_string()
            ),
        },
        unexpected => {
            runtime_error!(
                "Expected Bool from < comparison, found '{}'",
                unexpected.type_as_string()
            )
        }
    }
}
