//! As operators can be overloaded, we can't treat values specifically (i.e. use `PartialOrd` and
//! `Ord` for example). So we always need to call operators to compare them. This module contains
//! helpers for comparing and sorting [Value].

use std::cmp::Ordering;

use crate::{external_error, type_as_string, Operator, RuntimeError, Value, Vm};

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

/// The same as [quick_sort], but sorts the `arr` by provided `key`.
///
/// Notice, this function doesn't check equality of sizes between arrays.
pub fn quick_sort_by_key(
    vm: &mut Vm,
    key: &mut [Value],
    arr: &mut [Value],
    start: usize,
    end: usize,
) -> Result<(), RuntimeError> {
    if start >= end {
        return Ok(());
    }

    let pivot = partition_with_key(vm, key, arr, start, end)?;

    if pivot < 1 {
        return Ok(());
    }

    quick_sort_by_key(vm, key, arr, start, pivot - 1)?;
    quick_sort_by_key(vm, key, arr, pivot + 1, end)?;

    Ok(())
}

fn partition_with_key(
    vm: &mut Vm,
    key: &mut [Value],
    arr: &mut [Value],
    start: usize,
    end: usize,
) -> Result<usize, RuntimeError> {
    let pivot = key[end].clone();
    let mut index = start;
    let mut i = start;

    while i < end {
        if let Ordering::Less = compare_values(vm, &key[i], &pivot)? {
            key.swap(i, index);
            arr.swap(i, index);
            index += 1;
        }

        i += 1;
    }

    key.swap(index, end);
    arr.swap(index, end);

    Ok(index)
}

/// Compares values using Koto operators.
pub fn compare_values(vm: &mut Vm, a: &Value, b: &Value) -> Result<Ordering, RuntimeError> {
    match vm.run_binary_op(Operator::Less, a.clone(), b.clone())? {
        Value::Bool(true) => Ok(Ordering::Less),
        Value::Bool(false) => match vm.run_binary_op(Operator::Greater, a.clone(), b.clone())? {
            Value::Bool(true) => Ok(Ordering::Greater),
            Value::Bool(false) => Ok(Ordering::Equal),
            unexpected => external_error!(
                "Expected Bool from comparison, found '{}'",
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
