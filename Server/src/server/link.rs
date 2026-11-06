use arrayvec::ArrayVec;
use bstr::BStr;
use log::{debug, trace, warn};
use parking_lot::Mutex;

use super::{
    ConnectionId,
    frame::{EOM_FLAG_ON, Frame, FrameBody},
};

use crate::{
    frame::{self, FrameTransport, RawFrame},
    shared::{FrameError, Poll},
    transport::Transport,
};

/// Both sides start at the sequence announced in the RFC response. The original
/// server drew it from one shared generator, reproduced here.
pub(super) fn next_initial_seq_number() -> u8 {
    static STATE: Mutex<u16> = Mutex::new(5);

    let mut state = STATE.lock();
    *state = state.wrapping_mul(2005).wrapping_add(4227);

    *state as u8
}

pub(super) struct FrameLink<'t> {
    io: FrameTransport<'t>,
    connection_id: ConnectionId,
    last_seq_number: u8,
    recv_sequence: u8,
}

impl<'t> FrameLink<'t> {
    pub(super) fn new(
        transport: &'t mut dyn Transport,
        connection_id: ConnectionId,
        initial_sequence: u8,
    ) -> Self {
        Self {
            io: FrameTransport::new(transport),
            connection_id,
            last_seq_number: initial_sequence,
            recv_sequence: initial_sequence,
        }
    }

    pub(super) fn receive_data(&mut self) -> Result<Option<Vec<u8>>, FrameError> {
        loop {
            let frame = match self.receive_frame() {
                Ok(Some(frame)) => frame,
                Ok(None) => return Ok(None),
                Err(err) => return Err(err),
            };

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

                    self.recv_sequence = frame.seq_number;
                    self.write_frame(Frame::ack)?;
                    return Ok(Some(data.into_owned()));
                }
            }
        }
    }

    pub(super) fn send_data(&mut self, data: &[u8]) -> Result<(), FrameError> {
        self.last_seq_number = self.last_seq_number.wrapping_add(1);
        self.send_frame(Frame::data(EOM_FLAG_ON, self.last_seq_number, data.into()))?;
        self.read_ack()
    }

    fn read_ack(&mut self) -> Result<(), FrameError> {
        let frame = self.receive_frame()?.ok_or(FrameError::UnexpectedEof)?;
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
        let frame = frame(self.connection_id.get(), self.recv_sequence);
        self.send_frame(frame)
    }

    // Temporary synchronous adapters until the link runtime drives polling.
    fn receive_frame(&mut self) -> Result<Option<Frame<'static>>, FrameError> {
        let mut task = self.io.receive_frame();
        loop {
            match task.poll() {
                Poll::Ready(frame) => {
                    return Frame::deserialize(frame.to_vec().into()).map(Some);
                }
                Poll::Failed(frame::Error::ConnectionClosed) => {
                    return Ok(None);
                }
                Poll::Failed(err) => return Err(FrameError::from(err)),
                Poll::Pending => {}
                Poll::Finished => unreachable!(),
            }
        }
    }

    fn send_frame(&mut self, frame: Frame<'_>) -> Result<(), FrameError> {
        if frame.serialized_len() > RawFrame::MAX_SIZE {
            return Err(FrameError::FrameTooLarge {
                max: RawFrame::MAX_SIZE,
            });
        }
        let mut bytes = Vec::with_capacity(frame.serialized_len());
        frame.write_into(&mut bytes)?;
        let mut buffer = Box::<ArrayVec<u8, { RawFrame::MAX_SIZE }>>::default();
        buffer
            .try_extend_from_slice(&bytes)
            .map_err(|_| FrameError::FrameTooLarge {
                max: RawFrame::MAX_SIZE,
            })?;
        let mut task = self.io.send_frame(RawFrame::new(buffer));
        loop {
            match task.poll() {
                Poll::Ready(()) => return Ok(()),
                Poll::Failed(err) => return Err(FrameError::from(err)),
                Poll::Pending => {}
                Poll::Finished => unreachable!(),
            }
        }
    }
}
