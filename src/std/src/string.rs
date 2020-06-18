use {
    crate::single_arg_fn,
    koto_runtime::{value, Value, ValueList, ValueMap, ValueVec},
    std::sync::Arc,
};

pub fn register(global: &mut ValueMap) {
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

    global.add_value("string", Map(string));
}
