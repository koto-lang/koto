//! As operators can be overridden, we can't treat values specifically (i.e. use `PartialOrd` and
//! `Ord` for example). So we always need to call operators to compare them. This module contains
//! helpers for comparing and sorting [Value].

use std::cmp::Ordering;

use crate::{AsyncKotoVm, BinaryOp, KValue, Result, runtime_error};

/// Sorts values in a vec using Koto operators for comparison, allowing comparisons to suspend.
pub async fn sort_values_async(vm: &mut AsyncKotoVm, arr: &mut [KValue]) -> Result<()> {
    for i in 1..arr.len() {
        let mut j = i;

        while j > 0 && compare_values_async(vm, &arr[j], &arr[j - 1]).await? == Ordering::Less {
            arr.swap(j, j - 1);
            j -= 1;
        }
    }

    Ok(())
}

/// Returns a sorted copy of a slice of values, compared using a key function.
///
/// Calls to the key function and comparisons between keys are allowed to suspend.
pub async fn sort_by_key_async(
    vm: &mut AsyncKotoVm,
    input: &[KValue],
    key_fn: KValue,
) -> Result<Vec<(KValue, KValue)>> {
    let mut keys_and_values = Vec::with_capacity(input.len());

    for value in input {
        let key = vm
            .call_function_with_arg(key_fn.clone(), value.clone())
            .await?;
        keys_and_values.push((key, value.clone()));
    }

    for i in 1..keys_and_values.len() {
        let mut j = i;

        while j > 0
            && compare_values_async(vm, &keys_and_values[j].0, &keys_and_values[j - 1].0).await?
                == Ordering::Less
        {
            keys_and_values.swap(j, j - 1);
            j -= 1;
        }
    }

    Ok(keys_and_values)
}

/// Compares values using Koto operators, allowing overridden operators to suspend.
pub async fn compare_values_async(
    vm: &mut AsyncKotoVm,
    a: &KValue,
    b: &KValue,
) -> Result<Ordering> {
    use KValue::Bool;

    match vm
        .run_binary_op(BinaryOp::Less, a.clone(), b.clone())
        .await?
    {
        Bool(true) => Ok(Ordering::Less),
        Bool(false) => {
            match vm
                .run_binary_op(BinaryOp::Greater, a.clone(), b.clone())
                .await?
            {
                Bool(true) => Ok(Ordering::Greater),
                Bool(false) => Ok(Ordering::Equal),
                unexpected => runtime_error!(
                    "Expected Bool from > comparison, found '{}'",
                    unexpected.type_as_string()
                ),
            }
        }
        unexpected => {
            runtime_error!(
                "Expected Bool from < comparison, found '{}'",
                unexpected.type_as_string()
            )
        }
    }
}
