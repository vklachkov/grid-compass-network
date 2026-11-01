use super::{frame::Frame, raw_frame::RawFrame};
use crate::{shared::FrameError, transport::Transport};

pub(super) struct FrameIo<T: Transport> {
    transport: T,
    outgoing: Vec<u8>,
}

impl<T: Transport> FrameIo<T> {
    pub(super) fn new(transport: T) -> Self {
        Self {
            transport,
            outgoing: Vec::new(),
        }
    }

    pub(super) fn receive_frame(&mut self) -> Result<Option<Frame<Vec<u8>>>, FrameError> {
        let raw = match RawFrame::read_from_io(&mut self.transport) {
            Ok(raw) => raw,
            Err(FrameError::UnexpectedEof) => return Ok(None),
            Err(err) => return Err(err),
        };
        Frame::try_from_vec(raw.data).map(Some)
    }

    pub(super) fn send_frame(&mut self, frame: Frame<&[u8]>) -> Result<(), FrameError> {
        self.outgoing.clear();
        frame.write_into(&mut self.outgoing);
        RawFrame::write_data_to_io(&self.outgoing, &mut self.transport)
    }

    #[cfg(test)]
    pub(super) fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Cursor, Read, Write};

    use super::*;
    use crate::server::frame::FrameBody;

    const DATA: &[u8] = &[
        0x10, 2, 5, 1, b'D', 0x1d, 0x10, 0x10, 2, 3, 0x10, 3, 0xe2, 0x16,
    ];
    const ACK: &[u8] = &[0x10, 2, 2, 1, b'A', 0x1d, 0x7b, 0x10, 3, 0x61, 0x5b];
    const PING: &[u8] = &[0x10, 2, 4, 1, b'P', 0x1d, 0x7b, 0x10, 3, 0xb9, 0x5e];

    #[derive(Default)]
    struct MemoryTransport {
        input: Cursor<Vec<u8>>,
        output: Vec<u8>,
    }

    impl Read for MemoryTransport {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            self.input.read(buffer)
        }
    }

    impl Write for MemoryTransport {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            self.output.extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn frame_io(input: &[u8]) -> FrameIo<MemoryTransport> {
        FrameIo::new(MemoryTransport {
            input: Cursor::new(input.to_vec()),
            ..MemoryTransport::default()
        })
    }

    #[test]
    fn received_data_is_owned_and_survives_sending_and_receiving_other_frames() {
        let mut io = frame_io(&[DATA, PING].concat());
        let first = io.receive_frame().unwrap().unwrap();
        assert_eq!(
            (first.flags, first.window_size, first.seq_number),
            (1, b'D', 0x1d)
        );
        io.send_frame(Frame::ack(0x7b, 0x1d)).unwrap();
        let second = io.receive_frame().unwrap().unwrap();
        match second.body {
            FrameBody::Ping(body) => assert_eq!(body.connection_id, 0x7b),
            body => panic!("expected PING, got {body:?}"),
        }
        match first.body {
            FrameBody::Data(data) => assert_eq!(data, [0x10, 2, 3]),
            body => panic!("expected DATA, got {body:?}"),
        }
        assert_eq!(io.transport_mut().output, ACK);
        assert!(io.receive_frame().unwrap().is_none());
    }

    #[test]
    fn send_frame_encodes_dle_crc_and_clears_the_previous_frame_buffer() {
        let mut io = frame_io(&[]);
        io.send_frame(Frame::data(1, 0x1d, &[0x10, 2, 3])).unwrap();
        io.send_frame(Frame::ack(0x7b, 0x1d)).unwrap();
        assert_eq!(io.transport_mut().output, [DATA, ACK].concat());
    }

    #[test]
    fn send_frame_does_not_wait_for_ack_or_read_the_transport() {
        let mut io = frame_io(PING);
        io.send_frame(Frame::data(1, 0x1d, &[0x10, 2, 3])).unwrap();
        assert_eq!(io.transport_mut().input.position(), 0);
        assert_eq!(io.transport_mut().output, DATA);
    }

    #[test]
    fn receive_frame_does_not_acknowledge_or_filter_frames() {
        let mut io = frame_io(&[DATA, DATA, ACK].concat());
        for _ in 0..2 {
            let frame = io.receive_frame().unwrap().unwrap();
            assert_eq!(frame.seq_number, 0x1d);
            assert!(matches!(frame.body, FrameBody::Data(data) if data == [0x10, 2, 3]));
        }
        assert!(matches!(
            io.receive_frame().unwrap().unwrap().body,
            FrameBody::Ack(_)
        ));
        assert!(io.transport_mut().output.is_empty());
    }

    #[test]
    fn crc_error_is_reported_before_parsing_a_frame() {
        let mut input = DATA.to_vec();
        *input.last_mut().unwrap() ^= 1;
        assert!(matches!(
            frame_io(&input).receive_frame(),
            Err(FrameError::InvalidCrc { .. })
        ));
    }

    #[test]
    fn complete_but_truncated_pdl_header_is_not_transport_eof() {
        let mut io = frame_io(&[0x10, 2, 5, 1, 0x10, 3, 0xc2, 0x90]);
        assert!(matches!(io.receive_frame(), Err(FrameError::UnexpectedEof)));
    }

    #[test]
    fn unknown_pdl_frame_type_is_reported_as_a_parse_error() {
        let mut io = frame_io(&[0x10, 2, 0xff, 1, b'D', 0x1d, 0x10, 3, 0x92, 0xdd]);
        assert!(matches!(
            io.receive_frame(),
            Err(FrameError::Validation { .. })
        ));
    }
}
