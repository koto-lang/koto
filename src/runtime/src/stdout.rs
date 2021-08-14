use {
    crate::RuntimeError,
    std::io::{self, Write},
};

/// The trait used by the Koto runtime to write to standard output
pub trait KotoStdout: Send + Sync {
    /// Writes a slice of bytes to the output
    fn write(&self, bytes: &[u8]) -> Result<(), RuntimeError>;

    /// Writes text to the output, and appends a newline
    fn write_line(&self, text: &str) -> Result<(), RuntimeError>;

    /// Flushes any buffered output
    fn flush(&self) -> Result<(), RuntimeError>;
}

#[derive(Default)]
pub struct DefaultStdout {}

impl KotoStdout for DefaultStdout {
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

/// The trait used by the Koto runtime to write to standard error
pub trait KotoStderr: KotoStdout {}

#[derive(Default)]
pub struct DefaultStderr {}

impl KotoStderr for DefaultStderr {}

impl KotoStdout for DefaultStderr {
    fn write(&self, bytes: &[u8]) -> Result<(), RuntimeError> {
        io::stderr().write_all(bytes).map_err(map_io_err)
    }

    fn write_line(&self, output: &str) -> Result<(), RuntimeError> {
        let stderr = io::stderr();
        let mut handle = stderr.lock();
        handle.write_all(output.as_bytes()).map_err(map_io_err)?;
        handle.write_all("\n".as_bytes()).map_err(map_io_err)
    }

    fn flush(&self) -> Result<(), RuntimeError> {
        io::stderr().flush().map_err(map_io_err)
    }
}

fn map_io_err(e: io::Error) -> RuntimeError {
    e.to_string().into()
}
