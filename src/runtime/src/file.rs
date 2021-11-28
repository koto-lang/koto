use {
    crate::{runtime_error, RuntimeError},
    std::fmt::{Debug, Display},
};

/// A trait used for file-like-things in Koto
pub trait KotoFile: KotoRead + KotoWrite + Display + Debug {
    /// Returns the path of the file
    fn path(&self) -> Result<String, RuntimeError> {
        runtime_error!("unsupported for this file type")
    }

    /// Seeks to the provided position in the file
    fn seek(&self, _position: u64) -> Result<(), RuntimeError> {
        runtime_error!("unsupported for this file type")
    }
}

/// A trait that defines the read operations of a [KotoFile]
pub trait KotoRead {
    /// Returns the next line from the file
    ///
    /// If None is returned then the end of the file has been reached.
    fn read_line(&self) -> Result<Option<String>, RuntimeError> {
        runtime_error!("unsupported for this file type")
    }

    /// Returns the contents of the file from the current position
    fn read_to_string(&self) -> Result<String, RuntimeError> {
        runtime_error!("unsupported for this file type")
    }
}

/// A trait that defines the write operations of a [KotoFile]
pub trait KotoWrite {
    /// Writes bytes to the file
    fn write(&self, _bytes: &[u8]) -> Result<(), RuntimeError> {
        runtime_error!("unsupported for this file type")
    }

    /// Writes text to the file, and appends a newline
    fn write_line(&self, _text: &str) -> Result<(), RuntimeError> {
        runtime_error!("unsupported for this file type")
    }

    /// Flushes any remaining buffered output
    fn flush(&self) -> Result<(), RuntimeError> {
        runtime_error!("unsupported for this file type")
    }
}
