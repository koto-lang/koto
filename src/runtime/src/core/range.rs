use crate::{external_error, Value, ValueMap};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("contains", |_, args| match args {
        [Range(r), Number(n)] => Ok(Bool( *n >= r.start as f64 && n.ceil() < r.end as f64 )),
        _ => external_error!("range.contains: Expected range and number as arguments"),
    });

    result.add_fn("size", |_, args| match args {
        [Range(r)] => Ok(Number((r.end - r.start) as f64)),
        _ => external_error!("range.size: Expected range as argument"),
    });

    result
}
