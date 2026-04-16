//! A Koto language module for working with temporary files

cfg_select! {
    feature = "plugin" => {
        use koto_plugin as runtime;
    }
    _ => {
        use koto_runtime as runtime;
    }
}

use runtime::{Result, derive::*, prelude::*};
use std::{
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tempfile::NamedTempFile;

#[cfg(feature = "plugin")]
koto_plugin::export_plugin!(make_module);

pub fn make_module() -> KMap {
    let result = KMap::with_type("temp_file");

    result.add_fn("temp_file", |ctx| match ctx.args() {
        [] => NamedTempFile::new()
            .map(File::make_value)
            .map_err(map_io_err),
        unexpected => unexpected_args("||", unexpected),
    });

    result
}

fn value_to_text(vm: &KotoVm, value: &KValue) -> Result<String> {
    let mut display_context = DisplayContext::with_vm(vm);
    value.display(&mut display_context)?;
    Ok(display_context.result())
}

#[derive(Clone, KotoCopy, KotoType)]
#[koto(runtime = runtime, type_name = "File")]
struct File {
    file: Arc<Mutex<NamedTempFile>>,
    path: PathBuf,
}

#[koto_impl(runtime = runtime)]
impl File {
    fn make_value(file: NamedTempFile) -> KValue {
        let path = file.path().to_path_buf();
        KObject::from(Self {
            file: Arc::new(Mutex::new(file)),
            path,
        })
        .into()
    }

    #[koto_method]
    fn flush(&mut self) -> Result<()> {
        self.with_file_mut(|file| file.flush().map_err(map_io_err))
    }

    #[koto_method]
    fn is_terminal(&self) -> bool {
        false
    }

    #[koto_method]
    fn path(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }

    #[koto_method]
    fn read_line(&mut self) -> Result<KValue> {
        self.with_file_mut(|file| {
            let mut reader = BufReader::new(file);
            let mut result = String::new();
            match reader.read_line(&mut result).map_err(map_io_err)? {
                0 => Ok(KValue::Null),
                _ => {
                    if result.ends_with('\n') {
                        let newline_bytes = if result.ends_with("\r\n") { 2 } else { 1 };
                        result.truncate(result.len() - newline_bytes);
                    }
                    Ok(result.into())
                }
            }
        })
    }

    #[koto_method]
    fn read_to_string(&mut self) -> Result<String> {
        self.with_file_mut(|file| {
            let mut result = String::new();
            file.read_to_string(&mut result).map_err(map_io_err)?;
            Ok(result)
        })
    }

    #[koto_method]
    fn seek(&mut self, args: &[KValue]) -> Result<KValue> {
        match args {
            [KValue::Number(n)] => {
                if *n < 0.0 {
                    return runtime_error!("negative seek positions not allowed");
                }
                let position = i64::from(n);

                self.with_file_mut(|file| {
                    file.seek(SeekFrom::Start(position as u64))
                        .map_err(map_io_err)?;
                    Ok(KValue::Null)
                })
            }
            unexpected => unexpected_args("|Number|", unexpected),
        }
    }

    #[koto_method]
    fn write(ctx: MethodContext<Self>) -> Result<KValue> {
        let text = match ctx.args() {
            [value] => value_to_text(ctx.vm(), value)?,
            unexpected => return unexpected_args("|Any|", unexpected),
        };

        ctx.instance_mut()?.with_file_mut(|file| {
            file.write_all(text.as_bytes()).map_err(map_io_err)?;
            Ok(KValue::Null)
        })
    }

    #[koto_method]
    fn write_line(ctx: MethodContext<Self>) -> Result<KValue> {
        let text = match ctx.args() {
            [] => String::new(),
            [value] => value_to_text(ctx.vm(), value)?,
            unexpected => return unexpected_args("||, or |Any|", unexpected),
        };

        ctx.instance_mut()?.with_file_mut(|file| {
            file.write_all(text.as_bytes()).map_err(map_io_err)?;
            file.write_all(b"\n").map_err(map_io_err)?;
            Ok(KValue::Null)
        })
    }

    fn with_file_mut<T>(&self, f: impl FnOnce(&mut NamedTempFile) -> Result<T>) -> Result<T> {
        let mut file = self
            .file
            .lock()
            .map_err(|_| runtime::Error::from("temp file lock poisoned"))?;
        f(&mut file)
    }
}

impl KotoObjectOps<runtime::Backend> for File {
    fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
        ctx.append(format!("File({})", self.path.to_string_lossy()));
        Ok(())
    }
}

fn map_io_err(error: std::io::Error) -> runtime::Error {
    error.to_string().into()
}
