mod buffered_file;

use {
    crate::{runtime_error, ExternalData, ExternalValue, MetaMap, RuntimeError, Value, ValueMap},
    buffered_file::BufferedFile,
    lazy_static::lazy_static,
    parking_lot::RwLock,
    std::{
        fmt, fs,
        io::{self, Read, Seek, SeekFrom, Write},
        path::{Path, PathBuf},
        sync::Arc,
    },
};

pub fn make_module() -> ValueMap {
    use Value::{Bool, Empty, Str};

    let mut result = ValueMap::new();

    result.add_fn("create", {
        move |vm, args| match vm.get_args(args) {
            [Str(path)] => {
                let path = Path::new(path.as_str());
                match fs::File::create(&path) {
                    Ok(file) => Ok(File::with_file(file, path, false)),
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

    result.add_fn("current_dir", |_, _| {
        let result = match std::env::current_dir() {
            Ok(path) => Str(path.to_string_lossy().to_string().into()),
            Err(_) => Empty,
        };
        Ok(result)
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
                    Ok(file) => Ok(File::with_file(file, path, false)),
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

    result.add_fn("stderr", |_, _| Ok(File::stderr()));
    result.add_fn("stdin", |_, _| Ok(File::stdin()));
    result.add_fn("stdout", |_, _| Ok(File::stdout()));

    result.add_fn("temp_dir", {
        |_, _| Ok(Str(std::env::temp_dir().to_string_lossy().as_ref().into()))
    });

    result
}

lazy_static! {
    static ref FILE_META: Arc<RwLock<MetaMap>> = {
        use Value::{Empty, Number, Str};

        let mut meta = MetaMap::with_type_name("File");

        meta.add_named_instance_fn_mut("flush", |file: &mut File, _, _| match file.flush() {
            Ok(_) => Ok(Empty),
            Err(e) => Err(e.with_prefix("File.flush")),
        });

        meta.add_named_instance_fn("path", |file: &File, _, _| Ok(Str(file.path().into())));

        meta.add_named_instance_fn_mut("read_to_string", |file: &mut File, _, _| {
            match file.read_to_string() {
                Ok(result) => Ok(result.into()),
                Err(e) => Err(e.with_prefix("File.read_to_string")),
            }
        });

        meta.add_named_instance_fn_mut("seek", |file: &mut File, _, args| match args {
            [Number(n)] => {
                if *n < 0.0 {
                    return runtime_error!("File.seek: Negative seek positions not allowed");
                }
                match file.seek(n.into()) {
                    Ok(_) => Ok(Value::Empty),
                    Err(e) => Err(e.with_prefix("File.seek")),
                }
            }
            [unexpected] => runtime_error!(
                "File.seek: Expected Number for seek position, found '{}'",
                unexpected.type_as_string(),
            ),
            _ => runtime_error!("File.seek: Expected seek position as second argument"),
        });

        meta.add_named_instance_fn_mut("write", |file: &mut File, _, args| match args {
            [value] => match file.write(value.to_string().as_bytes()) {
                Ok(_) => Ok(Value::Empty),
                Err(e) => Err(e.with_prefix("File.write")),
            },
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
            match file.write(line.as_bytes()) {
                Ok(_) => Ok(Value::Empty),
                Err(e) => Err(e.with_prefix("File.write_line")),
            }
        });

        Arc::new(RwLock::new(meta))
    };
}

#[derive(Debug)]
pub enum File {
    System(SystemFile),
    Stderr(io::Stderr),
    Stdin(io::Stdin),
    Stdout(io::Stdout),
}

impl File {
    pub fn with_file(file: fs::File, path: &Path, temporary: bool) -> Value {
        let result = ExternalValue::with_shared_meta_map(
            File::System(SystemFile::new(file, path.to_path_buf(), temporary)),
            FILE_META.clone(),
        );
        Value::ExternalValue(result)
    }

    pub fn stderr() -> Value {
        let stderr = File::Stderr(io::stderr());
        let result = ExternalValue::with_shared_meta_map(stderr, FILE_META.clone());
        Value::ExternalValue(result)
    }

    pub fn stdin() -> Value {
        let stdin = File::Stdin(io::stdin());
        let result = ExternalValue::with_shared_meta_map(stdin, FILE_META.clone());
        Value::ExternalValue(result)
    }

    pub fn stdout() -> Value {
        let stdout = File::Stdout(io::stdout());
        let result = ExternalValue::with_shared_meta_map(stdout, FILE_META.clone());
        Value::ExternalValue(result)
    }

    pub fn flush(&mut self) -> Result<(), RuntimeError> {
        match self {
            File::System(file) => file.file.flush().map_err(|e| e.to_string().into()),
            _ => runtime_error!("seek unsupported for this file type"),
        }
    }

    pub fn path(&self) -> String {
        match self {
            File::System(file) => file.path.to_string_lossy().as_ref().into(),
            File::Stderr(_) => "_stderr_".into(),
            File::Stdin(_) => "_stdin_".into(),
            File::Stdout(_) => "_stdout_".into(),
        }
    }

    pub fn read_to_string(&mut self) -> Result<String, RuntimeError> {
        match self {
            File::System(file) => file.read_to_string(),
            File::Stdin(stdin) => {
                let mut result = String::new();
                match stdin.read_to_string(&mut result) {
                    Ok(_) => Ok(result),
                    Err(e) => Err(e.to_string().into()),
                }
            }
            _ => runtime_error!("seek unsupported for this file type"),
        }
    }

    pub fn seek(&mut self, position: u64) -> Result<(), RuntimeError> {
        match self {
            File::System(file) => file.seek(position),
            _ => runtime_error!("seek unsupported for this file type"),
        }
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<(), RuntimeError> {
        match self {
            File::System(file) => file.file.write_all(bytes).map_err(|e| e.to_string().into()),
            File::Stderr(stderr) => stderr.write_all(bytes).map_err(|e| e.to_string().into()),
            File::Stdout(stdout) => stdout.write_all(bytes).map_err(|e| e.to_string().into()),
            _ => runtime_error!("seek unsupported for this file type"),
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
        write!(f, "File({})", self.path())
    }
}

#[derive(Debug)]
pub struct SystemFile {
    pub file: BufferedFile,
    pub path: PathBuf,
    /// When set to true, the file will be removed when Dropped
    pub temporary: bool,
}

impl SystemFile {
    pub fn new(file: fs::File, path: PathBuf, temporary: bool) -> Self {
        Self {
            file: BufferedFile::new(file),
            path,
            temporary,
        }
    }

    pub fn read_to_string(&mut self) -> Result<String, RuntimeError> {
        match self.file.seek(SeekFrom::Start(0)) {
            Ok(_) => {
                let mut buffer = String::new();
                match self.file.read_to_string(&mut buffer) {
                    Ok(_) => Ok(buffer),
                    Err(e) => runtime_error!("Error while reading data: {}", e),
                }
            }
            Err(e) => runtime_error!("Error while seeking in file: {}", e),
        }
    }

    pub fn seek(&mut self, position: u64) -> Result<(), RuntimeError> {
        match self.file.seek(SeekFrom::Start(position)) {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string().into()),
        }
    }
}

impl Drop for SystemFile {
    fn drop(&mut self) {
        if self.temporary {
            let _ = fs::remove_file(&self.path).is_ok();
        }
    }
}
