use {
    crate::{external_error, single_arg_fn},
    koto_runtime::{value, value_is_immutable, Value, ValueList, ValueMap, ValueVec},
};

pub fn register(prelude: &mut ValueMap) {
    use Value::*;

    let mut map = ValueMap::new();

    single_arg_fn!(map, "keys", Map, m, {
        Ok(List(ValueList::with_data(
            m.data().keys().cloned().collect::<ValueVec>(),
        )))
    });

    map.add_fn("get", |_, args| match args {
        [Map(m), key] => match m.data().get(key) {
            Some(value) => Ok(value.clone()),
            None => Ok(Empty),
        },
        _ => external_error!("map.get: Expected map and key as arguments"),
    });

    map.add_fn("insert", |_, args| match args {
        [Map(m), key] if value_is_immutable(key) => match m.data_mut().insert(key.clone(), Empty) {
            Some(old_value) => Ok(old_value),
            None => Ok(Empty),
        },
        [Map(m), key, value] if value_is_immutable(key) => {
            match m.data_mut().insert(key.clone(), value.clone()) {
                Some(old_value) => Ok(old_value),
                None => Ok(Empty),
            }
        }
        _ => external_error!("map.insert: Expected map and key as arguments"),
    });

    map.add_fn("remove", |_, args| match args {
        [Map(m), key] if value_is_immutable(key) => match m.data_mut().remove(key) {
            Some(old_value) => Ok(old_value),
            None => Ok(Empty),
        },
        _ => external_error!("map.remove: Expected map and key as arguments"),
    });

    single_arg_fn!(map, "values", Map, m, {
        Ok(List(ValueList::with_data(
            m.data().values().cloned().collect::<ValueVec>(),
        )))
    });

    prelude.add_value("map", Map(map));
}
