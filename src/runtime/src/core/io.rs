use {
    crate::{
        external_error, get_external_instance, make_external_value, value::type_as_string,
        ExternalValue, RuntimeResult, Value, ValueMap,
    },
    std::{
        fmt, fs,
        io::{Read, Seek, SeekFrom, Write},
        path::{Path, PathBuf},
    },
};

pub fn make_file_map() -> ValueMap {
    use Value::{Number, Str};

    fn file_fn(
        fn_name: &str,
        args: &[Value],
        mut file_op: impl FnMut(&mut File) -> RuntimeResult,
    ) -> RuntimeResult {
        get_external_instance!(args, "File", fn_name, File, file_ref, { file_op(file_ref) })
    }

    let mut file_map = ValueMap::new();

    file_map.add_instance_fn("path", |vm, args| {
        file_fn("path", vm.get_args(args), |file_handle| {
            Ok(Str(file_handle.path.to_string_lossy().as_ref().into()))
        })
    });

    file_map.add_instance_fn("write", |vm, args| {
        file_fn("write", vm.get_args(args), |file_handle| {
            match vm.get_args(args) {
                [_, value] => {
                    let data = format!("{}", value);

                    match file_handle.file.write(data.as_bytes()) {
                        Ok(_) => Ok(Value::Empty),
                        Err(e) => external_error!("File.write: Error while writing to file: {}", e),
                    }
                }
                _ => external_error!("File.write: Expected single value to write as argument"),
            }
        })
    });

    file_map.add_instance_fn("write_line", |vm, args| {
        file_fn("write_line", vm.get_args(args), |file_handle| {
            let line = match vm.get_args(args) {
                [_] => "\n".to_string(),
                [_, value] => format!("{}\n", value),
                _ => {
                    return external_error!("File.write_line: Expected single value as argument");
                }
            };
            match file_handle.file.write(line.as_bytes()) {
                Ok(_) => Ok(Value::Empty),
                Err(e) => external_error!("File.write_line: Error while writing to file: {}", e),
            }
        })
    });

    file_map.add_instance_fn("read_to_string", |vm, args| {
        file_fn(
            "read_to_string",
            vm.get_args(args),
            |file_handle| match file_handle.file.seek(SeekFrom::Start(0)) {
                Ok(_) => {
                    let mut buffer = String::new();
                    match file_handle.file.read_to_string(&mut buffer) {
                        Ok(_) => Ok(Str(buffer.into())),
                        Err(e) => {
                            external_error!("File.read_to_string: Error while reading data: {}", e,)
                        }
                    }
                }
                Err(e) => {
                    external_error!("File.read_to_string: Error while seeking in file: {}", e)
                }
            },
        )
    });

    file_map.add_instance_fn("seek", |vm, args| {
        file_fn("seek", vm.get_args(args), |file_handle| {
            match vm.get_args(args) {
                [_, Number(n)] => {
                    if *n < 0.0 {
                        return external_error!("File.seek: Negative seek positions not allowed");
                    }
                    match file_handle.file.seek(SeekFrom::Start(*n as u64)) {
                        Ok(_) => Ok(Value::Empty),
                        Err(e) => external_error!("File.seek: Error while seeking in file: {}", e),
                    }
                }
                [_, unexpected] => external_error!(
                    "File.seek: Expected Number for seek position, found '{}'",
                    type_as_string(&unexpected),
                ),
                _ => external_error!("File.seek: Expected seek position as second argument"),
            }
        })
    });

    file_map
}

pub fn make_module() -> ValueMap {
    use Value::{Bool, Map, Str};

    let mut result = ValueMap::new();

    result.add_fn("exists", |vm, args| match vm.get_args(args) {
        [Str(path)] => Ok(Bool(Path::new(path.as_str()).exists())),
        _ => external_error!("io.exists: Expected path string as argument"),
    });

    result.add_fn("read_to_string", |vm, args| match vm.get_args(args) {
        [Str(path)] => match fs::read_to_string(Path::new(path.as_str())) {
            Ok(result) => Ok(Str(result.into())),
            Err(e) => external_error!("io.read_to_string: Unable to read file '{}': {}", path, e),
        },
        _ => external_error!("io.read_to_string: Expected path string as argument"),
    });

    result.add_fn("open", {
        move |vm, args| match vm.get_args(args) {
            [Str(path)] => {
                let path = Path::new(path.as_str());
                match fs::File::open(&path) {
                    Ok(file) => {
                        let file_map = make_file_map();

                        file_map.data_mut().insert(
                            Value::ExternalDataId,
                            make_external_value(File {
                                file,
                                path: path.to_path_buf(),
                                temporary: false,
                            }),
                        );

                        Ok(Map(file_map))
                    }
                    Err(e) => {
                        return external_error!("io.open: Error while opening path: {}", e);
                    }
                }
            }
            [unexpected] => external_error!(
                "io.open: Expected a String as argument, found '{}'",
                type_as_string(&unexpected),
            ),
            _ => external_error!("io.open: Expected a String as argument"),
        }
    });

    result.add_fn("create", {
        move |vm, args| match vm.get_args(args) {
            [Str(path)] => {
                let path = Path::new(path.as_str());
                match fs::File::create(&path) {
                    Ok(file) => {
                        let mut file_map = make_file_map();

                        file_map.insert(
                            Value::ExternalDataId,
                            make_external_value(File {
                                file,
                                path: path.to_path_buf(),
                                temporary: false,
                            }),
                        );

                        Ok(Map(file_map))
                    }
                    Err(e) => {
                        return external_error!("io.create: Error while creating file: {}", e);
                    }
                }
            }
            [unexpected] => external_error!(
                "io.create: Expected a String as argument, found '{}'",
                type_as_string(&unexpected),
            ),
            _ => external_error!("io.create: Expected a String as argument"),
        }
    });

    result.add_fn("temp_dir", {
        |_, _| Ok(Str(std::env::temp_dir().to_string_lossy().as_ref().into()))
    });

    result.add_fn("remove_file", {
        |vm, args| match vm.get_args(args) {
            [Str(path)] => {
                let path = Path::new(path.as_str());
                match fs::remove_file(&path) {
                    Ok(_) => Ok(Value::Empty),
                    Err(e) => external_error!(
                        "io.remove_file: Error while removing file '{}': {}",
                        path.to_string_lossy(),
                        e,
                    ),
                }
            }
            [unexpected] => external_error!(
                "io.remove_file: Expected a String as argument, found '{}'",
                type_as_string(&unexpected),
            ),
            _ => external_error!("io.remove_file: Expected a String as argument"),
        }
    });

    result
}

#[derive(Debug)]
pub struct File {
    pub file: fs::File,
    pub path: PathBuf,
    pub temporary: bool,
}

impl Drop for File {
    fn drop(&mut self) {
        if self.temporary {
            let _ = fs::remove_file(&self.path).is_ok();
        }
    }
}

impl ExternalValue for File {
    fn value_type(&self) -> String {
        "File".to_string()
    }
}

impl fmt::Display for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "File({})", self.path.to_string_lossy())
    }
}
