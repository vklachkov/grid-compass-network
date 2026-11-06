use std::io::{self, Cursor, Read, Write};

use crate::shared::Poll;

pub struct MemoryTransport {
    pub input: Cursor<Vec<u8>>,
    pub output: Vec<u8>,
    pub read_limit: usize,
    pub write_limit: usize,
    pub read_error: Option<io::ErrorKind>,
    pub write_error: Option<io::ErrorKind>,
    pub flush_count: usize,
}

impl Default for MemoryTransport {
    fn default() -> Self {
        Self {
            input: Cursor::new(Vec::new()),
            output: Vec::new(),
            read_limit: usize::MAX,
            write_limit: usize::MAX,
            read_error: None,
            write_error: None,
            flush_count: 0,
        }
    }
}

impl MemoryTransport {
    pub fn new(input: impl Into<Vec<u8>>) -> Self {
        Self {
            input: Cursor::new(input.into()),
            ..Self::default()
        }
    }
}

impl Read for MemoryTransport {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if let Some(kind) = self.read_error.take() {
            return Err(kind.into());
        }
        let count = buffer.len().min(self.read_limit);
        self.input.read(&mut buffer[..count])
    }
}

impl Write for MemoryTransport {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if let Some(kind) = self.write_error.take() {
            return Err(kind.into());
        }
        let count = buffer.len().min(self.write_limit);
        self.output.extend_from_slice(&buffer[..count]);
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_count += 1;
        Ok(())
    }
}

pub fn poll_ready<T>(mut poll: impl FnMut() -> Poll<T, io::Error>) -> io::Result<T> {
    for _ in 0..4096 {
        match poll() {
            Poll::Ready(result) => return Ok(result),
            Poll::Failed(err) => return Err(err),
            Poll::Finished => return Err(io::ErrorKind::UnexpectedEof.into()),
            Poll::Pending => {}
        }
    }
    Err(io::ErrorKind::TimedOut.into())
}
