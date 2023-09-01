//! The `range` core library module

use crate::prelude::*;

/// Initializes the `range` core library module
pub fn make_module() -> ValueMap {
    use Value::*;

    let result = ValueMap::with_type("core.range");

    result.add_fn("contains", |vm, args| match vm.get_args(args) {
        [Range(r), Number(n)] => Ok(r.contains(*n).into()),
        [Range(a), Range(b)] => {
            let r_a = a.as_sorted_range();
            let r_b = b.as_sorted_range();
            let result = r_b.start >= r_a.start && r_b.end <= r_a.end;
            Ok(result.into())
        }
        unexpected => {
            type_error_with_slice("a Range and a Number or Range as arguments", unexpected)
        }
    });

    result.add_fn("end", |vm, args| match vm.get_args(args) {
        [Range(r)] => Ok(r.end().map_or(Null, |(end, _inclusive)| end.into())),
        unexpected => type_error_with_slice("a Range as argument", unexpected),
    });

    result.add_fn("expanded", |vm, args| match vm.get_args(args) {
        [Range(r), Number(n)] => match (r.start(), r.end()) {
            (Some(start), Some((end, inclusive))) => {
                let n = i64::from(n);
                let result = if r.is_ascending() {
                    IntRange::bounded(start - n, end + n, inclusive)
                } else {
                    IntRange::bounded(start + n, end - n, inclusive)
                };
                Ok(result.into())
            }
            _ => runtime_error!("range.expanded can't be used with '{r}'"),
        },
        unexpected => type_error_with_slice("a Range and Number as arguments", unexpected),
    });

    result.add_fn("intersection", |vm, args| match vm.get_args(args) {
        [Range(a), Range(b)] => Ok(a.intersection(b).map_or(Null, |result| result.into())),
        unexpected => type_error_with_slice("two Ranges", unexpected),
    });

    result.add_fn("is_inclusive", |vm, args| match vm.get_args(args) {
        [Range(r)] => Ok(r.end().map_or(false, |(_end, inclusive)| inclusive).into()),
        unexpected => type_error_with_slice("a Range as argument", unexpected),
    });

    result.add_fn("size", |vm, args| match vm.get_args(args) {
        [Range(r)] => match r.size() {
            Some(size) => Ok(size.into()),
            None => runtime_error!("range.size can't be used with '{r}'"),
        },
        unexpected => type_error_with_slice("a Range as argument", unexpected),
    });

    result.add_fn("start", |vm, args| match vm.get_args(args) {
        [Range(r)] => Ok(r.start().map_or(Null, Value::from)),
        unexpected => type_error_with_slice("a Range as argument", unexpected),
    });

    result.add_fn("union", |vm, args| match vm.get_args(args) {
        [Range(r), Number(n)] => {
            let n = i64::from(n);
            match (r.start(), r.end()) {
                (Some(start), Some((end, inclusive))) => {
                    let result = if start <= end {
                        IntRange::bounded(start.min(n), end.max(n + 1), inclusive)
                    } else {
                        IntRange::bounded(start.max(n), end.min(n - 1), inclusive)
                    };
                    Ok(result.into())
                }
                _ => runtime_error!("range.union can't be used with '{r}'"),
            }
        }
        [Range(a), Range(b)] => match (a.start(), a.end()) {
            (Some(start), Some((end, inclusive))) => {
                let r_b = b.as_sorted_range();
                let result = if start <= end {
                    IntRange::bounded(start.min(r_b.start), end.max(r_b.end), inclusive)
                } else {
                    IntRange::bounded(start.max(r_b.end - 1), end.min(r_b.start), inclusive)
                };
                Ok(result.into())
            }
            _ => runtime_error!("range.union can't be used with '{a}' and '{b}'"),
        },
        unexpected => type_error_with_slice(
            "a Range and another Range or a Number as arguments",
            unexpected,
        ),
    });

    result
}
