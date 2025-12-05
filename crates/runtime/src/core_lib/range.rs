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
            (KValue::Range(r), []) => Ok(r.end().is_some_and(|(_end, inclusive)| inclusive).into()),
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

        let (a, b) = match ctx.instance_and_args(is_range, expected_error)? {
            (KValue::Range(a), [KValue::Number(n)]) => {
                let n: i64 = n.into();
                (a.clone(), KRange::from(n..n + 1))
            }
            (KValue::Range(a), [KValue::Range(b)]) => (a.clone(), b.clone()),
            (instance, args) => {
                return unexpected_args_after_instance(expected_error, instance, args);
            }
        };

        match (a.start(), a.end()) {
            (Some(_), Some((_, inclusive))) if b.is_bounded() => {
                let a_r = a.as_sorted_range();
                let b_r = b.as_sorted_range();
                let start = a_r.start.min(b_r.start);
                let end = a_r.end.max(b_r.end);

                let result = match (a.is_ascending(), inclusive) {
                    (true, true) => KRange::new(Some(start), Some((end - 1, true))),
                    (true, false) => KRange::new(Some(start), Some((end, false))),
                    (false, true) => KRange::new(Some(end - 1), Some((start, true))),
                    (false, false) => KRange::new(Some(end - 1), Some((start - 1, false))),
                };

                Ok(result.into())
            }
            _ => {
                runtime_error!("range.union can only be used with bounded ranges (a: {a}, b: {b})")
            }
        }
    });

    result
}

fn is_range(value: &KValue) -> bool {
    matches!(value, KValue::Range(_))
}
