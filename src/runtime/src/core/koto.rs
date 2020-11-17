use crate::{Value, ValueList, ValueMap};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_value("args", List(ValueList::default()));
    result.add_value("script_dir", Str("".into()));
    result.add_value("script_path", Str("".into()));

    result
}
