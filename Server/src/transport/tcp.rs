use std::{io, net::TcpListener};

use log::{error, info};

use crate::server::Server;

pub fn serve(addr: &str, server: Server) -> io::Result<()> {
    let listener = TcpListener::bind(addr)?;

    info!(target: "server", "start GRiD server at {addr}");
    loop {
        match listener.accept() {
            Ok((client, addr)) => server.spawn(client, addr.to_string()),
            Err(err) => error!(target: "server", "failed to accept client: {err}"),
        }
    }
}
