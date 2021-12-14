pub mod format;
pub mod iterators;

use {
    super::iterator::collect_pair,
    crate::{
        runtime_error, unexpected_type_error_with_slice,
        value_iterator::{make_iterator, ValueIterator, ValueIteratorOutput as Output},
        RuntimeResult, Value, ValueMap,
    },
    std::convert::TryFrom,
    unicode_segmentation::UnicodeSegmentation,
};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("bytes", |vm, args| match vm.get_args(args) {
        [Str(s)] => {
            let result = iterators::Bytes::new(s.clone());
            Ok(Iterator(ValueIterator::make_external(result)))
        }
        unexpected => expected_string_error("bytes", unexpected),
    });

    result.add_fn("chars", |vm, args| match vm.get_args(args) {
        [Str(s)] => Ok(Iterator(ValueIterator::with_string(s.clone()))),
        unexpected => expected_string_error("chars", unexpected),
    });

    result.add_fn("contains", |vm, args| match vm.get_args(args) {
        [Str(s1), Str(s2)] => Ok(Bool(s1.contains(s2.as_str()))),
        unexpected => expected_string_error("contains", unexpected),
    });

    result.add_fn("ends_with", |vm, args| match vm.get_args(args) {
        [Str(s), Str(pattern)] => {
            let result = s.as_str().ends_with(pattern.as_str());
            Ok(Bool(result))
        }
        unexpected => expected_two_strings_error("ends_with", unexpected),
    });

    result.add_fn("escape", |vm, args| match vm.get_args(args) {
        [Str(s)] => Ok(Str(s.escape_default().to_string().into())),
        unexpected => expected_string_error("escape", unexpected),
    });

    result.add_fn("format", |vm, args| match vm.get_args(args) {
        [result @ Str(_)] => Ok(result.clone()),
        [Str(format), format_args @ ..] => {
            let format = format.clone();
            let format_args = format_args.to_vec();
            match format::format_string(vm, &format, &format_args) {
                Ok(result) => Ok(Str(result.into())),
                Err(error) => Err(error.with_prefix("string.format")),
            }
        }
        unexpected => unexpected_type_error_with_slice(
            "string.format",
            "a String as argument, followed by optional additional Values",
            unexpected,
        ),
    });

    result.add_fn("from_bytes", |vm, args| match vm.get_args(args) {
        [iterable] if iterable.is_iterable() => {
            let iterator = make_iterator(iterable).unwrap();
            let (size_hint, _) = iterator.size_hint();
            let mut bytes = Vec::<u8>::with_capacity(size_hint);

            for output in iterator.map(collect_pair) {
                match output {
                    Output::Value(Number(n)) => match u8::try_from(n.as_i64()) {
                        Ok(byte) => bytes.push(byte),
                        Err(_) => {
                            return runtime_error!(
                                "string.from_bytes: '{}' is out of the valid byte range",
                                n
                            )
                        }
                    },
                    Output::Value(unexpected) => {
                        return unexpected_type_error_with_slice(
                            "string.from_bytes",
                            "a number",
                            &[unexpected],
                        )
                    }
                    Output::Error(error) => return Err(error),
                    _ => unreachable!(),
                }
            }

            match String::from_utf8(bytes) {
                Ok(result) => Ok(Str(result.into())),
                Err(_) => runtime_error!("string.from_bytes: input failed UTF-8 validation"),
            }
        }
        unexpected => unexpected_type_error_with_slice(
            "string.from_bytes",
            "an iterable value as argument",
            unexpected,
        ),
    });

    result.add_fn("is_empty", |vm, args| match vm.get_args(args) {
        [Str(s)] => Ok(Bool(s.is_empty())),
        unexpected => expected_string_error("is_empty", unexpected),
    });

    result.add_fn("lines", |vm, args| match vm.get_args(args) {
        [Str(s)] => {
            let result = iterators::Lines::new(s.clone());
            Ok(Iterator(ValueIterator::make_external(result)))
        }
        unexpected => expected_string_error("lines", unexpected),
    });

    result.add_fn("size", |vm, args| match vm.get_args(args) {
        [Str(s)] => Ok(Number(s.graphemes(true).count().into())),
        unexpected => expected_string_error("size", unexpected),
    });

    result.add_fn("slice", |vm, args| match vm.get_args(args) {
        [Str(input), Number(from)] => {
            let bounds = usize::from(*from)..input.len();
            let result = match input.with_bounds(bounds) {
                Some(result) => Str(result),
                None => Empty,
            };
            Ok(result)
        }
        [Str(input), Number(from), Number(to)] => {
            let bounds = usize::from(*from)..usize::from(*to);
            let result = match input.with_bounds(bounds) {
                Some(result) => Str(result),
                None => Empty,
            };
            Ok(result)
        }
        unexpected => unexpected_type_error_with_slice(
            "string.slice",
            "a String and Number as arguments",
            unexpected,
        ),
    });

    result.add_fn("split", |vm, args| {
        let iterator = match vm.get_args(args) {
            [Str(input), Str(pattern)] => {
                let result = iterators::Split::new(input.clone(), pattern.clone());
                ValueIterator::make_external(result)
            }
            [Str(input), predicate] if predicate.is_callable() => {
                let result = iterators::SplitWith::new(
                    input.clone(),
                    predicate.clone(),
                    vm.spawn_shared_vm(),
                );
                ValueIterator::make_external(result)
            }
            unexpected => {
                return unexpected_type_error_with_slice(
                    "string.split",
                    "a String and either a String or predicate Function as arguments",
                    unexpected,
                )
            }
        };

        Ok(Iterator(iterator))
    });

    result.add_fn("starts_with", |vm, args| match vm.get_args(args) {
        [Str(s), Str(pattern)] => {
            let result = s.as_str().starts_with(pattern.as_str());
            Ok(Bool(result))
        }
        unexpected => expected_two_strings_error("starts_with", unexpected),
    });

    result.add_fn("to_lowercase", |vm, args| match vm.get_args(args) {
        [Str(s)] => {
            let result = s.chars().flat_map(|c| c.to_lowercase()).collect::<String>();
            Ok(Str(result.into()))
        }
        unexpected => expected_string_error("to_lowercase", unexpected),
    });

    result.add_fn("to_number", |vm, args| match vm.get_args(args) {
        [Str(s)] => match s.parse::<i64>() {
            Ok(n) => Ok(Number(n.into())),
            Err(_) => match s.parse::<f64>() {
                Ok(n) => Ok(Number(n.into())),
                Err(_) => {
                    runtime_error!("string.to_number: Failed to convert '{}'", s)
                }
            },
        },
        unexpected => expected_string_error("to_number", unexpected),
    });

    result.add_fn("to_uppercase", |vm, args| match vm.get_args(args) {
        [Str(s)] => {
            let result = s.chars().flat_map(|c| c.to_uppercase()).collect::<String>();
            Ok(Str(result.into()))
        }
        unexpected => expected_string_error("to_uppercase", unexpected),
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
        unexpected => expected_string_error("trim", unexpected),
    });

    result
}

fn expected_string_error(name: &str, unexpected: &[Value]) -> RuntimeResult {
    unexpected_type_error_with_slice(
        &format!("string.{}", name),
        "a String as argument",
        unexpected,
    )
}

fn expected_two_strings_error(name: &str, unexpected: &[Value]) -> RuntimeResult {
    unexpected_type_error_with_slice(
        &format!("string.{}", name),
        "two Strings as arguments",
        unexpected,
    )
}
