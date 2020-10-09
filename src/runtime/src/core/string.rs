mod format;

use crate::{external_error, Value, ValueList, ValueMap, ValueVec};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("contains", |vm, args| match vm.get_args(args) {
        [Str(s1), Str(s2)] => Ok(Bool(s1.contains(s2.as_ref()))),
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
        [Str(s)] => Ok(List(ValueList::with_data(
            s.lines()
                .map(|line| Str(line.to_string().into()))
                .collect::<ValueVec>(),
        ))),
        _ => external_error!("string.lines: Expected string as argument"),
    });

    result.add_fn("slice", |vm, args| match vm.get_args(args) {
        [Str(input), Number(from)] => {
            let result = input
                .get((*from as usize)..)
                .map(|s| Str(s.into()))
                .unwrap_or(Empty);
            Ok(result)
        }
        [Str(input), Number(from), Number(to)] => {
            let result = input
                .get((*from as usize)..(*to as usize))
                .map(|s| Str(s.into()))
                .unwrap_or(Empty);
            Ok(result)
        }
        _ => external_error!("string.slice: Expected a string and slice index as arguments"),
    });

    result.add_fn("split", |vm, args| match vm.get_args(args) {
        [Str(input), Str(pattern)] => {
            let result = input
                .split(pattern.as_ref())
                .map(|s| Str(s.into()))
                .collect::<ValueVec>();
            Ok(List(ValueList::with_data(result)))
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
        [Str(s)] => Ok(Str(s.trim().to_string().into())),
        _ => external_error!("string.trim: Expected string as argument"),
    });

    result
}
