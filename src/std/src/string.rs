mod format;

use {
    crate::{external_error, single_arg_fn},
    koto_runtime::{value, Value, ValueList, ValueMap, ValueVec},
    std::sync::Arc,
};

pub fn register(prelude: &mut ValueMap) {
    use Value::*;

    let mut string = ValueMap::new();

    single_arg_fn!(string, "escape", Str, s, {
        Ok(Str(Arc::new(s.escape_default().to_string())))
    });

    string.add_fn("format", |_, args| match args {
        [result @ Str(_)] => Ok(result.clone()),
        [Str(format), format_args @ ..] => match format::format_string(format, format_args) {
            Ok(result) => Ok(Str(Arc::new(result))),
            Err(error) => external_error!("string.format: {}", error),
        },
        _ => external_error!("string.format: Expected a string as first argument"),
    });

    single_arg_fn!(string, "lines", Str, s, {
        Ok(List(ValueList::with_data(
            s.lines()
                .map(|line| Str(Arc::new(line.to_string())))
                .collect::<ValueVec>(),
        )))
    });

    string.add_fn("split", |_, args| match args {
        [Str(input), Str(pattern)] => {
            let result = input
                .split(pattern.as_ref())
                .map(|s| Str(Arc::new(s.to_string())))
                .collect::<ValueVec>();
            Ok(List(ValueList::with_data(result)))
        }
        _ => external_error!("string.split: Expected two strings as arguments"),
    });
    single_arg_fn!(string, "to_number", Str, s, {
        match s.parse::<f64>() {
            Ok(n) => Ok(Number(n)),
            Err(_) => external_error!("string.to_number: Failed to convert '{}'", s),
        }
    });

    prelude.add_value("string", Map(string));
}
