use crate::{external_error, type_as_string, Value, ValueList, ValueMap};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_value("args", List(ValueList::default()));

    result.add_fn("current_dir", |_, _| {
        let result = match std::env::current_dir() {
            Ok(path) => Str(path.to_string_lossy().to_string().into()),
            Err(_) => Empty,
        };
        Ok(result)
    });

    result.add_fn("exports", |vm, _| {
        Ok(Value::Map(vm.context_mut().global.clone()))
    });

    result.add_fn("type", |vm, args| match vm.get_args(args) {
        [value] => Ok(Str(type_as_string(value).into())),
        _ => external_error!("koto.type: Expected single argument"),
    });

    result
}
