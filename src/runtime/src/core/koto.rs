use crate::{type_error_with_slice, Value, ValueMap, ValueTuple};

pub fn make_module() -> ValueMap {
    use Value::*;

    let result = ValueMap::new();

    result.add_value("args", Tuple(ValueTuple::default()));

    result.add_fn("exports", |vm, _| Ok(Value::Map(vm.exports().clone())));

    result.add_value("script_dir", Null);
    result.add_value("script_path", Null);

    result.add_fn("type", |vm, args| match vm.get_args(args) {
        [value] => Ok(Str(value.type_as_string().into())),
        unexpected => type_error_with_slice("koto.type", "a single argument", unexpected),
    });

    result
}
