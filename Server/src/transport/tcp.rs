use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};

use log::{error, info};

use crate::server::Server;

pub struct TcpTransport(TcpStream);

impl TcpTransport {
    fn new(stream: TcpStream) -> io::Result<Self> {
        stream.set_nonblocking(true)?;
        Ok(Self(stream))
    }
}

impl Read for TcpTransport {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl Write for TcpTransport {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

pub fn serve(addr: &str, server: Server) -> io::Result<()> {
    let listener = TcpListener::bind(addr)?;

    info!(target: "server", "start GRiD server at {addr}");
    loop {
        match listener.accept() {
            Ok((client, addr)) => match TcpTransport::new(client) {
                Ok(client) => server.spawn(client, addr.to_string()),
                Err(err) => error!(target: "server", "failed to configure client {addr}: {err}"),
            },
            Err(err) => error!(target: "server", "failed to accept client: {err}"),
        }
    }
}
