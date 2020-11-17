use crate::{external_error, type_as_string, Value, ValueList, ValueMap};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_value("args", List(ValueList::default()));
    result.add_value("script_dir", Str("".into()));
    result.add_value("script_path", Str("".into()));

    result.add_fn("type", |vm, args| match vm.get_args(args) {
        [value] => Ok(Str(type_as_string(value).into())),
        _ => external_error!("koto.type: Expected single argument"),
    });

    result
}
