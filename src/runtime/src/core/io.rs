mod buffered_file;

pub use buffered_file::BufferedFile;

use {
    super::string::format,
    crate::{
        runtime_error, ExternalData, ExternalValue, KotoFile, KotoRead, KotoWrite, MetaMap,
        RuntimeError, Value, ValueMap, Vm,
    },
    std::{
        cell::RefCell,
        fmt, fs,
        io::{self, BufRead, Read, Seek, SeekFrom, Write},
        ops::Deref,
        path::{Path, PathBuf},
        rc::Rc,
    },
};

pub fn make_module() -> ValueMap {
    use Value::{Bool, Empty, Str};

    let mut result = ValueMap::new();

    result.add_fn("create", {
        move |vm, args| match vm.get_args(args) {
            [Str(path)] => {
                let path = Path::new(path.as_str()).to_path_buf();
                match fs::File::create(&path) {
                    Ok(file) => Ok(File::system_file(file, path)),
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
        [Str(path)] => Ok(Bool(fs::canonicalize(path.as_str()).is_ok())),
        _ => runtime_error!("io.exists: Expected path string as argument"),
    });

    result.add_fn("extend_path", |vm, args| match vm.get_args(args) {
        [Str(path), nodes @ ..] => {
            let mut path = PathBuf::from(path.as_str());
            for node in nodes {
                match node {
                    Str(s) => path.push(s.as_str()),
                    other => path.push(other.to_string()),
                }
            }
            Ok(path.to_string_lossy().to_string().into())
        }
        _ => runtime_error!("io.extend_path: Expected path string as first argument"),
    });

    result.add_fn("open", {
        move |vm, args| match vm.get_args(args) {
            [Str(path)] => match fs::canonicalize(path.as_str()) {
                Ok(path) => match fs::File::open(&path) {
                    Ok(file) => Ok(File::system_file(file, path)),
                    Err(e) => {
                        return runtime_error!("io.open: Error while opening path: {}", e);
                    }
                },
                Err(_) => runtime_error!("io.open: Failed to canonicalize path"),
            },
            [unexpected] => runtime_error!(
                "io.open: Expected a String as argument, found '{}'",
                unexpected.type_as_string(),
            ),
            _ => runtime_error!("io.open: Expected a String as argument"),
        }
    });

    result.add_fn("print", |vm, args| {
        let result = match vm.get_args(args) {
            [Str(s)] => vm.stdout().write_line(s.as_str()),
            [value] => vm.stdout().write_line(&value.to_string()),
            [Str(format), format_args @ ..] => {
                let format = format.clone();
                let format_args = format_args.to_vec();
                match format::format_string(vm, &format, &format_args) {
                    Ok(result) => vm.stdout().write_line(&result),
                    Err(error) => Err(error),
                }
            }
            _ => return runtime_error!("io.print: Expected a string as format argument"),
        };

        match result {
            Ok(_) => Ok(Empty),
            Err(e) => Err(e.with_prefix("string.print")),
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

    result.add_fn("stderr", |vm, _| Ok(File::stderr(vm)));
    result.add_fn("stdin", |vm, _| Ok(File::stdin(vm)));
    result.add_fn("stdout", |vm, _| Ok(File::stdout(vm)));

    result.add_fn("temp_dir", {
        |_, _| Ok(Str(std::env::temp_dir().to_string_lossy().as_ref().into()))
    });

    result
}

thread_local!(
    pub static FILE_META: Rc<RefCell<MetaMap>> = {
        use Value::{Empty, Number, Str};

        let mut meta = MetaMap::with_type_name("File");

        meta.add_named_instance_fn_mut("flush", |file: &mut File, _, _| match file.flush() {
            Ok(_) => Ok(Empty),
            Err(e) => Err(e.with_prefix("File.flush")),
        });

        meta.add_named_instance_fn("path", |file: &File, _, _| match file.path() {
            Ok(path) => Ok(Str(path.into())),
            Err(e) => Err(e.with_prefix("File.path")),
        });

        meta.add_named_instance_fn_mut("read_line", |file: &mut File, _, _| {
            match file.read_line() {
                Ok(Some(result)) => {
                    let newline_bytes = if result.ends_with("\r\n") { 2 } else { 1 };
                    Ok(result[..result.len() - newline_bytes].into())
                }
                Ok(None) => Ok(Empty),
                Err(e) => Err(e.with_prefix("File.read_line")),
            }
        });

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

        Rc::new(RefCell::new(meta))
    }
);

/// The File type used in the io module
pub struct File(Rc<dyn KotoFile>);

impl Deref for File {
    type Target = Rc<dyn KotoFile>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl File {
    /// Wraps a file that implements traits typical of a system file in a buffered reader/writer
    pub fn system_file<T>(file: T, path: PathBuf) -> Value
    where
        T: Read + Write + Seek + 'static,
    {
        let result = ExternalValue::with_shared_meta_map(
            Self(Rc::new(BufferedSystemFile::new(file, path))),
            Self::meta(),
        );
        Value::ExternalValue(result)
    }

    fn stderr(vm: &Vm) -> Value {
        let result = ExternalValue::with_shared_meta_map(Self(vm.stderr().clone()), Self::meta());
        Value::ExternalValue(result)
    }

    fn stdin(vm: &Vm) -> Value {
        let result = ExternalValue::with_shared_meta_map(Self(vm.stdin().clone()), Self::meta());
        Value::ExternalValue(result)
    }

    fn stdout(vm: &Vm) -> Value {
        let result = ExternalValue::with_shared_meta_map(Self(vm.stdout().clone()), Self::meta());
        Value::ExternalValue(result)
    }

    fn meta() -> Rc<RefCell<MetaMap>> {
        FILE_META.with(|meta| meta.clone())
    }
}

impl ExternalData for File {
    fn value_type(&self) -> String {
        "File".to_string()
    }
}

impl fmt::Display for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.deref())
    }
}

impl fmt::Debug for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string())
    }
}

struct BufferedSystemFile<T>
where
    T: Write,
{
    file: RefCell<BufferedFile<T>>,
    path: PathBuf,
}

impl<T> BufferedSystemFile<T>
where
    T: Read + Write + Seek,
{
    pub fn new(file: T, path: PathBuf) -> Self {
        Self {
            file: RefCell::new(BufferedFile::new(file)),
            path,
        }
    }
}

impl<T> KotoFile for BufferedSystemFile<T>
where
    T: Read + Write + Seek,
{
    fn path(&self) -> Result<String, RuntimeError> {
        Ok(self.path.to_string_lossy().into())
    }

    fn seek(&self, position: u64) -> Result<(), RuntimeError> {
        self.file
            .borrow_mut()
            .seek(SeekFrom::Start(position))
            .map_err(map_io_err)?;
        Ok(())
    }
}

impl<T> KotoRead for BufferedSystemFile<T>
where
    T: Read + Write,
{
    fn read_line(&self) -> Result<Option<String>, RuntimeError> {
        let mut buffer = String::new();
        match self
            .file
            .borrow_mut()
            .read_line(&mut buffer)
            .map_err(map_io_err)?
        {
            0 => Ok(None),
            _ => Ok(Some(buffer)),
        }
    }

    fn read_to_string(&self) -> Result<String, RuntimeError> {
        let mut buffer = String::new();
        self.file
            .borrow_mut()
            .read_to_string(&mut buffer)
            .map_err(map_io_err)?;
        Ok(buffer)
    }
}

impl<T> KotoWrite for BufferedSystemFile<T>
where
    T: Read + Write,
{
    fn write(&self, bytes: &[u8]) -> Result<(), RuntimeError> {
        self.file.borrow_mut().write(bytes).map_err(map_io_err)?;
        Ok(())
    }

    fn write_line(&self, text: &str) -> Result<(), RuntimeError> {
        let mut borrowed = self.file.borrow_mut();
        borrowed.write(text.as_bytes()).map_err(map_io_err)?;
        borrowed.write("\n".as_bytes()).map_err(map_io_err)?;
        Ok(())
    }

    fn flush(&self) -> Result<(), RuntimeError> {
        self.file.borrow_mut().flush().map_err(map_io_err)
    }
}

impl<T> fmt::Display for BufferedSystemFile<T>
where
    T: Write,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "File({})", self.path.to_string_lossy())
    }
}

impl<T> fmt::Debug for BufferedSystemFile<T>
where
    T: Write,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string())
    }
}

pub fn map_io_err(e: io::Error) -> RuntimeError {
    e.to_string().into()
}
