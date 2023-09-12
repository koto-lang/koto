//! The `string` core library module

pub mod format;
pub mod iterators;

use super::iterator::collect_pair;
use crate::prelude::*;
use std::convert::TryFrom;
use unicode_segmentation::UnicodeSegmentation;

/// Initializes the `string` core library module
pub fn make_module() -> ValueMap {
    use Value::*;

    let result = ValueMap::with_type("core.string");

    result.add_fn("bytes", |ctx| {
        let expected_error = "a String";

        match ctx.instance_and_args(is_string, expected_error)? {
            (Str(s), []) => {
                let result = iterators::Bytes::new(s.clone());
                Ok(ValueIterator::new(result).into())
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("chars", |ctx| {
        let expected_error = "a String";

        match ctx.instance_and_args(is_string, expected_error)? {
            (Str(s), []) => Ok(Iterator(ValueIterator::with_string(s.clone()))),
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("contains", |ctx| {
        let expected_error = "a String";

        match ctx.instance_and_args(is_string, expected_error)? {
            (Str(s1), [Str(s2)]) => Ok(s1.contains(s2.as_str()).into()),
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("ends_with", |ctx| {
        let expected_error = "a String";

        match ctx.instance_and_args(is_string, expected_error)? {
            (Str(s), [Str(pattern)]) => Ok(s.as_str().ends_with(pattern.as_str()).into()),
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("escape", |ctx| {
        let expected_error = "a String";

        match ctx.instance_and_args(is_string, expected_error)? {
            (Str(s), []) => Ok(s.escape_default().to_string().into()),
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("format", |ctx| {
        let expected_error = "a String optionally followed by additional values";

        match ctx.instance_and_args(is_string, expected_error)? {
            (Str(s), []) => Ok(Str(s.clone())),
            (Str(format), format_args) => {
                let format = format.clone();
                let format_args = format_args.to_vec();
                match format::format_string(ctx.vm, &format, &format_args) {
                    Ok(result) => Ok(result.into()),
                    Err(error) => Err(error),
                }
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("from_bytes", |ctx| match ctx.args() {
        [iterable] if iterable.is_iterable() => {
            let iterable = iterable.clone();
            let iterator = ctx.vm.make_iterator(iterable)?;
            let (size_hint, _) = iterator.size_hint();
            let mut bytes = Vec::<u8>::with_capacity(size_hint);

            for output in iterator.map(collect_pair) {
                use ValueIteratorOutput as Output;
                match output {
                    Output::Value(Number(n)) => match u8::try_from(n.as_i64()) {
                        Ok(byte) => bytes.push(byte),
                        Err(_) => return runtime_error!("'{n}' is out of the valid byte range"),
                    },
                    Output::Value(unexpected) => return type_error("a number", &unexpected),
                    Output::Error(error) => return Err(error),
                    _ => unreachable!(),
                }
            }

            match String::from_utf8(bytes) {
                Ok(result) => Ok(result.into()),
                Err(_) => runtime_error!("Input failed UTF-8 validation"),
            }
        }
        unexpected => type_error_with_slice("an iterable", unexpected),
    });

    result.add_fn("is_empty", |ctx| {
        let expected_error = "a String";

        match ctx.instance_and_args(is_string, expected_error)? {
            (Str(s), []) => Ok(s.is_empty().into()),
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("lines", |ctx| {
        let expected_error = "a String";

        match ctx.instance_and_args(is_string, expected_error)? {
            (Str(s), []) => {
                let result = iterators::Lines::new(s.clone());
                Ok(ValueIterator::new(result).into())
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("replace", |ctx| {
        let expected_error = "a String, followed by pattern and replacement Strings";

        match ctx.instance_and_args(is_string, expected_error)? {
            (Str(input), [Str(pattern), Str(replace)]) => {
                Ok(input.replace(pattern.as_str(), replace).into())
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("size", |ctx| {
        let expected_error = "a String";

        match ctx.instance_and_args(is_string, expected_error)? {
            (Str(s), []) => Ok(s.graphemes(true).count().into()),
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("split", |ctx| {
        let iterator = {
            let expected_error = "a String, and either a String or a predicate function";

            match ctx.instance_and_args(is_string, expected_error)? {
                (Str(input), [Str(pattern)]) => {
                    let result = iterators::Split::new(input.clone(), pattern.clone());
                    ValueIterator::new(result)
                }
                (Str(input), [predicate]) if predicate.is_callable() => {
                    let result = iterators::SplitWith::new(
                        input.clone(),
                        predicate.clone(),
                        ctx.vm.spawn_shared_vm(),
                    );
                    ValueIterator::new(result)
                }
                (_, unexpected) => return type_error_with_slice(expected_error, unexpected),
            }
        };

        Ok(Iterator(iterator))
    });

    result.add_fn("starts_with", |ctx| {
        let expected_error = "two Strings";

        match ctx.instance_and_args(is_string, expected_error)? {
            (Str(s), [Str(pattern)]) => Ok(s.as_str().starts_with(pattern.as_str()).into()),
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("to_lowercase", |ctx| {
        let expected_error = "a String";

        match ctx.instance_and_args(is_string, expected_error)? {
            (Str(s), []) => {
                let result = s.chars().flat_map(|c| c.to_lowercase()).collect::<String>();
                Ok(result.into())
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("to_number", |ctx| {
        let expected_error = "a String";

        match ctx.instance_and_args(is_string, expected_error)? {
            (Str(s), []) => match s.parse::<i64>() {
                Ok(n) => Ok(Number(n.into())),
                Err(_) => match s.parse::<f64>() {
                    Ok(n) => Ok(Number(n.into())),
                    Err(_) => {
                        runtime_error!("string.to_number: Failed to convert '{s}'")
                    }
                },
            },
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("to_uppercase", |ctx| {
        let expected_error = "a String";

        match ctx.instance_and_args(is_string, expected_error)? {
            (Str(s), []) => {
                let result = s.chars().flat_map(|c| c.to_uppercase()).collect::<String>();
                Ok(result.into())
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result.add_fn("trim", |ctx| {
        let expected_error = "a String";

        match ctx.instance_and_args(is_string, expected_error)? {
            (Str(s), []) => {
                let result = match s.find(|c: char| !c.is_whitespace()) {
                    Some(start) => {
                        let end = s.rfind(|c: char| !c.is_whitespace()).unwrap();
                        s.with_bounds(start..(end + 1)).unwrap()
                    }
                    None => s.with_bounds(0..0).unwrap(),
                };

                Ok(result.into())
            }
            (_, unexpected) => type_error_with_slice(expected_error, unexpected),
        }
    });

    result
}

fn is_string(value: &Value) -> bool {
    matches!(value, Value::Str(_))
}
