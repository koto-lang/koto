use crate::single_arg_fn;
use koto_runtime::{value, Error, Value, ValueHashMap, ValueList, ValueMap, ValueVec};
use std::rc::Rc;

pub fn register(global: &mut ValueHashMap) {
    use Value::*;

    let mut map = ValueMap::new();

    single_arg_fn!(map, "keys", Map, m, {
        Ok(List(ValueList::with_data(
            m.data()
                .keys()
                .map(|k| Str(Rc::new(k.as_str().to_string())))
                .collect::<ValueVec>(),
        )))
    });

    global.add_value("map", Map(map));
}
