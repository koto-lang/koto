use crate::{builtin_error, single_arg_fn};
use koto_runtime::{value, value::deref_value, Error, Value, ValueMap};
use std::{fs, path::Path, rc::Rc};

pub fn register(global: &mut ValueMap) {
    use Value::{Bool, Str};

    let mut io = ValueMap::new();

    single_arg_fn!(io, "exists", Str, path, {
        Ok(Bool(Path::new(path.as_ref()).exists()))
    });

    single_arg_fn!(io, "read_string", Str, path, {
        {
            match fs::read_to_string(Path::new(path.as_ref())) {
                Ok(result) => Ok(Str(Rc::new(result))),
                Err(e) => builtin_error!("Unable to read file {}: {}", path, e),
            }
        }
    });

    global.add_map("io", io);
}
