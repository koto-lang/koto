//! As operators can be overloaded, we can't treat values specifically (i.e. use `PartialOrd` and
//! `Ord` for example). So we always need to call operators to compare them. This module contains
//! helpers for comparing and sorting [Value].

use std::cmp::Ordering;

use crate::{external_error, type_as_string, Operator, RuntimeError, Value, Vm};

/// An implementation of the quick sort algorithm for a slice of [Value].
pub fn quick_sort(
    vm: &mut Vm,
    arr: &mut [Value],
    start: usize,
    end: usize,
) -> Result<(), RuntimeError> {
    if start >= end {
        return Ok(());
    }

    let pivot = partition(vm, arr, start, end)?;

    if pivot < 1 {
        return Ok(());
    }

    quick_sort(vm, arr, start, pivot - 1)?;
    quick_sort(vm, arr, pivot + 1, end)?;

    Ok(())
}

fn partition(
    vm: &mut Vm,
    arr: &mut [Value],
    start: usize,
    end: usize,
) -> Result<usize, RuntimeError> {
    let pivot = arr[end].clone();
    let mut index = start;
    let mut i = start;

    while i < end {
        if let Ordering::Less = cmp(vm, &arr[i], &pivot)? {
            arr.swap(i, index);
            index += 1;
        }

        i += 1;
    }

    arr.swap(index, end);

    Ok(index)
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
        if let Ordering::Less = cmp(vm, &key[i], &pivot)? {
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

/// Replaces `Ord::cmp` in implementations where `Ordering` needed.
pub fn cmp(vm: &mut Vm, a: &Value, b: &Value) -> Result<Ordering, RuntimeError> {
    match vm.run_binary_op(Operator::Equal, a.clone(), b.clone())? {
        Value::Bool(true) => return Ok(Ordering::Equal),
        Value::Bool(false) => (),
        unexpected => {
            return external_error!(
                "iterator.min: Expected Bool from == comparison, found '{}'",
                type_as_string(&unexpected)
            );
        }
    }

    match vm.run_binary_op(Operator::Less, a.clone(), b.clone())? {
        Value::Bool(true) => Ok(Ordering::Less),
        Value::Bool(false) => Ok(Ordering::Greater),
        unexpected => external_error!(
            "iterator.min: Expected Bool from < comparison, found '{}'",
            type_as_string(&unexpected)
        ),
    }
}
