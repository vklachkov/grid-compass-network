use std::io;

use crate::frame::{EncodedFrame, RawFrame};
use crate::shared::Poll;

use super::FrameTransport;

#[must_use = "sending a frame requires polling the task"]
pub struct SendFrameTask<'ft, 't> {
    io: &'ft mut FrameTransport<'t>,
    finished: bool,
}

impl<'ft, 't> SendFrameTask<'ft, 't> {
    pub fn new(io: &'ft mut FrameTransport<'t>, frame: RawFrame) -> Self {
        io.sender.reset(frame);
        Self {
            io,
            finished: false,
        }
    }

    pub fn poll(&mut self) -> Poll<(), super::Error> {
        if self.finished {
            return Poll::Finished;
        }

        let writer = self.io.transport.get_mut();
        let result = self.io.sender.poll(writer);

        self.finished = !result.is_pending();

        result
    }
}

enum State {
    Idle,
    Writing {
        frame: EncodedFrame,
        offset: usize,
    },
}

pub struct Sender {
    state: State,
}

impl Sender {
    pub fn new() -> Self {
        Self { state: State::Idle }
    }

    fn reset(&mut self, frame: RawFrame) {
        self.state = State::Writing {
            frame: frame.encode(),
            offset: 0,
        };
    }

    fn poll(&mut self, dest: &mut impl io::Write) -> Poll<(), super::Error> {
        let State::Writing { frame, offset } = &mut self.state else {
            return Poll::Finished;
        };

        let remainder = &frame[*offset..];

        match dest.write(remainder) {
            Ok(0) => {
                self.state = State::Idle;
                Poll::Failed(super::Error::Io(io::ErrorKind::WriteZero.into()))
            }
            Ok(count) => {
                *offset += count;
                if *offset != frame.len() {
                    return Poll::Pending;
                }

                self.state = State::Idle;

                if let Err(err) = dest.flush() {
                    Poll::Failed(super::Error::Io(err))
                } else {
                    Poll::Ready(())
                }
            }
            Err(err)
                if err.kind() == io::ErrorKind::WouldBlock
                    || err.kind() == io::ErrorKind::Interrupted =>
            {
                Poll::Pending
            }
            Err(err) => {
                self.state = State::Idle;
                Poll::Failed(super::Error::Io(err))
            }
        }
    }
}
