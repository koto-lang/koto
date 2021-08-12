use std::{
    fs::File,
    io::{self, prelude::*, BufReader, BufWriter, Result, Seek, SeekFrom},
};

/// A combination of BufReader and BufWriter
#[derive(Debug)]
pub struct BufferedFile(Reader);
type Reader = BufReader<BufWriterWrapper>;
type Writer = BufWriter<File>;

impl BufferedFile {
    pub fn new(file: File) -> Self {
        Self(BufReader::new(BufWriterWrapper::new(file)))
    }

    fn reader(&mut self) -> &mut Reader {
        &mut self.0
    }

    fn writer(&mut self) -> &mut Writer {
        self.reader().get_mut().writer()
    }
}

impl Seek for BufferedFile {
    fn seek(&mut self, position: SeekFrom) -> Result<u64> {
        self.reader().seek(position)
    }
}

impl Read for BufferedFile {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.reader().read(buf)
    }
}

impl BufRead for BufferedFile {
    fn fill_buf(&mut self) -> Result<&[u8]> {
        self.reader().fill_buf()
    }

    fn consume(&mut self, amount: usize) {
        self.reader().consume(amount)
    }

    fn read_until(&mut self, byte: u8, buffer: &mut Vec<u8>) -> Result<usize> {
        self.reader().read_until(byte, buffer)
    }

    fn read_line(&mut self, string: &mut String) -> Result<usize> {
        self.reader().read_line(string)
    }
}

impl Write for BufferedFile {
    fn write(&mut self, buffer: &[u8]) -> Result<usize> {
        self.writer().write(buffer)
    }

    fn flush(&mut self) -> Result<()> {
        self.writer().flush()
    }
}

#[derive(Debug)]
struct BufWriterWrapper(Writer);

impl BufWriterWrapper {
    fn new(file: File) -> Self {
        Self(BufWriter::new(file))
    }

    fn writer(&mut self) -> &mut Writer {
        &mut self.0
    }
}

impl Seek for BufWriterWrapper {
    fn seek(&mut self, position: SeekFrom) -> Result<u64> {
        self.writer().get_mut().seek(position)
    }
}

impl Read for BufWriterWrapper {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.writer().get_mut().read(buffer)
    }
}
