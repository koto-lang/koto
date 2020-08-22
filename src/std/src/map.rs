use crate::{external_error, single_arg_fn};
use koto_runtime::{value, Value, ValueList, ValueMap, ValueVec};
use std::sync::Arc;

pub fn register(prelude: &mut ValueMap) {
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

    map.add_fn("get", |_, args|{
        match args {
            [Map(m), Str(key)] => {
                match m.data().get(key) {
                    Some(value) => Ok(value.clone()),
                    None => Ok(Empty),
                }
            }
            _ => external_error!("map.get: Expected map and key as arguments"),
        }
    });

    map.add_fn("insert", |_, args|{
        match args {
            [Map(m), Str(key)] => {
                match m.data_mut().add_value(key.as_ref(), Empty) {
                    Some(old_value) => Ok(old_value),
                    None => Ok(Empty),
                }
            }
            [Map(m), Str(key), value] => {
                match m.data_mut().add_value(key.as_ref(), value.clone()) {
                    Some(old_value) => Ok(old_value),
                    None => Ok(Empty),
                }
            }
            _ => external_error!("map.insert: Expected map and key as arguments"),
        }
    });

    prelude.add_value("map", Map(map));
}
