//! The `io` core library module

mod buffered_file;

pub use buffered_file::BufferedFile;

use {
    super::string::format,
    crate::{
        error::type_error_with_slice, runtime_error, type_error, ExternalData, ExternalValue,
        KotoFile, KotoRead, KotoWrite, MetaMap, MetaMapBuilder, RuntimeError, UnaryOp, Value,
        ValueMap, Vm,
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

/// The initializer for the io module
pub fn make_module() -> ValueMap {
    use Value::{Bool, Null, Str};

    let result = ValueMap::new();

    result.add_fn("create", {
        move |vm, args| match vm.get_args(args) {
            [Str(path)] => {
                let path = Path::new(path.as_str()).to_path_buf();
                match fs::File::create(&path) {
                    Ok(file) => Ok(File::system_file(file, path)),
                    Err(error) => runtime_error!("io.create: Error while creating file: {error}"),
                }
            }
            unexpected => type_error_with_slice("a path String as argument", unexpected),
        }
    });

    result.add_fn("current_dir", |_, _| {
        let result = match std::env::current_dir() {
            Ok(path) => Str(path.to_string_lossy().to_string().into()),
            Err(_) => Null,
        };
        Ok(result)
    });

    result.add_fn("exists", |vm, args| match vm.get_args(args) {
        [Str(path)] => Ok(Bool(fs::canonicalize(path.as_str()).is_ok())),
        unexpected => type_error_with_slice("a path String as argument", unexpected),
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
        unexpected => type_error_with_slice(
            "a path String as argument, followed by some additional path nodes",
            unexpected,
        ),
    });

    result.add_fn("open", {
        |vm, args| match vm.get_args(args) {
            [Str(path)] => match fs::canonicalize(path.as_str()) {
                Ok(path) => match fs::File::open(&path) {
                    Ok(file) => Ok(File::system_file(file, path)),
                    Err(error) => runtime_error!("io.open: Error while opening path: {error}"),
                },
                Err(_) => runtime_error!("io.open: Failed to canonicalize path"),
            },
            unexpected => type_error_with_slice("a path String as argument", unexpected),
        }
    });

    result.add_fn("print", |vm, args| {
        let result = match vm.get_args(args) {
            [Str(s)] => vm.stdout().write_line(s.as_str()),
            [Str(format), format_args @ ..] => {
                let format = format.clone();
                let format_args = format_args.to_vec();
                match format::format_string(vm, &format, &format_args) {
                    Ok(result) => vm.stdout().write_line(&result),
                    Err(error) => Err(error),
                }
            }
            [value] => {
                let value = value.clone();
                match vm.run_unary_op(crate::UnaryOp::Display, value)? {
                    Str(s) => vm.stdout().write_line(s.as_str()),
                    unexpected => return type_error("string from @display", &unexpected),
                }
            }
            values @ [_, ..] => {
                let tuple_data = Vec::from(values);
                match vm.run_unary_op(crate::UnaryOp::Display, Value::Tuple(tuple_data.into()))? {
                    Str(s) => vm.stdout().write_line(s.as_str()),
                    unexpected => return type_error("string from @display", &unexpected),
                }
            }
            unexpected => {
                return type_error_with_slice(
                    "a String as argument, followed by optional additional Values",
                    unexpected,
                )
            }
        };

        result.map(|_| Null)
    });

    result.add_fn("read_to_string", |vm, args| match vm.get_args(args) {
        [Str(path)] => match fs::read_to_string(Path::new(path.as_str())) {
            Ok(result) => Ok(result.into()),
            Err(error) => {
                runtime_error!("io.read_to_string: Unable to read file '{path}': {error}")
            }
        },
        unexpected => type_error_with_slice("a path String as argument", unexpected),
    });

    result.add_fn("remove_file", {
        |vm, args| match vm.get_args(args) {
            [Str(path)] => {
                let path = Path::new(path.as_str());
                match fs::remove_file(path) {
                    Ok(_) => Ok(Value::Null),
                    Err(error) => runtime_error!(
                        "io.remove_file: Error while removing file '{}': {error}",
                        path.to_string_lossy(),
                    ),
                }
            }
            unexpected => type_error_with_slice("a path String as argument", unexpected),
        }
    });

    result.add_fn("stderr", |vm, _| Ok(File::stderr(vm)));
    result.add_fn("stdin", |vm, _| Ok(File::stdin(vm)));
    result.add_fn("stdout", |vm, _| Ok(File::stdout(vm)));

    result.add_fn("temp_dir", {
        |_, _| Ok(std::env::temp_dir().to_string_lossy().as_ref().into())
    });

    result
}

thread_local! {
    /// The meta map used by Files
    pub static FILE_META: Rc<RefCell<MetaMap>> = make_file_meta_map();
}

fn make_file_meta_map() -> Rc<RefCell<MetaMap>> {
    use Value::{Null, Number};

    MetaMapBuilder::<File>::new("File")
        .data_fn_mut("flush", |file| file.flush().map(|_| Null))
        .data_fn_mut("path", |file| file.path().map(Value::from))
        .data_fn_mut("read_line", |file| {
            file.read_line().map(|result| match result {
                Some(result) => {
                    if !result.is_empty() {
                        let newline_bytes = if result.ends_with("\r\n") { 2 } else { 1 };
                        result[..result.len() - newline_bytes].into()
                    } else {
                        Null
                    }
                }
                None => Null,
            })
        })
        .data_fn_mut("read_to_string", |file: &mut File| {
            file.read_to_string().map(Value::from)
        })
        .data_fn_with_args_mut("seek", |file, args| match args {
            [Number(n)] => {
                if *n < 0.0 {
                    return runtime_error!("Negative seek positions not allowed");
                }
                file.seek(n.into()).map(|_| Value::Null)
            }
            unexpected => {
                type_error_with_slice("a non-negative Number as the seek position", unexpected)
            }
        })
        .data_fn_with_args_mut("write", |file, args| match args {
            [value] => file
                .write(value.to_string().as_bytes())
                .map(|_| Value::Null),
            unexpected => type_error_with_slice("a single argument", unexpected),
        })
        .data_fn_with_args_mut("write_line", |file, args| {
            let line = match args {
                [] => "\n".to_string(),
                [value] => format!("{value}\n"),
                unexpected => return type_error_with_slice("a single argument", unexpected),
            };
            file.write(line.as_bytes()).map(|_| Value::Null)
        })
        .data_fn(UnaryOp::Display, |file| {
            Ok(format!("File({})", file.0.id()).into())
        })
        .build()
}

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

impl ExternalData for File {}

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
    fn id(&self) -> String {
        self.path.to_string_lossy().to_string()
    }

    fn path(&self) -> Result<String, RuntimeError> {
        Ok(self.id())
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
        f.write_str(&self.path.to_string_lossy())
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

/// Converts an io::Error into a RuntimeError
pub fn map_io_err(e: io::Error) -> RuntimeError {
    e.to_string().into()
}
