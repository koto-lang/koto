//! A Koto language module for working with temporary files

use koto_runtime::{
    core::io::{map_io_err, File},
    ValueMap,
};
use tempfile::NamedTempFile;

pub fn make_module() -> ValueMap {
    let result = ValueMap::new();

    result.add_fn("temp_file", {
        |_, _| match NamedTempFile::new().map_err(map_io_err) {
            Ok(file) => {
                let path = file.path().to_path_buf();
                Ok(File::system_file(file, path))
            }
            Err(e) => Err(e),
        }
    });

    result
}
