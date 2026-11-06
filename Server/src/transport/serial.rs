use std::io::{self, Read, Write};
use std::path::Path;
use std::time::Duration;

pub struct SerialTransport {
    port: serial2::SerialPort,
}

impl SerialTransport {
    pub fn open(path: impl AsRef<Path>, settings: impl serial2::IntoSettings) -> io::Result<Self> {
        let mut port = serial2::SerialPort::open(path, settings)?;

        port.set_read_timeout(Duration::ZERO)?;
        port.set_write_timeout(Duration::ZERO)?;

        Ok(Self { port })
    }
}

impl Read for SerialTransport {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.port.read(buf).map_err(nonblocking_error)
    }
}

impl Write for SerialTransport {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.port.write(buf).map_err(nonblocking_error)
    }

    fn flush(&mut self) -> io::Result<()> {
        // Skip flush: serial2::flush would block waiting for the UART.
        Ok(())
    }
}

fn nonblocking_error(err: io::Error) -> io::Error {
    // With zero timeouts, serial2 reports lack of readiness as TimedOut.
    if err.kind() == io::ErrorKind::TimedOut {
        io::ErrorKind::WouldBlock.into()
    } else {
        err
    }
}
