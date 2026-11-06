mod error;
mod frame;
mod receive;
mod send;

use receive::Receiver;
use send::Sender;

pub use error::Error;
pub use frame::{EncodedFrame, RawFrame};
pub use receive::ReceiveFrameTask;
pub use send::SendFrameTask;

use std::io;

use crate::transport::Transport;

/// Data Link Escape. Used to prefix special commands or escape data bytes.
const DLE: u8 = 0x10;
/// Start of Text. Marks the beginning of the payload.
const STX: u8 = 0x02;
/// End of Text. Marks the end of the payload.
const ETX: u8 = 0x03;

type BufferedTransport<'t> = io::BufReader<&'t mut dyn Transport>;

pub struct FrameTransport<'t> {
    transport: BufferedTransport<'t>,
    receiver: Receiver,
    sender: Sender,
}

impl<'t> FrameTransport<'t> {
    pub fn new(transport: &'t mut dyn Transport) -> Self {
        Self {
            transport: io::BufReader::with_capacity(EncodedFrame::MAX_SIZE, transport),
            receiver: Receiver::new(),
            sender: Sender::new(),
        }
    }

    pub fn receive_frame(&mut self) -> ReceiveFrameTask<'_, 't> {
        ReceiveFrameTask::new(self)
    }

    pub fn send_frame(&mut self, frame: RawFrame) -> SendFrameTask<'_, 't> {
        SendFrameTask::new(self, frame)
    }
}
