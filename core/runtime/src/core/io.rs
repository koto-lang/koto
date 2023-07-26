//! The `io` core library module

mod buffered_file;

pub use buffered_file::BufferedFile;

use {
    super::string::format,
    crate::{prelude::*, Result},
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
            let mut display_vm = None;
            let mut node_string = String::new();
            for node in nodes {
                match node {
                    Str(s) => path.push(s.as_str()),
                    other => {
                        node_string.clear();
                        other.display(
                            &mut node_string,
                            display_vm.get_or_insert_with(|| vm.spawn_shared_vm()),
                            KotoDisplayOptions::default(),
                        )?;
                        path.push(&node_string);
                    }
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

/// The File type used in the io module
#[derive(Clone)]
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
        Self(Rc::new(BufferedSystemFile::new(file, path))).into()
    }

    fn stderr(vm: &Vm) -> Value {
        Self(vm.stderr().clone()).into()
    }

    fn stdin(vm: &Vm) -> Value {
        Self(vm.stdin().clone()).into()
    }

    fn stdout(vm: &Vm) -> Value {
        Self(vm.stdout().clone()).into()
    }
}

impl KotoType for File {
    const TYPE: &'static str = "File";
}

impl KotoObject for File {
    fn object_type(&self) -> ValueString {
        FILE_TYPE_STRING.with(|t| t.clone())
    }

    fn lookup(&self, key: &ValueKey) -> Option<Value> {
        FILE_ENTRIES.with(|entries| entries.get(key).cloned())
    }

    fn display(&self, out: &mut String, _: &mut Vm, _: KotoDisplayOptions) -> Result<()> {
        out.push_str(&format!("{}({})", Self::TYPE, self.id()));
        Ok(())
    }
}

impl From<File> for Value {
    fn from(file: File) -> Self {
        Object::from(file).into()
    }
}

fn file_entries() -> DataMap {
    use Value::*;

    ObjectEntryBuilder::<File>::new()
        .method("flush", |ctx| ctx.instance_mut()?.flush().map(|_| Null))
        .method("path", |ctx| ctx.instance()?.path().map(Value::from))
        .method("read_line", |ctx| {
            ctx.instance_mut()?.read_line().map(|result| match result {
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
        .method("read_to_string", |ctx| {
            ctx.instance_mut()?.read_to_string().map(Value::from)
        })
        .method("seek", |ctx| match ctx.args {
            [Number(n)] => {
                if *n < 0.0 {
                    return runtime_error!("Negative seek positions not allowed");
                }
                ctx.instance_mut()?.seek(n.into()).map(|_| Null)
            }
            unexpected => {
                type_error_with_slice("a non-negative Number as the seek position", unexpected)
            }
        })
        .method("write", |ctx| match ctx.args {
            [value] => {
                let mut string_to_write = String::new();
                value.display(
                    &mut string_to_write,
                    &mut ctx.vm.spawn_shared_vm(),
                    KotoDisplayOptions::default(),
                )?;
                ctx.instance_mut()?
                    .write(string_to_write.as_bytes())
                    .map(|_| Null)
            }
            unexpected => type_error_with_slice("a single argument", unexpected),
        })
        .method("write_line", |ctx| {
            let mut string_to_write = String::new();
            match ctx.args {
                [] => {}
                [value] => {
                    value.display(
                        &mut string_to_write,
                        &mut ctx.vm.spawn_shared_vm(),
                        KotoDisplayOptions::default(),
                    )?;
                }
                unexpected => return type_error_with_slice("a single argument", unexpected),
            };
            string_to_write.push('\n');
            ctx.instance_mut()?
                .write(string_to_write.as_bytes())
                .map(|_| Null)
        })
        .build()
}

thread_local! {
    static FILE_TYPE_STRING: ValueString = File::TYPE.into();
    static FILE_ENTRIES: DataMap = file_entries();
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
    fn id(&self) -> ValueString {
        self.path.to_string_lossy().to_string().into()
    }

    fn path(&self) -> Result<ValueString> {
        Ok(self.id())
    }

    fn seek(&self, position: u64) -> Result<()> {
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
    fn read_line(&self) -> Result<Option<String>> {
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

    fn read_to_string(&self) -> Result<String> {
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
    fn write(&self, bytes: &[u8]) -> Result<()> {
        self.file.borrow_mut().write(bytes).map_err(map_io_err)?;
        Ok(())
    }

    fn write_line(&self, text: &str) -> Result<()> {
        let mut borrowed = self.file.borrow_mut();
        borrowed.write(text.as_bytes()).map_err(map_io_err)?;
        borrowed.write("\n".as_bytes()).map_err(map_io_err)?;
        Ok(())
    }

    fn flush(&self) -> Result<()> {
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
