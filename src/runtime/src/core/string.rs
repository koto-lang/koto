mod format;

use {
    crate::{
        external_error,
        value_iterator::{ValueIterator, ValueIteratorOutput},
        Value, ValueMap,
    },
    unicode_segmentation::UnicodeSegmentation,
};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("chars", |vm, args| match vm.get_args(args) {
        [Str(s)] => Ok(Iterator(ValueIterator::with_string(s.clone()))),
        _ => external_error!("string.chars: Expected a string as argument"),
    });

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

    result.add_fn("ends_with", |vm, args| match vm.get_args(args) {
        [Str(s), Str(pattern)] => {
            let result = s.as_str().ends_with(pattern.as_str());
            Ok(Bool(result))
        }
        _ => external_error!("string.ends_with: Expected two strings as arguments"),
    });

    result.add_fn("format", |vm, args| match vm.get_args(args) {
        [result @ Str(_)] => Ok(result.clone()),
        [Str(format), format_args @ ..] => {
            let format = format.clone();
            let format_args = format_args.to_vec();
            let vm = vm.child_vm();
            match format::format_string(vm, &format, &format_args) {
                Ok(result) => Ok(Str(result.into())),
                Err(error) => Err(error.with_prefix("string.format")),
            }
        }
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

                    let result = Str(input.with_bounds(start..end).unwrap());
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

    result.add_fn("print", |vm, args| {
        match vm.get_args(args) {
            [Str(s)] => vm.logger().writeln(s.as_str()),
            [Str(format), format_args @ ..] => {
                let format = format.clone();
                let format_args = format_args.to_vec();
                let vm = vm.child_vm();
                match format::format_string(vm, &format, &format_args) {
                    Ok(result) => vm.logger().writeln(&result),
                    Err(error) => return Err(error.with_prefix("string.print")),
                }
            }
            _ => return external_error!("string.print: Expected a string as first argument"),
        }
        Ok(Empty)
    });

    result.add_fn("size", |vm, args| match vm.get_args(args) {
        [Str(s)] => Ok(Number(s.graphemes(true).count().into())),
        _ => external_error!("string.size: Expected string as argument"),
    });

    result.add_fn("slice", |vm, args| match vm.get_args(args) {
        [Str(input), Number(from)] => {
            let bounds = usize::from(*from)..input.len();
            let result = match input.with_bounds(bounds) {
                Ok(result) => Str(result),
                Err(_) => Empty,
            };
            Ok(result)
        }
        [Str(input), Number(from), Number(to)] => {
            let bounds = usize::from(*from)..usize::from(*to);
            let result = match input.with_bounds(bounds) {
                Ok(result) => Str(result),
                Err(_) => Empty,
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
            let pattern_len = pattern.len();
            let iterator = ValueIterator::make_external(move || {
                if start <= input.len() {
                    let end = match input[start..].find(pattern.as_str()) {
                        Some(end) => start + end,
                        None => input.len(),
                    };

                    let result = Str(input.with_bounds(start..end).unwrap());
                    start = end + pattern_len;
                    Some(Ok(ValueIteratorOutput::Value(result)))
                } else {
                    None
                }
            });

            Ok(Iterator(iterator))
        }
        _ => external_error!("string.split: Expected two strings as arguments"),
    });

    result.add_fn("starts_with", |vm, args| match vm.get_args(args) {
        [Str(s), Str(pattern)] => {
            let result = s.as_str().starts_with(pattern.as_str());
            Ok(Bool(result))
        }
        _ => external_error!("string.starts_with: Expected two strings as arguments"),
    });

    result.add_fn("to_lowercase", |vm, args| match vm.get_args(args) {
        [Str(s)] => {
            let result = s.chars().flat_map(|c| c.to_lowercase()).collect::<String>();
            Ok(Str(result.into()))
        }
        _ => external_error!("string.to_lowercase: Expected string as argument"),
    });

    result.add_fn("to_number", |vm, args| match vm.get_args(args) {
        [Str(s)] => match s.parse::<i64>() {
            Ok(n) => Ok(Number(n.into())),
            Err(_) => match s.parse::<f64>() {
                Ok(n) => Ok(Number(n.into())),
                Err(_) => {
                    external_error!("string.to_number: Failed to convert '{}'", s)
                }
            },
        },
        _ => external_error!("string.to_number: Expected string as argument"),
    });

    result.add_fn("to_uppercase", |vm, args| match vm.get_args(args) {
        [Str(s)] => {
            let result = s.chars().flat_map(|c| c.to_uppercase()).collect::<String>();
            Ok(Str(result.into()))
        }
        _ => external_error!("string.to_uppercase: Expected string as argument"),
    });

    result.add_fn("trim", |vm, args| match vm.get_args(args) {
        [Str(s)] => {
            let result = match s.find(|c: char| !c.is_whitespace()) {
                Some(start) => {
                    let end = s.rfind(|c: char| !c.is_whitespace()).unwrap();
                    s.with_bounds(start..(end + 1)).unwrap()
                }
                None => s.with_bounds(0..0).unwrap(),
            };

            Ok(Str(result))
        }
        _ => external_error!("string.trim: Expected string as argument"),
    });

    result
}
