//! The `io` core library module

use super::string::format;
use crate::{prelude::*, BufferedFile, Result};
use std::{
    cell::RefCell,
    fmt, fs,
    io::{self, BufRead, Read, Seek, SeekFrom, Write},
    ops::Deref,
    path::{Path, PathBuf},
    rc::Rc,
};

/// The initializer for the io module
pub fn make_module() -> KMap {
    use Value::{Bool, Null, Str};

    let result = KMap::with_type("core.io");

    result.add_fn("create", {
        move |ctx| match ctx.args() {
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

    result.add_fn("current_dir", |_| {
        let result = match std::env::current_dir() {
            Ok(path) => Str(path.to_string_lossy().to_string().into()),
            Err(_) => Null,
        };
        Ok(result)
    });

    result.add_fn("exists", |ctx| match ctx.args() {
        [Str(path)] => Ok(Bool(fs::canonicalize(path.as_str()).is_ok())),
        unexpected => type_error_with_slice("a path String as argument", unexpected),
    });

    result.add_fn("extend_path", |ctx| match ctx.args() {
        [Str(path), nodes @ ..] => {
            let mut path = PathBuf::from(path.as_str());

            for node in nodes {
                match node {
                    Str(s) => path.push(s.as_str()),
                    other => {
                        let mut display_context = DisplayContext::with_vm(ctx.vm);
                        other.display(&mut display_context)?;
                        path.push(&display_context.result());
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
        |ctx| match ctx.args() {
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

    result.add_fn("print", |ctx| {
        let result = match ctx.args() {
            [Str(s)] => ctx.vm.stdout().write_line(s.as_str()),
            [Str(format), format_args @ ..] => {
                let format = format.clone();
                let format_args = format_args.to_vec();
                match format::format_string(ctx.vm, &format, &format_args) {
                    Ok(result) => ctx.vm.stdout().write_line(&result),
                    Err(error) => Err(error),
                }
            }
            [value] => {
                let value = value.clone();
                match ctx.vm.run_unary_op(crate::UnaryOp::Display, value)? {
                    Str(s) => ctx.vm.stdout().write_line(s.as_str()),
                    unexpected => return type_error("string from @display", &unexpected),
                }
            }
            values @ [_, ..] => {
                let tuple_data = Vec::from(values);
                match ctx
                    .vm
                    .run_unary_op(crate::UnaryOp::Display, Value::Tuple(tuple_data.into()))?
                {
                    Str(s) => ctx.vm.stdout().write_line(s.as_str()),
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

    result.add_fn("read_to_string", |ctx| match ctx.args() {
        [Str(path)] => match fs::read_to_string(Path::new(path.as_str())) {
            Ok(result) => Ok(result.into()),
            Err(error) => {
                runtime_error!("io.read_to_string: Unable to read file '{path}': {error}")
            }
        },
        unexpected => type_error_with_slice("a path String as argument", unexpected),
    });

    result.add_fn("remove_file", {
        |ctx| match ctx.args() {
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

    result.add_fn("stderr", |ctx| Ok(File::stderr(ctx.vm)));
    result.add_fn("stdin", |ctx| Ok(File::stdin(ctx.vm)));
    result.add_fn("stdout", |ctx| Ok(File::stdout(ctx.vm)));

    result.add_fn("temp_dir", {
        |_| Ok(std::env::temp_dir().to_string_lossy().as_ref().into())
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
    fn object_type(&self) -> KString {
        FILE_TYPE_STRING.with(|t| t.clone())
    }

    fn copy(&self) -> Object {
        self.clone().into()
    }

    fn lookup(&self, key: &ValueKey) -> Option<Value> {
        FILE_ENTRIES.with(|entries| entries.get(key).cloned())
    }

    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(format!("{}({})", Self::TYPE, self.id()));
        Ok(())
    }
}

impl From<File> for Value {
    fn from(file: File) -> Self {
        Object::from(file).into()
    }
}

fn file_entries() -> ValueMap {
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
                let mut display_context = DisplayContext::with_vm(ctx.vm);
                value.display(&mut display_context)?;
                ctx.instance_mut()?
                    .write(display_context.result().as_bytes())
                    .map(|_| Null)
            }
            unexpected => type_error_with_slice("a single argument", unexpected),
        })
        .method("write_line", |ctx| {
            let mut display_context = DisplayContext::with_vm(ctx.vm);
            match ctx.args {
                [] => {}
                [value] => value.display(&mut display_context)?,
                unexpected => return type_error_with_slice("a single argument", unexpected),
            };
            display_context.append('\n');
            ctx.instance_mut()?
                .write(display_context.result().as_bytes())
                .map(|_| Null)
        })
        .build()
}

thread_local! {
    static FILE_TYPE_STRING: KString = File::TYPE.into();
    static FILE_ENTRIES: ValueMap = file_entries();
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
    fn id(&self) -> KString {
        self.path.to_string_lossy().to_string().into()
    }

    fn path(&self) -> Result<KString> {
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
