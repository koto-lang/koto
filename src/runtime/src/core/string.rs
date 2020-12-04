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
        [Str(s)] => Ok(Iterator(ValueIterator::make_external({
            let mut cluster_start = 0;
            let s = s.clone();

            move || match s[cluster_start..].grapheme_indices(true).next() {
                Some((_, cluster)) => {
                    let cluster_end = cluster_start + cluster.len();

                    let result = match s.with_bounds(cluster_start..cluster_end) {
                        Ok(result) => {
                            cluster_start = cluster_end;
                            Ok(ValueIteratorOutput::Value(Str(result)))
                        }
                        Err(_) => external_error!("string.chars: Failed to produce a substring"),
                    };
                    Some(result)
                }
                None => None,
            }
        }))),
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
            [Str(s)] => println!("{}", s.as_str()),
            [Str(format), format_args @ ..] => match format::format_string(format, format_args) {
                Ok(result) => println!("{}", result.as_str()),
                Err(error) => return external_error!("string.print: {}", error),
            },
            _ => return external_error!("string.print: Expected a string as first argument"),
        }
        Ok(Empty)
    });

    result.add_fn("size", |vm, args| match vm.get_args(args) {
        [Str(s)] => Ok(Number(s.graphemes(true).count() as f64)),
        _ => external_error!("string.size: Expected string as argument"),
    });

    result.add_fn("slice", |vm, args| match vm.get_args(args) {
        [Str(input), Number(from)] => {
            let bounds = (*from as usize)..input.len();
            let result = match input.with_bounds(bounds) {
                Ok(result) => Str(result),
                Err(_) => Empty,
            };
            Ok(result)
        }
        [Str(input), Number(from), Number(to)] => {
            let bounds = (*from as usize)..(*to as usize);
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
            let iterator = ValueIterator::make_external(move || {
                if start <= input.len() {
                    let end = match input[start..].find(pattern.as_str()) {
                        Some(end) => start + end,
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
        _ => external_error!("string.split: Expected two strings as arguments"),
    });

    result.add_fn("to_lowercase", |vm, args| match vm.get_args(args) {
        [Str(s)] => {
            let result = s.chars().flat_map(|c| c.to_lowercase()).collect::<String>();
            Ok(Str(result.into()))
        }
        _ => external_error!("string.to_lowercase: Expected string as argument"),
    });

    result.add_fn("to_number", |vm, args| match vm.get_args(args) {
        [Str(s)] => match s.parse::<f64>() {
            Ok(n) => Ok(Number(n)),
            Err(_) => external_error!("string.to_number: Failed to convert '{}'", s),
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
