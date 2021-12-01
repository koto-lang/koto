use crate::{unexpected_type_error_with_slice, IntRange, Value, ValueMap};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("contains", |vm, args| match vm.get_args(args) {
        [Range(r), Number(n)] => Ok(Bool(*n >= r.start && n.ceil() < r.end)),
        unexpected => unexpected_type_error_with_slice(
            "range.contains",
            "a Range and Number as arguments",
            unexpected,
        ),
    });

    result.add_fn("end", |vm, args| match vm.get_args(args) {
        [Range(r)] => Ok(Number(r.end.into())),
        unexpected => {
            unexpected_type_error_with_slice("range.end", "a Range as argument", unexpected)
        }
    });

    result.add_fn("expanded", |vm, args| match vm.get_args(args) {
        [Range(r), Number(n)] => {
            let n = isize::from(n);
            if r.is_ascending() {
                Ok(Range(IntRange {
                    start: r.start - n,
                    end: r.end + n,
                }))
            } else {
                Ok(Range(IntRange {
                    start: r.start + n,
                    end: r.end - n,
                }))
            }
        }
        unexpected => unexpected_type_error_with_slice(
            "range.expanded",
            "a Range and Number as arguments",
            unexpected,
        ),
    });

    result.add_fn("size", |vm, args| match vm.get_args(args) {
        [Range(r)] => Ok(Number((r.end - r.start).into())),
        unexpected => {
            unexpected_type_error_with_slice("range.size", "a Range as argument", unexpected)
        }
    });

    result.add_fn("start", |vm, args| match vm.get_args(args) {
        [Range(r)] => Ok(Number(r.start.into())),
        unexpected => {
            unexpected_type_error_with_slice("range.start", "a Range as argument", unexpected)
        }
    });

    result.add_fn("union", |vm, args| match vm.get_args(args) {
        [Range(r), Number(n)] => {
            let n = isize::from(n);
            if r.is_ascending() {
                Ok(Range(IntRange {
                    start: r.start.min(n),
                    end: r.end.max(n + 1),
                }))
            } else {
                Ok(Range(IntRange {
                    start: r.start.max(n),
                    end: r.end.min(n - 1),
                }))
            }
        }
        [Range(a), Range(b)] => {
            let result = match (a.is_ascending(), b.is_ascending()) {
                (true, true) => Range(IntRange {
                    start: a.start.min(b.start),
                    end: a.end.max(b.end),
                }),
                (true, false) => Range(IntRange {
                    start: a.start.min(b.end + 1),
                    end: a.end.max(b.start + 1),
                }),
                (false, true) => Range(IntRange {
                    start: a.start.max(b.end - 1),
                    end: a.end.min(b.start),
                }),
                (false, false) => Range(IntRange {
                    start: a.start.max(b.start),
                    end: a.end.min(b.end),
                }),
            };

            Ok(result)
        }
        unexpected => unexpected_type_error_with_slice(
            "range.union",
            "a Range and another Range or a Number as arguments",
            unexpected,
        ),
    });

    result
}
