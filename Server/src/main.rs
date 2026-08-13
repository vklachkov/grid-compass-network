use std::{
    io,
    net::{SocketAddr, TcpListener, TcpStream},
    process::ExitCode,
    sync::OnceLock,
    thread,
    time::Instant,
};

use bstr::BStr;

use gridlink::*;
use vfs::Vfs;
use vipc::Vipc;

static STARTED_AT: OnceLock<Instant> = OnceLock::new();

macro_rules! info {
    ($($arg:tt)*) => {
        println!("[{:>10.3}] {}", crate::STARTED_AT.get_or_init(std::time::Instant::now).elapsed().as_secs_f64(), format_args!($($arg)*))
    };
}

macro_rules! error {
    ($($arg:tt)*) => {
        eprintln!("[{:>10.3}] {}", crate::STARTED_AT.get_or_init(std::time::Instant::now).elapsed().as_secs_f64(), format_args!($($arg)*))
    };
}

mod gridlink;
mod mail;
mod sentry;
mod vfs;
mod vipc;

#[derive(PartialEq, Eq)]
enum ProcessFrameResult {
    Continue,
    Disconnect,
}

fn main() -> ExitCode {
    match server() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            error!("Fatal error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn server() -> io::Result<()> {
    let addr = std::env::var("LISTEN_ADDR").map_err(|_| {
        io::Error::new(io::ErrorKind::InvalidInput, "env var LISTEN_ADDR not found")
    })?;

    let listener = TcpListener::bind(&addr)?;

    info!("Start GRiD Server at {addr}");
    loop {
        match listener.accept() {
            Ok((client, addr)) => {
                info!("Accepted client {addr}");
                thread::spawn(move || worker(client, addr));
            }
            Err(err) => {
                error!("Failed to accept client: {err}");
            }
        };
    }
}

fn worker(client: TcpStream, addr: SocketAddr) {
    if let Err(err) = try_worker(client, addr) {
        error!("worker({addr}): fatal error: {err}");
    }
}

fn try_worker(client: TcpStream, addr: SocketAddr) -> io::Result<()> {
    let vfs = Box::new(Vfs::new());

    let mut session = Session {
        client,
        // TODO: automatically choose the connection ID from the available IDs.
        connection_id: 0x7B,
        last_seq_number: 0x1C,
        recv_sequence: 0x1C,
        vipc: Box::new(Vipc::new(vfs)),
    };

    loop {
        match gridlink::RawFrame::read_from_io(&mut session.client) {
            Ok(frame) => {
                // println!("worker({addr}): received new frame");
                if session.process_frame(frame)? == ProcessFrameResult::Disconnect {
                    info!("worker({addr}): disconnect");
                    return Ok(());
                }
            }
            Err(FrameError::UnexpectedEof) => {
                info!("worker({addr}): connection closed");
                return Ok(());
            }
            Err(FrameError::Io(err)) => {
                return Err(err);
            }
            Err(err) => {
                return Err(io::Error::other(err));
            }
        }
    }
}

struct Session {
    client: TcpStream,
    connection_id: u8,
    last_seq_number: u8,
    recv_sequence: u8,
    vipc: Box<Vipc>,
}

impl Session {
    fn process_frame(&mut self, raw: RawFrame) -> io::Result<ProcessFrameResult> {
        match self.process_frame_(raw) {
            Ok(result) => Ok(result),
            Err(FrameError::Io(err)) => Err(err),
            Err(err) => Err(io::Error::other(err)),
        }
    }

    fn process_frame_(&mut self, raw: RawFrame) -> Result<ProcessFrameResult, FrameError> {
        let frame = Frame::try_from_raw(&raw)?;

        info!("session: received {frame:?}");

        match frame.body {
            FrameBody::Rfc(_) => {
                self.recv_sequence = self.last_seq_number;
                Frame::rfc(self.connection_id, self.last_seq_number)
                    .into_raw()
                    .write_to_io(&mut self.client)?;
            }
            FrameBody::Ack(_) => {
                // TODO
            }
            FrameBody::Disc(_) => {
                return Ok(ProcessFrameResult::Disconnect);
            }
            FrameBody::Ping(_) => {
                Frame::ack(self.connection_id, frame.seq_number)
                    .into_raw()
                    .write_to_io(&mut self.client)?;
            }
            FrameBody::Data(data) => {
                let expected = self.recv_sequence.wrapping_add(1);
                if frame.seq_number == self.recv_sequence {
                    info!(
                        "session: ignored duplicate data frame seq={}",
                        frame.seq_number
                    );
                    return Ok(ProcessFrameResult::Continue);
                }
                if frame.seq_number != expected {
                    info!(
                        "session: ignored out-of-sequence data frame seq={}, expected={expected}",
                        frame.seq_number
                    );
                    Frame::ack(self.connection_id, self.recv_sequence)
                        .into_raw()
                        .write_to_io(&mut self.client)?;
                    return Ok(ProcessFrameResult::Continue);
                }

                self.recv_sequence = frame.seq_number;
                Frame::ack(self.connection_id, self.recv_sequence)
                    .into_raw()
                    .write_to_io(&mut self.client)?;
                self.process_data_frame(data)?;
            }
        }

        Ok(ProcessFrameResult::Continue)
    }

    #[rustfmt::skip]
    fn process_data_frame(&mut self, data: &[u8]) -> Result<(), FrameError> {
        let req = DataFrameRequest::try_from_slice(data)?;

        info!("session: received request {req:?}");

        match req {
            DataFrameRequest::Connect { header, path } => {
                self.connect(header.local_path_id, path)
            }
            DataFrameRequest::Disconnect { header, reason } => {
                self.disconnect(header.local_path_id, reason)
            }
            DataFrameRequest::SignOn { properties } => {
                self.sign_on(properties)
            }
            DataFrameRequest::SignOff {} => {
                self.sign_off()
            }
            DataFrameRequest::Msg { header, payload } => {
                self.process_msg(header, payload)
            }
        }
    }

    fn connect(&mut self, remote_path_id: u16, path: &BStr) -> Result<(), FrameError> {
        info!("session: requested connect to {path}");

        // TODO: proper connect to resource.

        let body = DataFrameResponse::Connect {
            header: ConnectHeader {
                local_path_id: 1,
                remote_path_id,
            },
            status: 0, // OK
        };

        self.write_response(&body.to_bytes())?;

        Ok(())
    }

    fn disconnect(&mut self, remote_path_id: u16, _reason: u16) -> Result<(), FrameError> {
        info!("session: requested disconnect");

        // TODO: proper disconnect from resource.

        let body = DataFrameResponse::Disconnect {
            header: ConnectHeader {
                local_path_id: 1,
                remote_path_id,
            },
        };

        self.write_response(&body.to_bytes())?;

        Ok(())
    }

    fn sign_on(&mut self, _properties: Vec<SignOnProperty<'_>>) -> Result<(), FrameError> {
        info!("session: requested sign on");

        // TODO: Check credentials.

        let body = DataFrameResponse::SignOn {
            status: 0, // OK
            server_name: BStr::new("vklachkov server"),
        };

        self.write_response(&body.to_bytes())?;

        Ok(())
    }

    fn sign_off(&mut self) -> Result<(), FrameError> {
        info!("session: requested sign off");

        // TODO: Sign off properly.

        Ok(())
    }

    fn process_msg(&mut self, header: ConnectHeader, payload: &[u8]) -> Result<(), FrameError> {
        for outgoing in self.vipc.process_message(payload)? {
            let response = DataFrameResponse::Msg {
                header: ConnectHeader {
                    local_path_id: header.remote_path_id,
                    remote_path_id: header.local_path_id,
                },
                payload: outgoing.to_bytes(),
            };

            self.write_response(&response.to_bytes())?;
        }

        Ok(())
    }

    fn write_response(&mut self, body: &[u8]) -> Result<(), FrameError> {
        self.last_seq_number = self.last_seq_number.wrapping_add(1);

        Frame::data(EOM_FLAG_ON, self.last_seq_number, body)
            .into_raw()
            .write_to_io(&mut self.client)?;

        Ok(())
    }
}
