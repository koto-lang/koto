//! As operators can be overridden, we can't treat values specifically (i.e. use `PartialOrd` and
//! `Ord` for example). So we always need to call operators to compare them. This module contains
//! helpers for comparing and sorting [Value].

use std::cmp::Ordering;

use crate::{runtime_error, BinaryOp, Error, KValue, KotoVm};

/// Sorts values in a slice using Koto operators for comparison.
pub fn sort_values(vm: &mut KotoVm, arr: &mut [KValue]) -> Result<(), Error> {
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

/// Returns a sorted copy of a slice of values, compared using a key function
///
/// The returned data is a sorted vec of key/value pairs, sorted by key.
///
/// Used by list.sort and tuple.sort_copy
pub fn sort_by_key(
    vm: &mut KotoVm,
    input: &[KValue],
    key_fn: KValue,
) -> Result<Vec<(KValue, KValue)>, Error> {
    // Build up a vec of key/value pairs by calling key_fn for each value
    let mut keys_and_values: Vec<(KValue, KValue)> = input
        .iter()
        .map(|value| {
            vm.call_function(key_fn.clone(), value.clone())
                .map(|key| (key, value.clone()))
        })
        .collect::<Result<_, _>>()?;

    // Sort the data by key
    let mut error = None;
    keys_and_values.sort_by(|a, b| {
        // If an error has occurred then short-circuit the sorting to exit as quickly as possible
        if error.is_some() {
            return Ordering::Equal;
        }

        match compare_values(vm, &a.0, &b.0) {
            Ok(ordering) => ordering,
            Err(e) => {
                error = Some(e);
                Ordering::Equal
            }
        }
    });

    if let Some(error) = error {
        Err(error)
    } else {
        Ok(keys_and_values)
    }
}

/// Compares values using Koto operators.
pub fn compare_values(vm: &mut KotoVm, a: &KValue, b: &KValue) -> Result<Ordering, Error> {
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
