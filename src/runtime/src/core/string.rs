mod format;

use crate::{
    external_error,
    value_iterator::{ValueIterator, ValueIteratorOutput},
    Value, ValueMap,
};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("contains", |vm, args| match vm.get_args(args) {
        [Str(s1), Str(s2)] => Ok(Bool(s1.contains(s2.as_str()))),
        _ => external_error!("string.contains: Expected two strings as arguments"),
    });

    result.add_fn("escape", |vm, args| match vm.get_args(args) {
        [Str(s)] => Ok(Str(s.escape_default().to_string().into())),
        _ => external_error!("string.escape: Expected string as argument"),
    });

    result.add_fn("is_empty", |vm, args| match vm.get_args(args) {
        [Str(s)] => Ok(Bool(s.is_empty())),
        _ => external_error!("string.is_empty: Expected string as argument"),
    });

    result.add_fn("format", |vm, args| match vm.get_args(args) {
        [result @ Str(_)] => Ok(result.clone()),
        [Str(format), format_args @ ..] => match format::format_string(format, format_args) {
            Ok(result) => Ok(Str(result.into())),
            Err(error) => external_error!("string.format: {}", error),
        },
        _ => external_error!("string.format: Expected a string as first argument"),
    });

    result.add_fn("lines", |vm, args| match vm.get_args(args) {
        [Str(s)] => {
            let input = s.clone();

            let mut start = 0;

            let iterator = ValueIterator::make_external(move || {
                if start < input.len() {
                    let end = match input[start..].find('\n') {
                        Some(end) => {
                            if end > start && input.as_bytes()[end - 1] == b'\r' {
                                start + end - 1
                            } else {
                                start + end
                            }
                        }
                        None => input.len(),
                    };

                    let result = Str(input.with_bounds(start..end));
                    start = end + 1;
                    Some(Ok(ValueIteratorOutput::Value(result)))
                } else {
                    None
                }
            });

            Ok(Iterator(iterator))
        }
        _ => external_error!("string.lines: Expected string as argument"),
    });

    result.add_fn("slice", |vm, args| match vm.get_args(args) {
        [Str(input), Number(from)] => {
            let bounds = (*from as usize)..input.len();
            let result = if input.get(bounds.clone()).is_some() {
                Str(input.with_bounds(bounds))
            } else {
                Empty
            };
            Ok(result)
        }
        [Str(input), Number(from), Number(to)] => {
            let bounds = (*from as usize)..(*to as usize);
            let result = if input.get(bounds.clone()).is_some() {
                Str(input.with_bounds(bounds))
            } else {
                Empty
            };
            Ok(result)
        }
        _ => external_error!("string.slice: Expected a string and slice index as arguments"),
    });

    result.add_fn("split", |vm, args| match vm.get_args(args) {
        [Str(input), Str(pattern)] => {
            let input = input.clone();
            let pattern = pattern.clone();

            let mut start = 0;
            let iterator = ValueIterator::make_external(move || {
                if start <= input.len() {
                    let end = match input[start..].find(pattern.as_str()) {
                        Some(end) => start + end,
                        None => input.len(),
                    };

                    let result = Str(input.with_bounds(start..end));
                    start = end + 1;
                    Some(Ok(ValueIteratorOutput::Value(result)))
                } else {
                    None
                }
            });

            Ok(Iterator(iterator))
        }
        _ => external_error!("string.split: Expected two strings as arguments"),
    });

    result.add_fn("to_number", |vm, args| match vm.get_args(args) {
        [Str(s)] => match s.parse::<f64>() {
            Ok(n) => Ok(Number(n)),
            Err(_) => external_error!("string.to_number: Failed to convert '{}'", s),
        },
        _ => external_error!("string.to_number: Expected string as argument"),
    });

    result.add_fn("trim", |vm, args| match vm.get_args(args) {
        [Str(s)] => {
            let result = match s.find(|c: char| !c.is_whitespace()) {
                Some(start) => {
                    let end = s.rfind(|c: char| !c.is_whitespace()).unwrap();
                    s.with_bounds(start..(end + 1))
                }
                None => s.with_bounds(0..0),
            };

            Ok(Str(result))
        }
        _ => external_error!("string.trim: Expected string as argument"),
    });

    result
}
