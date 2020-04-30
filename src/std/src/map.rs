use crate::single_arg_fn;
use koto_runtime::{value, Value, ValueList, ValueMap, ValueVec};
use std::sync::Arc;

pub fn register(global: &mut ValueMap) {
    use Value::*;

    let mut map = ValueMap::new();

    single_arg_fn!(map, "keys", Map, m, {
        Ok(List(ValueList::with_data(
            m.data()
                .keys()
                .map(|k| Str(Arc::new(k.as_str().to_string())))
                .collect::<ValueVec>(),
        )))
    });

    global.add_value("map", Map(map));
}
