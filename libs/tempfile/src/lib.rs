//! A Koto language module for working with temporary files

use koto_runtime::{
    core::io::{make_file_map, File},
    runtime_error, Value, ValueMap,
};

pub fn make_module() -> ValueMap {
    use Value::*;

    let mut result = ValueMap::new();

    result.add_fn("temp_path", {
        |_, _| match tempfile::NamedTempFile::new() {
            Ok(file) => match file.keep() {
                Ok((_temp_file, path)) => Ok(Str(path.to_string_lossy().as_ref().into())),
                Err(e) => runtime_error!("io.temp_file: Error while making temp path: {}", e),
            },
            Err(e) => runtime_error!("io.temp_file: Error while making temp path: {}", e),
        }
    });

    result.add_fn("temp_file", {
        move |_, _| {
            let (temp_file, path) = match tempfile::NamedTempFile::new() {
                Ok(file) => match file.keep() {
                    Ok((temp_file, path)) => (temp_file, path),
                    Err(e) => {
                        return runtime_error!(
                            "io.temp_file: Error while creating temp file: {}",
                            e,
                        );
                    }
                },
                Err(e) => {
                    return runtime_error!("io.temp_file: Error while creating temp file: {}", e);
                }
            };
            
            //TODO: use once_cell::sync::Lazy to amortize cost
            let file_map = make_file_map();

            Ok(Value::make_external_value(File {
                    file: temp_file,
                    path,
                    temporary: true,
                },
                file_map,
            ))
        }
    });

    result
}
