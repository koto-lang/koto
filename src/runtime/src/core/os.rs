use crate::ValueMap;

pub fn make_module() -> ValueMap {
    let mut result = ValueMap::new();

    result.add_fn("name", |_vm, _args| Ok(std::env::consts::OS.into()));

    result
}
