//! The `range` core library module

use crate::prelude::*;

/// Initializes the `range` core library module
pub fn make_module() -> KMap {
    let result = KMap::with_type("core.range");

    result.add_fn("contains", |ctx| {
        let expected_error = "|Range, Number|, or |Range, Range|";

        match ctx.instance_and_args(is_range, expected_error)? {
            (KValue::Range(r), [KValue::Number(n)]) => Ok(r.contains(*n).into()),
            (KValue::Range(a), [KValue::Range(b)]) => {
                let r_a = a.as_sorted_range();
                let r_b = b.as_sorted_range();
                let result = r_b.start >= r_a.start && r_b.end <= r_a.end;
                Ok(result.into())
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("end", |ctx| {
        let expected_error = "|Range|";

        match ctx.instance_and_args(is_range, expected_error)? {
            (KValue::Range(r), []) => {
                Ok(r.end().map_or(KValue::Null, |(end, _inclusive)| end.into()))
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("expanded", |ctx| {
        let expected_error = "|Range, Number|";

        match ctx.instance_and_args(is_range, expected_error)? {
            (KValue::Range(r), [KValue::Number(n)]) => match (r.start(), r.end()) {
                (Some(start), Some((end, inclusive))) => {
                    let n = i64::from(n);
                    let result = if r.is_ascending() {
                        KRange::new(Some(start - n), Some((end + n, inclusive)))
                    } else {
                        KRange::new(Some(start + n), Some((end - n, inclusive)))
                    };
                    Ok(result.into())
                }
                _ => runtime_error!("range.expanded can't be used with '{r}'"),
            },
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("intersection", |ctx| {
        let expected_error = "|Range, Range|";

        match ctx.instance_and_args(is_range, expected_error)? {
            (KValue::Range(a), [KValue::Range(b)]) => Ok(a
                .intersection(b)
                .map_or(KValue::Null, |result| result.into())),
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("is_inclusive", |ctx| {
        let expected_error = "|Range|";

        match ctx.instance_and_args(is_range, expected_error)? {
            (KValue::Range(r), []) => {
                Ok(r.end().map_or(false, |(_end, inclusive)| inclusive).into())
            }
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("start", |ctx| {
        let expected_error = "|Range|";

        match ctx.instance_and_args(is_range, expected_error)? {
            (KValue::Range(r), []) => Ok(r.start().map_or(KValue::Null, KValue::from)),
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result.add_fn("union", |ctx| {
        let expected_error = "|Range, Number|, or |Range, Range|";

        match ctx.instance_and_args(is_range, expected_error)? {
            (KValue::Range(r), [KValue::Number(n)]) => {
                let n = i64::from(n);
                match (r.start(), r.end()) {
                    (Some(start), Some((end, inclusive))) => {
                        let result = if start <= end {
                            KRange::new(Some(start.min(n)), Some((end.max(n + 1), inclusive)))
                        } else {
                            KRange::new(Some(start.max(n)), Some((end.min(n - 1), inclusive)))
                        };
                        Ok(result.into())
                    }
                    _ => runtime_error!("range.union can't be used with '{r}'"),
                }
            }
            (KValue::Range(a), [KValue::Range(b)]) => match (a.start(), a.end()) {
                (Some(start), Some((end, inclusive))) => {
                    let r_b = b.as_sorted_range();
                    let result = if start <= end {
                        KRange::new(
                            Some(start.min(r_b.start)),
                            Some((end.max(r_b.end), inclusive)),
                        )
                    } else {
                        KRange::new(
                            Some(start.max(r_b.end - 1)),
                            Some((end.min(r_b.start), inclusive)),
                        )
                    };
                    Ok(result.into())
                }
                _ => runtime_error!("range.union can't be used with '{a}' and '{b}'"),
            },
            (instance, args) => unexpected_args_after_instance(expected_error, instance, args),
        }
    });

    result
}

fn is_range(value: &KValue) -> bool {
    matches!(value, KValue::Range(_))
}
