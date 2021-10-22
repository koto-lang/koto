use crate::{Value, ValueMap};

pub fn make_module() -> ValueMap {
    use Value::Number;

    let mut result = ValueMap::new();

    result.add_fn("cpu_count", |_vm, _args| Ok(Number(num_cpus::get().into())));

    result.add_fn("name", |_vm, _args| Ok(std::env::consts::OS.into()));

    result.add_fn("physical_cpu_count", |_vm, _args| {
        Ok(Number(num_cpus::get_physical().into()))
    });

    result
}
