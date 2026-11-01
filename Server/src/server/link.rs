use bstr::BStr;
use log::{debug, trace, warn};
use parking_lot::Mutex;

use super::{
    ConnectionId,
    frame::{EOM_FLAG_ON, Frame, FrameBody},
    raw_frame::RawFrame,
};
use crate::{shared::FrameError, transport::Transport};

/// Both sides start at the sequence announced in the RFC response. The original
/// server drew it from one shared generator, reproduced here.
pub(super) fn next_initial_seq_number() -> u8 {
    static STATE: Mutex<u16> = Mutex::new(5);

    let mut state = STATE.lock();
    *state = state.wrapping_mul(2005).wrapping_add(4227);

    *state as u8
}

pub(super) struct PdlLink<T: Transport> {
    transport: T,
    connection_id: ConnectionId,
    last_seq_number: u8,
    recv_sequence: u8,
    frame: Vec<u8>,
}

#[cfg(test)]
impl<T: Transport> PdlLink<T> {
    pub(super) fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    pub(super) fn next_response_ack(&self) -> Vec<u8> {
        let mut frame = Vec::new();
        Frame::ack(
            self.connection_id.get(),
            self.last_seq_number.wrapping_add(1),
        )
        .write_into(&mut frame);
        frame
    }
}

impl<T: Transport> PdlLink<T> {
    pub(super) fn new(transport: T, connection_id: ConnectionId, initial_sequence: u8) -> Self {
        Self {
            transport,
            connection_id,
            last_seq_number: initial_sequence,
            recv_sequence: initial_sequence,
            frame: Vec::new(),
        }
    }

    pub(super) fn receive_data(&mut self) -> Result<Option<Vec<u8>>, FrameError> {
        loop {
            let mut raw = match self.read_frame() {
                Ok(raw) => raw,
                Err(FrameError::UnexpectedEof) => return Ok(None),
                Err(err) => return Err(err),
            };
            let frame = Frame::try_from_raw(&raw)?;

            trace!(target: "session", "received {frame:?}");

            match frame.body {
                FrameBody::Rfc(body) => {
                    debug!(
                        target: "session",
                        "requested a connection, version {:?}, seq={}",
                        BStr::new(&body.version),
                        self.last_seq_number,
                    );
                    self.recv_sequence = self.last_seq_number;
                    self.write_frame(Frame::rfc)?;
                }
                FrameBody::Ack(_) => {
                    trace!(target: "session", "acknowledged seq={}", frame.seq_number);
                }
                FrameBody::Disc(_) => {
                    debug!(target: "session", "requested a link disconnect");
                    return Ok(None);
                }
                FrameBody::Ping(_) => {
                    trace!(target: "session", "pinged, seq={}", self.recv_sequence);
                    self.write_frame(Frame::ack)?;
                }
                FrameBody::Data(data) => {
                    let expected = self.recv_sequence.wrapping_add(1);
                    if frame.seq_number == self.recv_sequence {
                        trace!(
                            target: "session",
                            "ignored duplicate data frame seq={}",
                            frame.seq_number
                        );
                        continue;
                    }
                    if frame.seq_number != expected {
                        warn!(
                            target: "session",
                            "ignored out-of-sequence data frame seq={}, expected={expected}",
                            frame.seq_number
                        );
                        self.write_frame(Frame::ack)?;
                        continue;
                    }

                    let header_length = raw.data.len() - data.len();
                    self.recv_sequence = frame.seq_number;
                    self.write_frame(Frame::ack)?;
                    raw.data.drain(..header_length);
                    return Ok(Some(raw.data));
                }
            }
        }
    }

    pub(super) fn send_data(&mut self, data: &[u8]) -> Result<(), FrameError> {
        self.last_seq_number = self.last_seq_number.wrapping_add(1);
        self.frame.clear();
        Frame::data(EOM_FLAG_ON, self.last_seq_number, data).write_into(&mut self.frame);
        RawFrame::write_data_to_io(&self.frame, &mut self.transport)?;
        self.read_ack()
    }

    fn read_frame(&mut self) -> Result<RawFrame, FrameError> {
        RawFrame::read_from_io(&mut self.transport)
    }

    fn read_ack(&mut self) -> Result<(), FrameError> {
        let raw = self.read_frame()?;
        let frame = Frame::try_from_raw(&raw)?;
        if let FrameBody::Ack(_) = frame.body {
            Ok(())
        } else {
            Err(FrameError::Validation {
                reason: "expected ack, got something else".to_owned(),
            })
        }
    }

    fn write_frame<'a>(
        &mut self,
        frame: impl FnOnce(u8, u8) -> Frame<'a>,
    ) -> Result<(), FrameError> {
        self.frame.clear();
        let frame = frame(self.connection_id.get(), self.recv_sequence);
        frame.write_into(&mut self.frame);
        RawFrame::write_data_to_io(&self.frame, &mut self.transport)
    }
}
