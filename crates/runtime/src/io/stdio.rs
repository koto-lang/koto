use crate::{core_lib::io::map_io_err, Error, KString, KotoFile, KotoRead, KotoWrite};
use std::io::{self, Read, Write};

/// The default stdin used in Koto
#[derive(Default)]
pub struct DefaultStdin {}

impl KotoFile for DefaultStdin {
    fn id(&self) -> KString {
        STDIN_ID.with(|id| id.clone())
    }
}

impl KotoWrite for DefaultStdin {}
impl KotoRead for DefaultStdin {
    fn read_line(&self) -> Result<Option<String>, Error> {
        let mut result = String::new();
        io::stdin().read_line(&mut result).map_err(map_io_err)?;
        Ok(Some(result))
    }

    fn read_to_string(&self) -> Result<String, Error> {
        let mut result = String::new();
        io::stdin()
            .lock()
            .read_to_string(&mut result)
            .map_err(map_io_err)?;
        Ok(result)
    }
}

/// The default stdout used in Koto
#[derive(Default)]
pub struct DefaultStdout {}

impl KotoFile for DefaultStdout {
    fn id(&self) -> KString {
        STDOUT_ID.with(|id| id.clone())
    }
}

impl KotoRead for DefaultStdout {}
impl KotoWrite for DefaultStdout {
    fn write(&self, bytes: &[u8]) -> Result<(), Error> {
        io::stdout().write_all(bytes).map_err(map_io_err)
    }

    fn write_line(&self, output: &str) -> Result<(), Error> {
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        handle.write_all(output.as_bytes()).map_err(map_io_err)?;
        handle.write_all("\n".as_bytes()).map_err(map_io_err)
    }

    fn flush(&self) -> Result<(), Error> {
        io::stdout().flush().map_err(map_io_err)
    }
}

/// The default stderr used in Koto
#[derive(Default)]
pub struct DefaultStderr {}

impl KotoFile for DefaultStderr {
    fn id(&self) -> KString {
        STDERR_ID.with(|id| id.clone())
    }
}

impl KotoRead for DefaultStderr {}
impl KotoWrite for DefaultStderr {
    fn write(&self, bytes: &[u8]) -> Result<(), Error> {
        io::stdout().write_all(bytes).map_err(map_io_err)
    }

    fn write_line(&self, output: &str) -> Result<(), Error> {
        let stderr = io::stderr();
        let mut handle = stderr.lock();
        handle.write_all(output.as_bytes()).map_err(map_io_err)?;
        handle.write_all("\n".as_bytes()).map_err(map_io_err)
    }

    fn flush(&self) -> Result<(), Error> {
        io::stdout().flush().map_err(map_io_err)
    }
}

thread_local! {
    static STDIN_ID: KString = "_stdin_".into();
    static STDOUT_ID: KString = "_stdout_".into();
    static STDERR_ID: KString = "_stderr_".into();
}
