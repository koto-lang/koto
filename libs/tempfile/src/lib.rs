//! A Koto language module for working with temporary files

use koto_runtime::{
    KMap,
    core_lib::io::{File, map_io_err},
    unexpected_args,
};
use tempfile::NamedTempFile;

pub fn make_module() -> KMap {
    let result = KMap::with_type("temp_file");

    result.add_fn("temp_file", {
        |ctx| match ctx.args() {
            [] => match NamedTempFile::new().map_err(map_io_err) {
                Ok(file) => {
                    let path = file.path().to_path_buf();
                    Ok(File::system_file(file, path))
                }
                Err(e) => Err(e),
            },
            unexpected => unexpected_args("||", unexpected),
        }
    });

    result
}
