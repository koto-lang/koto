use crate::{KString, KotoFile, KotoRead, KotoWrite, Result, core_lib::io::map_io_err, lazy};
use std::io::{self, IsTerminal, Read, Write};

macro_rules! runtime_error_unavailable {
    ($stream:literal) => {
        crate::runtime_error!(concat!($stream, " is unavailable"))
    };
}

macro_rules! stream {
    (
        get: $get:ident,
        name: $name:literal,
        system: $system:ident,
        unavailable: $unavailable:ident,
    ) => {
        #[doc = concat!("The process's ", $name, " used in Koto")]
        #[derive(Default)]
        pub struct $system {}

        #[doc = concat!("Represents an unavailable ", $name, " stream")]
        #[derive(Default)]
        pub struct $unavailable {}

        const _: () = {
            fn id() -> KString {
        lazy!(KString; concat!("_", $name, "_"))
            }

            impl KotoFile for $system {
                fn id(&self) -> KString {
                    id()
                }

                fn is_terminal(&self) -> bool {
                    io::$get().is_terminal()
                }
            }

            impl KotoFile for $unavailable {
                fn id(&self) -> KString {
                    id()
                }

                fn is_terminal(&self) -> bool {
                    false
                }
            }
        };

        impl KotoWrite for $unavailable {
            fn write(&self, _bytes: &[u8]) -> Result<()> {
                runtime_error_unavailable!($name)
            }

            fn write_line(&self, _text: &str) -> Result<()> {
                runtime_error_unavailable!($name)
            }

            fn flush(&self) -> Result<()> {
                runtime_error_unavailable!($name)
            }
        }

        impl KotoRead for $unavailable {
            fn read_line(&self) -> Result<Option<String>> {
                runtime_error_unavailable!($name)
            }

            fn read_to_string(&self) -> Result<String> {
                runtime_error_unavailable!($name)
            }
        }
    };
}

stream! {
    get: stdin,
    name: "stdin",
    system: SystemStdin,
    unavailable: UnavailableStdin,
}

stream! {
    get: stdout,
    name: "stdout",
    system: SystemStdout,
    unavailable: UnavailableStdout,
}

stream! {
    get: stderr,
    name: "stderr",
    system: SystemStderr,
    unavailable: UnavailableStderr,
}

impl KotoWrite for SystemStdin {}
impl KotoRead for SystemStdin {
    fn read_line(&self) -> Result<Option<String>> {
        let mut result = String::new();
        let bytes_read = io::stdin().read_line(&mut result).map_err(map_io_err)?;
        if bytes_read > 0 {
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    fn read_to_string(&self) -> Result<String> {
        let mut result = String::new();
        io::stdin()
            .lock()
            .read_to_string(&mut result)
            .map_err(map_io_err)?;
        Ok(result)
    }
}

impl KotoRead for SystemStdout {}
impl KotoWrite for SystemStdout {
    fn write(&self, bytes: &[u8]) -> Result<()> {
        io::stdout().write_all(bytes).map_err(map_io_err)
    }

    fn write_line(&self, output: &str) -> Result<()> {
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        handle.write_all(output.as_bytes()).map_err(map_io_err)?;
        handle.write_all("\n".as_bytes()).map_err(map_io_err)
    }

    fn flush(&self) -> Result<()> {
        io::stdout().flush().map_err(map_io_err)
    }
}

impl KotoRead for SystemStderr {}
impl KotoWrite for SystemStderr {
    fn write(&self, bytes: &[u8]) -> Result<()> {
        io::stderr().write_all(bytes).map_err(map_io_err)
    }

    fn write_line(&self, output: &str) -> Result<()> {
        let stderr = io::stderr();
        let mut handle = stderr.lock();
        handle.write_all(output.as_bytes()).map_err(map_io_err)?;
        handle.write_all("\n".as_bytes()).map_err(map_io_err)
    }

    fn flush(&self) -> Result<()> {
        io::stderr().flush().map_err(map_io_err)
    }
}
