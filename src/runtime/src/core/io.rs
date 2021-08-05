use {
    crate::{runtime_error, ExternalData, ExternalValue, MetaMap, Value, ValueMap},
    lazy_static::lazy_static,
    parking_lot::RwLock,
    std::{
        fmt, fs,
        io::{Read, Seek, SeekFrom, Write},
        path::{Path, PathBuf},
        sync::Arc,
    },
};

pub fn make_module() -> ValueMap {
    use Value::{Bool, Str};

    let mut result = ValueMap::new();

    result.add_fn("create", {
        move |vm, args| match vm.get_args(args) {
            [Str(path)] => {
                let path = Path::new(path.as_str());
                match fs::File::create(&path) {
                    Ok(file) => Ok(File::make_external_value(file, path, false)),
                    Err(e) => {
                        return runtime_error!("io.create: Error while creating file: {}", e);
                    }
                }
            }
            [unexpected] => runtime_error!(
                "io.create: Expected a String as argument, found '{}'",
                unexpected.type_as_string(),
            ),
            _ => runtime_error!("io.create: Expected a String as argument"),
        }
    });

    result.add_fn("exists", |vm, args| match vm.get_args(args) {
        [Str(path)] => Ok(Bool(Path::new(path.as_str()).exists())),
        _ => runtime_error!("io.exists: Expected path string as argument"),
    });

    result.add_fn("open", {
        move |vm, args| match vm.get_args(args) {
            [Str(path)] => {
                let path = Path::new(path.as_str());
                match fs::File::open(&path) {
                    Ok(file) => Ok(File::make_external_value(file, path, false)),
                    Err(e) => {
                        return runtime_error!("io.open: Error while opening path: {}", e);
                    }
                }
            }
            [unexpected] => runtime_error!(
                "io.open: Expected a String as argument, found '{}'",
                unexpected.type_as_string(),
            ),
            _ => runtime_error!("io.open: Expected a String as argument"),
        }
    });

    result.add_fn("read_to_string", |vm, args| match vm.get_args(args) {
        [Str(path)] => match fs::read_to_string(Path::new(path.as_str())) {
            Ok(result) => Ok(Str(result.into())),
            Err(e) => runtime_error!("io.read_to_string: Unable to read file '{}': {}", path, e),
        },
        _ => runtime_error!("io.read_to_string: Expected path string as argument"),
    });

    result.add_fn("remove_file", {
        |vm, args| match vm.get_args(args) {
            [Str(path)] => {
                let path = Path::new(path.as_str());
                match fs::remove_file(&path) {
                    Ok(_) => Ok(Value::Empty),
                    Err(e) => runtime_error!(
                        "io.remove_file: Error while removing file '{}': {}",
                        path.to_string_lossy(),
                        e,
                    ),
                }
            }
            [unexpected] => runtime_error!(
                "io.remove_file: Expected a String as argument, found '{}'",
                unexpected.type_as_string(),
            ),
            _ => runtime_error!("io.remove_file: Expected a String as argument"),
        }
    });

    result.add_fn("temp_dir", {
        |_, _| Ok(Str(std::env::temp_dir().to_string_lossy().as_ref().into()))
    });

    result
}

lazy_static! {
    static ref FILE_META: Arc<RwLock<MetaMap>> = {
        use Value::{Number, Str};

        let mut meta = MetaMap::with_type_name("File");

        meta.add_named_instance_fn("path", |file: &File, _, _| {
            Ok(Str(file.path.to_string_lossy().as_ref().into()))
        });

        meta.add_named_instance_fn_mut("read_to_string", |file: &mut File, _, _| {
            match file.file.seek(SeekFrom::Start(0)) {
                Ok(_) => {
                    let mut buffer = String::new();
                    match file.file.read_to_string(&mut buffer) {
                        Ok(_) => Ok(Str(buffer.into())),
                        Err(e) => {
                            runtime_error!("File.read_to_string: Error while reading data: {}", e,)
                        }
                    }
                }
                Err(e) => {
                    runtime_error!("File.read_to_string: Error while seeking in file: {}", e)
                }
            }
        });

        meta.add_named_instance_fn_mut("seek", |file: &mut File, _, args| match args {
            [Number(n)] => {
                if *n < 0.0 {
                    return runtime_error!("File.seek: Negative seek positions not allowed");
                }
                match file.file.seek(SeekFrom::Start(n.into())) {
                    Ok(_) => Ok(Value::Empty),
                    Err(e) => {
                        runtime_error!("File.seek: Error while seeking in file: {}", e)
                    }
                }
            }
            [unexpected] => runtime_error!(
                "File.seek: Expected Number for seek position, found '{}'",
                unexpected.type_as_string(),
            ),
            _ => runtime_error!("File.seek: Expected seek position as second argument"),
        });

        meta.add_named_instance_fn_mut("write", |file: &mut File, _, args| match args {
            [value] => {
                let data = format!("{}", value);

                match file.file.write(data.as_bytes()) {
                    Ok(_) => Ok(Value::Empty),
                    Err(e) => {
                        runtime_error!("File.write: Error while writing to file: {}", e)
                    }
                }
            }
            _ => runtime_error!("File.write: Expected single value to write as argument"),
        });

        meta.add_named_instance_fn_mut("write_line", |file: &mut File, _, args| {
            let line = match args {
                [] => "\n".to_string(),
                [value] => format!("{}\n", value),
                _ => {
                    return runtime_error!("File.write_line: Expected single value as argument");
                }
            };
            match file.file.write(line.as_bytes()) {
                Ok(_) => Ok(Value::Empty),
                Err(e) => runtime_error!("File.write_line: Error while writing to file: {}", e),
            }
        });

        Arc::new(RwLock::new(meta))
    };
}

#[derive(Debug)]
pub struct File {
    pub file: fs::File,
    pub path: PathBuf,
    /// When set to true the file will be removed when Dropped
    pub temporary: bool,
}

impl File {
    pub fn make_external_value(file: fs::File, path: &Path, temporary: bool) -> Value {
        let result = ExternalValue::with_shared_meta_map(
            File {
                file,
                path: path.to_path_buf(),
                temporary,
            },
            FILE_META.clone(),
        );
        Value::ExternalValue(result)
    }
}

impl Drop for File {
    fn drop(&mut self) {
        if self.temporary {
            let _ = fs::remove_file(&self.path).is_ok();
        }
    }
}

impl ExternalData for File {
    fn value_type(&self) -> String {
        "File".to_string()
    }
}

impl fmt::Display for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "File({})", self.path.to_string_lossy())
    }
}
