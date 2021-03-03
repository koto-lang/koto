use std::cmp::Ordering;

use crate::{Operator, RuntimeError, Value, Vm};

/// Used internally in sorting methods implementations.
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
        if is_less(vm, arr[i].clone(), pivot.clone())? {
            arr.swap(i, index);
            index += 1;
        }

        i += 1;
    }

    arr.swap(index, end);

    Ok(index)
}

fn is_less(vm: &mut Vm, a: Value, b: Value) -> Result<bool, RuntimeError> {
    match vm.run_binary_op(Operator::Less, a, b)? {
        Value::Bool(val) => Ok(val),
        _ => unreachable!(),
    }
}

/// Replace for `Ord::cmp`.
pub fn cmp(vm: &mut Vm, a: &Value, b: &Value) -> Result<Ordering, RuntimeError> {
    if let Value::Bool(true) = vm.run_binary_op(Operator::Equal, a.clone(), b.clone())? {
        return Ok(Ordering::Equal);
    }

    match vm.run_binary_op(Operator::Less, a.clone(), b.clone())? {
        Value::Bool(true) => Ok(Ordering::Less),
        Value::Bool(false) => Ok(Ordering::Greater),
        _ => unreachable!(),
    }
}
