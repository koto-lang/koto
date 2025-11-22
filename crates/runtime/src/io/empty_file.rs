use koto_memory::lazy;
use koto_parser::KString;

use crate::{KotoFile, KotoRead, KotoWrite, Result};

impl KotoFile for std::io::Empty {
    fn id(&self) -> KString {
        lazy!(KString; "_empty_")
    }
}

impl KotoRead for std::io::Empty {
    fn read_line(&self) -> Result<Option<String>> {
        Ok(None)
    }

    fn read_to_string(&self) -> Result<String> {
        Ok(String::new())
    }
}

impl KotoWrite for std::io::Empty {
    fn write(&self, _bytes: &[u8]) -> Result<()> {
        Ok(())
    }

    fn write_line(&self, _text: &str) -> Result<()> {
        Ok(())
    }

    fn flush(&self) -> Result<()> {
        Ok(())
    }
}
