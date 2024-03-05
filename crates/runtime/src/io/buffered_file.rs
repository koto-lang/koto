use std::io::{self, prelude::*, BufReader, BufWriter, Result, SeekFrom};

/// A combination of BufReader and BufWriter
#[derive(Debug)]
pub struct BufferedFile<T: Write>(Reader<T>);

type Reader<T> = BufReader<BufWriterWrapper<T>>;
type Writer<T> = BufWriter<T>;

impl<T> BufferedFile<T>
where
    T: Read + Write,
{
    /// Creates a BufferedFile that wraps the provided Read + Write value
    pub fn new(file: T) -> Self {
        Self(BufReader::new(BufWriterWrapper::new(file)))
    }

    fn reader(&mut self) -> &mut Reader<T> {
        &mut self.0
    }

    fn writer(&mut self) -> &mut Writer<T> {
        self.reader().get_mut().writer()
    }
}

impl<T> Seek for BufferedFile<T>
where
    T: Read + Write + Seek,
{
    fn seek(&mut self, position: SeekFrom) -> Result<u64> {
        self.reader().seek(position)
    }
}

impl<T> Read for BufferedFile<T>
where
    T: Read + Write,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.reader().read(buf)
    }
}

impl<T> BufRead for BufferedFile<T>
where
    T: Read + Write,
{
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

impl<T> Write for BufferedFile<T>
where
    T: Read + Write,
{
    fn write(&mut self, buffer: &[u8]) -> Result<usize> {
        self.writer().write(buffer)
    }

    fn flush(&mut self) -> Result<()> {
        self.writer().flush()
    }
}

#[derive(Debug)]
struct BufWriterWrapper<T>(Writer<T>)
where
    T: Write;

impl<T> BufWriterWrapper<T>
where
    T: Write,
{
    fn new(file: T) -> Self {
        Self(BufWriter::new(file))
    }

    fn writer(&mut self) -> &mut Writer<T> {
        &mut self.0
    }
}

impl<T> Seek for BufWriterWrapper<T>
where
    T: Seek + Write,
{
    fn seek(&mut self, position: SeekFrom) -> Result<u64> {
        self.writer().get_mut().seek(position)
    }
}

impl<T> Read for BufWriterWrapper<T>
where
    T: Read + Write,
{
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.writer().get_mut().read(buffer)
    }
}
