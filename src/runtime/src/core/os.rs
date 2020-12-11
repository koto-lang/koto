use crate::{Value, ValueMap};

pub fn make_module() -> ValueMap {
    use Value::Number;

    let mut result = ValueMap::new();

    result.add_fn("cpu_count", |_vm, _args| Ok(Number(num_cpus::get() as f64)));

    result.add_fn("physical_cpu_count", |_vm, _args| {
        Ok(Number(num_cpus::get_physical() as f64))
    });

    result
}
