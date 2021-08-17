use {
    crate::{core::io::map_io_err, KotoFile, KotoRead, KotoWrite, RuntimeError},
    std::{
        fmt,
        io::{self, Read, Write},
    },
};

/// The default stdin used in Koto
#[derive(Default)]
pub struct DefaultStdin {}

impl KotoFile for DefaultStdin {}
impl KotoWrite for DefaultStdin {}

impl KotoRead for DefaultStdin {
    fn read_line(&self) -> Result<Option<String>, RuntimeError> {
        let mut result = String::new();
        io::stdin().read_line(&mut result).map_err(map_io_err)?;
        Ok(Some(result))
    }

    fn read_to_string(&self) -> Result<String, RuntimeError> {
        let mut result = String::new();
        io::stdin()
            .lock()
            .read_to_string(&mut result)
            .map_err(map_io_err)?;
        Ok(result)
    }
}

impl fmt::Display for DefaultStdin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("_stdin_")
    }
}

impl fmt::Debug for DefaultStdin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string())
    }
}

/// The default stdout used in Koto
#[derive(Default)]
pub struct DefaultStdout {}

impl KotoFile for DefaultStdout {}
impl KotoRead for DefaultStdout {}

impl KotoWrite for DefaultStdout {
    fn write(&self, bytes: &[u8]) -> Result<(), RuntimeError> {
        io::stdout().write_all(bytes).map_err(map_io_err)
    }

    fn write_line(&self, output: &str) -> Result<(), RuntimeError> {
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        handle.write_all(output.as_bytes()).map_err(map_io_err)?;
        handle.write_all("\n".as_bytes()).map_err(map_io_err)
    }

    fn flush(&self) -> Result<(), RuntimeError> {
        io::stdout().flush().map_err(map_io_err)
    }
}

impl fmt::Display for DefaultStdout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("_stdout_")
    }
}

impl fmt::Debug for DefaultStdout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string())
    }
}

/// The default stderr used in Koto
#[derive(Default)]
pub struct DefaultStderr {}

impl KotoFile for DefaultStderr {}
impl KotoRead for DefaultStderr {}

impl KotoWrite for DefaultStderr {
    fn write(&self, bytes: &[u8]) -> Result<(), RuntimeError> {
        io::stdout().write_all(bytes).map_err(map_io_err)
    }

    fn write_line(&self, output: &str) -> Result<(), RuntimeError> {
        let stderr = io::stderr();
        let mut handle = stderr.lock();
        handle.write_all(output.as_bytes()).map_err(map_io_err)?;
        handle.write_all("\n".as_bytes()).map_err(map_io_err)
    }

    fn flush(&self) -> Result<(), RuntimeError> {
        io::stdout().flush().map_err(map_io_err)
    }
}

impl fmt::Display for DefaultStderr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("_stderr_")
    }
}

impl fmt::Debug for DefaultStderr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string())
    }
}
