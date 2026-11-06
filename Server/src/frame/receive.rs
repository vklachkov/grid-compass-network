use std::{io, mem};

use arrayvec::ArrayVec;

use crate::frame::RawFrame;
use crate::shared::Poll;
use crate::shared::crc::crc16;

use super::{DLE, ETX, FrameTransport, STX};

#[must_use = "receiving a frame requires polling the task"]
pub struct ReceiveFrameTask<'ft, 't> {
    io: &'ft mut FrameTransport<'t>,
    finished: bool,
}

impl<'ft, 't> ReceiveFrameTask<'ft, 't> {
    pub fn new(io: &'ft mut FrameTransport<'t>) -> Self {
        io.receiver.reset();
        Self {
            io,
            finished: false,
        }
    }

    pub fn poll(&mut self) -> Poll<RawFrame, super::Error> {
        if self.finished {
            return Poll::Finished;
        }

        let reader = &mut self.io.transport;
        let result = self.io.receiver.poll(reader);

        self.finished = !result.is_pending();

        result
    }
}

#[derive(PartialEq, Eq)]
enum State {
    WaitingForStartDle,
    WaitingForStartStx,
    ReadingPayload,
    ReadingPayloadEscape,
    ReadingCrcLow,
    ReadingCrcHigh {
        low: u8,
    },
    Done,
}

pub struct Receiver {
    state: State,
    buffer: Box<ArrayVec<u8, { RawFrame::MAX_SIZE }>>,
}

impl Receiver {
    pub fn new() -> Self {
        Self {
            state: State::WaitingForStartDle,
            buffer: Box::default(),
        }
    }

    fn reset(&mut self) {
        self.state = State::WaitingForStartDle;
        self.buffer.clear();
    }

    fn poll(&mut self, source: &mut impl io::BufRead) -> Poll<RawFrame, super::Error> {
        if let State::Done = self.state {
            return Poll::Finished;
        }

        let bytes = match self.buffered_bytes(source) {
            Ok(Some(bytes)) => {
                bytes //
            }
            Ok(None) => {
                return Poll::Pending; //
            }
            Err(err) => {
                self.state = State::Done;
                return Poll::Failed(err);
            }
        };

        let mut consumed = 0usize;
        let mut result = Poll::Pending;

        for &byte in bytes {
            result = self.process_byte(byte);
            consumed += 1;

            if !result.is_pending() {
                break;
            }
        }

        source.consume(consumed);

        result
    }

    fn buffered_bytes<'s>(
        &self,
        source: &'s mut impl io::BufRead,
    ) -> Result<Option<&'s [u8]>, super::Error> {
        match source.fill_buf() {
            Ok([]) => {
                let error = if self.state == State::WaitingForStartDle {
                    super::Error::ConnectionClosed
                } else {
                    super::Error::UnexpectedEof
                };

                Err(error)
            }
            Ok(bytes) => {
                Ok(Some(bytes)) //
            }
            Err(err)
                if err.kind() == io::ErrorKind::WouldBlock
                    || err.kind() == io::ErrorKind::Interrupted =>
            {
                Ok(None)
            }
            Err(err) => Err(super::Error::Io(err)),
        }
    }

    fn process_byte(&mut self, byte: u8) -> Poll<RawFrame, super::Error> {
        match self.state {
            State::WaitingForStartDle => {
                if byte == DLE {
                    self.state = State::WaitingForStartStx;
                }

                return Poll::Pending;
            }
            State::WaitingForStartStx => {
                if byte == DLE {
                    // Repeated DLE bytes keep the start prefix armed, as in PdlFrameIO.
                } else if byte == STX {
                    self.state = State::ReadingPayload;
                } else {
                    self.state = State::WaitingForStartDle;
                }

                return Poll::Pending;
            }
            State::ReadingPayload => {
                if byte == DLE {
                    self.state = State::ReadingPayloadEscape;
                    return Poll::Pending;
                }

                if self.buffer.try_push(byte).is_err() {
                    self.state = State::Done;
                    return Poll::Failed(super::Error::TooBigFrame);
                }

                return Poll::Pending;
            }
            State::ReadingPayloadEscape => match byte {
                DLE => {
                    if self.buffer.try_push(byte).is_ok() {
                        self.state = State::ReadingPayload;
                        return Poll::Pending;
                    } else {
                        self.state = State::Done;
                        return Poll::Failed(super::Error::TooBigFrame);
                    }
                }
                STX => {
                    self.buffer.clear();

                    self.state = State::ReadingPayload;
                    return Poll::Pending;
                }
                ETX => {
                    self.state = State::ReadingCrcLow;
                    return Poll::Pending;
                }
                _ => {
                    self.state = State::Done;
                    return Poll::Failed(super::Error::MalformedEscape);
                }
            },
            State::ReadingCrcLow => {
                self.state = State::ReadingCrcHigh { low: byte };
                return Poll::Pending;
            }
            State::ReadingCrcHigh { low } => {
                self.state = State::Done;

                let expected_crc16 = u16::from_le_bytes([low, byte]);
                let calc_crc16 = crc16(&self.buffer);

                return if expected_crc16 == calc_crc16 {
                    Poll::Ready(RawFrame::new(mem::take(&mut self.buffer)))
                } else {
                    Poll::Failed(super::Error::CrcMismatch)
                };
            }
            State::Done => unreachable!(),
        }
    }
}
