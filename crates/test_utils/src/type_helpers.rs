use koto_runtime::prelude::*;
use std::ops::RangeBounds;

/// Returns a KValue::List from a slice of integers
pub fn number_list<T>(values: &[T]) -> KValue
where
    T: Copy,
    i64: From<T>,
{
    let values = values
        .iter()
        .map(|n| i64::from(*n).into())
        .collect::<Vec<_>>();
    list(&values)
}

/// Returns a KValue::Tuple from a slice of integers
pub fn number_tuple<T>(values: &[T]) -> KValue
where
    T: Copy,
    i64: From<T>,
{
    let values = values
        .iter()
        .map(|n| i64::from(*n).into())
        .collect::<Vec<_>>();
    tuple(&values)
}

/// Returns a KValue::List from a slice of KValues
pub fn list(values: &[KValue]) -> KValue {
    KList::from_slice(values).into()
}

/// Returns a KValue::Tuple from a slice of KValues
pub fn tuple(values: &[KValue]) -> KValue {
    KTuple::from(values).into()
}

/// Returns a KValue::Range from given bounds
pub fn range(bounds: impl RangeBounds<i64>) -> KValue {
    KRange::from(bounds).into()
}
