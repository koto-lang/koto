//! The `io` core library module

use crate::{derive::*, prelude::*, BufferedFile, Error, Ptr, Result};
use std::{
    fmt, fs,
    io::{self, BufRead, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

/// The initializer for the io module
pub fn make_module() -> KMap {
    use KValue::{Bool, Null, Str};

    let result = KMap::with_type("core.io");

    result.add_fn("create", {
        move |ctx| match ctx.args() {
            [Str(path)] => {
                let path = Path::new(path.as_str()).to_path_buf();
                match fs::File::create(&path) {
                    Ok(file) => Ok(File::system_file(file, path)),
                    Err(error) => runtime_error!("Error while creating file: {error}"),
                }
            }
            unexpected => unexpected_args("|String|", unexpected),
        }
    });

    result.add_fn("current_dir", |ctx| match ctx.args() {
        [] => {
            let result = match std::env::current_dir() {
                Ok(path) => Str(path.to_string_lossy().to_string().into()),
                Err(_) => Null,
            };
            Ok(result)
        }
        unexpected => unexpected_args("||", unexpected),
    });

    result.add_fn("exists", |ctx| match ctx.args() {
        [Str(path)] => Ok(Bool(fs::canonicalize(path.as_str()).is_ok())),
        unexpected => unexpected_args("|String|", unexpected),
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
                        path.push(display_context.result());
                    }
                }
            }
            Ok(path.to_string_lossy().to_string().into())
        }
        unexpected => unexpected_args("|String, Any...|", unexpected),
    });

    result.add_fn("open", {
        |ctx| match ctx.args() {
            [Str(path)] => match fs::canonicalize(path.as_str()) {
                Ok(path) => match fs::File::open(&path) {
                    Ok(file) => Ok(File::system_file(file, path)),
                    Err(error) => runtime_error!("Error while opening path: {error}"),
                },
                Err(_) => runtime_error!("Failed to canonicalize path"),
            },
            unexpected => unexpected_args("|String|", unexpected),
        }
    });

    result.add_fn("print", |ctx| {
        let result = match ctx.args() {
            [Str(s)] => ctx.vm.stdout().write_line(s.as_str()),
            [value] => {
                let value = value.clone();
                match ctx.vm.run_unary_op(crate::UnaryOp::Display, value)? {
                    Str(s) => ctx.vm.stdout().write_line(s.as_str()),
                    unexpected => return unexpected_type("String from @display", &unexpected),
                }
            }
            values @ [_, ..] => {
                let tuple_data = Vec::from(values);
                match ctx
                    .vm
                    .run_unary_op(crate::UnaryOp::Display, KValue::Tuple(tuple_data.into()))?
                {
                    Str(s) => ctx.vm.stdout().write_line(s.as_str()),
                    unexpected => return unexpected_type("String from @display", &unexpected),
                }
            }
            unexpected => return unexpected_args("|Any|, or |Any, Any...|", unexpected),
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
        unexpected => unexpected_args("|String|", unexpected),
    });

    result.add_fn("remove_file", {
        |ctx| match ctx.args() {
            [Str(path)] => {
                let path = Path::new(path.as_str());
                match fs::remove_file(path) {
                    Ok(_) => Ok(KValue::Null),
                    Err(error) => runtime_error!(
                        "io.remove_file: Error while removing file '{}': {error}",
                        path.to_string_lossy(),
                    ),
                }
            }
            unexpected => unexpected_args("|String|", unexpected),
        }
    });

    result.add_fn("stderr", |ctx| match ctx.args() {
        [] => Ok(File::stderr(ctx.vm)),
        unexpected => unexpected_args("||", unexpected),
    });
    result.add_fn("stdin", |ctx| match ctx.args() {
        [] => Ok(File::stdin(ctx.vm)),
        unexpected => unexpected_args("||", unexpected),
    });
    result.add_fn("stdout", |ctx| match ctx.args() {
        [] => Ok(File::stdout(ctx.vm)),
        unexpected => unexpected_args("||", unexpected),
    });

    result.add_fn("temp_dir", |ctx| match ctx.args() {
        [] => Ok(std::env::temp_dir().to_string_lossy().as_ref().into()),
        unexpected => unexpected_args("||", unexpected),
    });

    result
}

/// The File type used in the io module
#[derive(Clone, KotoCopy, KotoType)]
pub struct File(Ptr<dyn KotoFile>);

#[koto_impl(runtime = crate)]
impl File {
    /// Wraps a file that implements traits typical of a system file in a buffered reader/writer
    pub fn system_file<T>(file: T, path: PathBuf) -> KValue
    where
        T: Read + Write + Seek + KotoSend + KotoSync + 'static,
    {
        Self(make_ptr!(BufferedSystemFile::new(file, path))).into()
    }

    fn stderr(vm: &KotoVm) -> KValue {
        Self(vm.stderr().clone()).into()
    }

    fn stdin(vm: &KotoVm) -> KValue {
        Self(vm.stdin().clone()).into()
    }

    fn stdout(vm: &KotoVm) -> KValue {
        Self(vm.stdout().clone()).into()
    }

    #[koto_method]
    fn flush(&mut self) -> Result<KValue> {
        self.0.flush().map(|_| KValue::Null)
    }

    #[koto_method]
    fn path(&self) -> Result<KValue> {
        self.0.path().map(KValue::from)
    }

    #[koto_method]
    fn read_line(&mut self) -> Result<KValue> {
        self.0.read_line().map(|result| match result {
            Some(result) => {
                if !result.is_empty() {
                    let newline_bytes = if result.ends_with("\r\n") { 2 } else { 1 };
                    result[..result.len() - newline_bytes].into()
                } else {
                    KValue::Null
                }
            }
            None => KValue::Null,
        })
    }

    #[koto_method]
    fn read_to_string(&mut self) -> Result<KValue> {
        self.0.read_to_string().map(KValue::from)
    }

    #[koto_method]
    fn seek(&mut self, args: &[KValue]) -> Result<KValue> {
        match args {
            [KValue::Number(n)] => {
                if *n < 0.0 {
                    return runtime_error!("Negative seek positions not allowed");
                }
                self.0.seek(n.into()).map(|_| KValue::Null)
            }
            unexpected => unexpected_args("|Number|", unexpected),
        }
    }

    #[koto_method]
    fn write(ctx: MethodContext<Self>) -> Result<KValue> {
        match ctx.args {
            [value] => {
                let mut display_context = DisplayContext::with_vm(ctx.vm);
                value.display(&mut display_context)?;
                ctx.instance_mut()?
                    .0
                    .write(display_context.result().as_bytes())
                    .map(|_| KValue::Null)
            }
            unexpected => unexpected_args("|Any|", unexpected),
        }
    }

    #[koto_method]
    fn write_line(ctx: MethodContext<Self>) -> Result<KValue> {
        let mut display_context = DisplayContext::with_vm(ctx.vm);
        match ctx.args {
            [] => {}
            [value] => value.display(&mut display_context)?,
            unexpected => return unexpected_args("||, or |Any|", unexpected),
        };
        display_context.append('\n');
        ctx.instance_mut()?
            .0
            .write(display_context.result().as_bytes())
            .map(|_| KValue::Null)
    }
}

impl KotoObject for File {
    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(format!("{}({})", Self::type_static(), self.0.id()));
        Ok(())
    }
}

impl From<File> for KValue {
    fn from(file: File) -> Self {
        KObject::from(file).into()
    }
}

struct BufferedSystemFile<T>
where
    T: Write + KotoSend + KotoSync,
{
    file: KCell<BufferedFile<T>>,
    path: PathBuf,
}

impl<T> BufferedSystemFile<T>
where
    T: Read + Write + Seek + KotoSend + KotoSync,
{
    pub fn new(file: T, path: PathBuf) -> Self {
        Self {
            file: BufferedFile::new(file).into(),
            path,
        }
    }
}

impl<T> KotoFile for BufferedSystemFile<T>
where
    T: Read + Write + Seek + KotoSend + KotoSync,
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
    T: Read + Write + KotoSend + KotoSync,
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
    T: Read + Write + KotoSend + KotoSync,
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
    T: Write + KotoSend + KotoSync,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.path.to_string_lossy())
    }
}

impl<T> fmt::Debug for BufferedSystemFile<T>
where
    T: Write + KotoSend + KotoSync,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string())
    }
}

/// Converts an io::Error into a RuntimeError
pub fn map_io_err(e: io::Error) -> Error {
    e.to_string().into()
}
