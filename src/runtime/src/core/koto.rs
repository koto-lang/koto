use crate::{runtime_error, Value, ValueMap, ValueTuple};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_value("args", Tuple(ValueTuple::default()));

    result.add_fn("exports", |vm, _| {
        Ok(Value::Map(vm.context_mut().exports.clone()))
    });

    result.add_value("script_dir", Empty);
    result.add_value("script_path", Empty);

    result.add_fn("type", |vm, args| match vm.get_args(args) {
        [value] => Ok(Str(value.type_as_string().into())),
        _ => runtime_error!("koto.type: Expected single argument"),
    });

    result
}
