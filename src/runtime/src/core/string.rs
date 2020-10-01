mod format;

use {
    crate::{external_error, Value, ValueList, ValueMap, ValueVec},
    std::sync::Arc,
};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("contains", |_, args| match args {
        [Str(s1), Str(s2)] => Ok(Bool(s1.contains(s2.as_ref()))),
        _ => external_error!("string.contains: Expected two strings as arguments"),
    });

    result.add_fn("escape", |_, args| match args {
        [Str(s)] => Ok(Str(Arc::new(s.escape_default().to_string()))),
        _ => external_error!("string.escape: Expected string as argument"),
    });

    result.add_fn("format", |_, args| match args {
        [result @ Str(_)] => Ok(result.clone()),
        [Str(format), format_args @ ..] => match format::format_string(format, format_args) {
            Ok(result) => Ok(Str(Arc::new(result))),
            Err(error) => external_error!("string.format: {}", error),
        },
        _ => external_error!("string.format: Expected a string as first argument"),
    });

    result.add_fn("lines", |_, args| match args {
        [Str(s)] => Ok(List(ValueList::with_data(
            s.lines()
                .map(|line| Str(Arc::new(line.to_string())))
                .collect::<ValueVec>(),
        ))),
        _ => external_error!("string.lines: Expected string as argument"),
    });

    result.add_fn("slice", |_, args| match args {
        [Str(input), Number(from)] => {
            let result = input
                .get((*from as usize)..)
                .map(|s| Str(Arc::new(s.to_string())))
                .unwrap_or(Empty);
            Ok(result)
        }
        [Str(input), Number(from), Number(to)] => {
            let result = input
                .get((*from as usize)..(*to as usize))
                .map(|s| Str(Arc::new(s.to_string())))
                .unwrap_or(Empty);
            Ok(result)
        }
        _ => external_error!("string.slice: Expected a string and slice index as arguments"),
    });

    result.add_fn("split", |_, args| match args {
        [Str(input), Str(pattern)] => {
            let result = input
                .split(pattern.as_ref())
                .map(|s| Str(Arc::new(s.to_string())))
                .collect::<ValueVec>();
            Ok(List(ValueList::with_data(result)))
        }
        _ => external_error!("string.split: Expected two strings as arguments"),
    });

    result.add_fn("to_number", |_, args| match args {
        [Str(s)] => match s.parse::<f64>() {
            Ok(n) => Ok(Number(n)),
            Err(_) => external_error!("string.to_number: Failed to convert '{}'", s),
        },
        _ => external_error!("string.to_number: Expected string as argument"),
    });

    result.add_fn("trim", |_, args| match args {
        [Str(s)] => Ok(Str(Arc::new(s.trim().to_string()))),
        _ => external_error!("string.trim: Expected string as argument"),
    });

    result
}
