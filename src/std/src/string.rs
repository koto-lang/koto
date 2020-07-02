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

    single_arg_fn!(string, "lines", Str, s, {
        Ok(List(ValueList::with_data(
            s.lines()
                .map(|line| Str(Arc::new(line.to_string())))
                .collect::<ValueVec>(),
        )))
    });

    single_arg_fn!(string, "to_number", Str, s, {
        match s.parse::<f64>() {
            Ok(n) => Ok(Number(n)),
            Err(_) => external_error!("string.to_number: Failed to convert '{}'", s),
        }
    });

    prelude.add_value("string", Map(string));
}
