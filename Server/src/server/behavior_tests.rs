use std::io::{self, Cursor, Read, Write};

use tempfile::TempDir;

use super::raw_frame::RawFrame;
use super::*;

const INITIAL_SEQUENCE: u8 = 0x1c;
const CONNECTION_ID: u8 = 0x7b;

// Selected unstuffed requests from working-log-2241.txt, not a replay of its
// interleaved DATA/ACK ordering or the older VFS implementation's responses.
const LOG_SIGN_ON: &str = "13 00 21 01 01
    07 0c 47 52 69 44 20 53 79 73 74 65 6d 73
    08 0d 44 65 66 61 75 6c 74 20 47 72 6f 75 70
    09 07 53 79 73 75 73 65 72 0a 07 53 79 73 75 73 65 72
    0b 08 00 00 00 00 00 00 00 00";
const LOG_CONNECT_VFS: &str = "01 00 41 65 00 00 1e
    60 76 6b 6c 61 63 68 6b 6f 76 20 73 65 72 76 65 72
    3a 56 66 73 7e 4d 61 6e 61 67 65 72 7e";
const LOG_ATTACH_RESOURCES: &str = "00 00 41 65 01 00 53 00 01 00 4a 00
    08 00 45 65 00 00 01 05
    07 47 52 69 44 69 52 47 01 64 7f b6 16 f4 65 30 01
    30 60 76 6b 6c 61 63 68 6b 6f 76 20 73 65 72 76 65 72
    3a 4e 61 6d 65 20 44 65 76 69 63 65
    60 52 65 73 6f 75 72 63 65 73 7e 53 75 62 6a 65 63 74 7e";
const LOG_SET_STATUS: &str = "00 00 41 65 01 00 53 00 02 00 15 00
    14 00 45 65 01 00 0d 00 fd 01 00 01 fe 06 00 f7 7e 66 73 7e f7";
const LOG_SENTRY_QUERY: &str = "00 00 25 7e 01 00 ff ff ff ff 01 00 04";
const LOG_SENTRY_REPLY: &str = "00 00 01 00 25 7e ff ff ff ff 04 00 03 25 01 33";

fn hex(value: &str) -> Vec<u8> {
    value
        .split_ascii_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).expect("hex fixture"))
        .collect()
}

// Independent wire oracle: expected headers and payloads never use the server's
// Frame/DataFrame serializers. Fixed CRC vectors below also check this helper.
fn wire(data: &[u8]) -> Vec<u8> {
    let mut crc = 0u16;
    let mut encoded = vec![0x10, 0x02];
    for &byte in data {
        encoded.push(byte);
        if byte == 0x10 {
            encoded.push(byte);
        }
        crc ^= u16::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ if crc & 1 == 0 { 0 } else { 0xa001 };
        }
    }
    encoded.extend([0x10, 0x03]);
    encoded.extend(crc.to_le_bytes());
    encoded
}

fn data(sequence: u8, payload: &[u8]) -> Vec<u8> {
    let mut frame = vec![5, 1, b'D', sequence];
    frame.extend_from_slice(payload);
    frame
}

fn ack(sequence: u8) -> Vec<u8> {
    vec![2, 1, b'A', sequence, CONNECTION_ID]
}

fn sign_on_reply(status: u16) -> Vec<u8> {
    let mut reply = vec![6, 0];
    reply.extend(status.to_le_bytes());
    reply.push(16);
    reply.extend(b"vklachkov server");
    reply
}

#[derive(Default)]
struct Conversation {
    incoming: Vec<u8>,
    expected: Vec<u8>,
    receive_sequence: u8,
    send_sequence: u8,
}

impl Conversation {
    fn new() -> Self {
        Self {
            receive_sequence: INITIAL_SEQUENCE,
            send_sequence: INITIAL_SEQUENCE,
            ..Self::default()
        }
    }

    fn receive(&mut self, frame: &[u8]) -> usize {
        self.incoming.extend(wire(frame));
        self.incoming.len()
    }

    fn expect(&mut self, frame: &[u8]) {
        self.expected.extend(wire(frame));
    }

    fn handshake(&mut self) {
        self.receive(&hex("01 aa aa aa aa 30 32"));
        self.expect(&[1, 1, 1, INITIAL_SEQUENCE, CONNECTION_ID, b'0', b'2']);
    }

    fn request(&mut self, payload: &[u8], reply: Option<&[u8]>) -> Option<(usize, usize)> {
        self.receive_sequence = self.receive_sequence.wrapping_add(1);
        self.receive(&data(self.receive_sequence, payload));
        self.expect(&ack(self.receive_sequence));
        reply.map(|reply| {
            self.send_sequence = self.send_sequence.wrapping_add(1);
            let response_start = self.expected.len();
            self.expect(&data(self.send_sequence, reply));
            let ack_end = self.receive(&ack(self.send_sequence));
            (response_start, ack_end)
        })
    }

    fn sign_on(&mut self) {
        self.request(&hex(LOG_SIGN_ON), Some(&sign_on_reply(0)));
    }

    fn ping(&mut self) {
        self.receive(&[4, 1, b'P', self.receive_sequence, CONNECTION_ID]);
        self.expect(&ack(self.receive_sequence));
    }

    fn disconnect(&mut self) {
        self.receive(&[3, 1, b'C', self.receive_sequence, CONNECTION_ID]);
    }

    fn run(self) -> Capture {
        let harness = Harness::new(self.incoming);
        let (result, capture) = harness.run();
        result.expect("serve the conversation");
        assert_eq!(
            capture.bytes, self.expected,
            "complete outgoing wire stream"
        );
        capture
    }
}

#[derive(Clone, Default)]
struct Capture {
    bytes: Vec<u8>,
    writes: Vec<(usize, usize)>,
    input_position: usize,
}

impl Capture {
    fn input_position_at_output(&self, output_offset: usize) -> usize {
        self.writes
            .iter()
            .find(|(_, end)| *end > output_offset)
            .expect("output byte was written")
            .0
    }
}

struct TestTransport {
    input: Cursor<Vec<u8>>,
    capture: Arc<Mutex<Capture>>,
    max_read: usize,
    max_write: usize,
    read_error: Option<(usize, io::ErrorKind)>,
    write_error: Option<(usize, io::ErrorKind)>,
}

impl Read for TestTransport {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let mut length = buffer.len().min(self.max_read);
        if length == 0 {
            return Ok(0);
        }
        if let Some((at, kind)) = self.read_error {
            let position = self.input.position() as usize;
            if position >= at {
                return Err(io::Error::from(kind));
            }
            length = length.min(at - position);
        }
        let count = self.input.read(&mut buffer[..length])?;
        self.capture.lock().input_position = self.input.position() as usize;
        Ok(count)
    }
}

impl Write for TestTransport {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let mut length = buffer.len().min(self.max_write);
        if length == 0 {
            return Ok(0);
        }
        let mut capture = self.capture.lock();
        if let Some((at, kind)) = self.write_error {
            if capture.bytes.len() >= at {
                return Err(io::Error::from(kind));
            }
            length = length.min(at - capture.bytes.len());
        }
        capture.bytes.extend_from_slice(&buffer[..length]);
        let end = capture.bytes.len();
        capture.writes.push((self.input.position() as usize, end));
        Ok(length)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct Harness {
    session: Session,
    link: PdlLink<TestTransport>,
    conn: Arc<db::Database>,
    capture: Arc<Mutex<Capture>>,
    ids: Arc<Mutex<IdMap8>>,
    _root: TempDir,
}

impl Harness {
    fn new(input: Vec<u8>) -> Self {
        let root = TempDir::new().expect("test VFS root");
        let capture = Arc::new(Mutex::new(Capture::default()));
        let ids = Arc::new(Mutex::new(IdMap8::new()));
        for id in 1..CONNECTION_ID {
            ids.lock().set(NonZeroU8::new(id).unwrap());
        }
        let transport = TestTransport {
            input: Cursor::new(input),
            capture: capture.clone(),
            max_read: usize::MAX,
            max_write: usize::MAX,
            read_error: None,
            write_error: None,
        };
        let connection_id = ConnectionId::acquire(&ids).expect("connection id");
        assert_eq!(connection_id.get(), CONNECTION_ID);
        let link = PdlLink::new(transport, connection_id, INITIAL_SEQUENCE);
        let conn = db::Database::open_in_memory();
        let session = Session::new(
            conn.clone(),
            Arc::new(VfsDirManager::new(root.path()).expect("open VFS root")),
        );
        Self {
            session,
            link,
            conn,
            capture,
            ids,
            _root: root,
        }
    }

    fn run(self) -> (io::Result<()>, Capture) {
        let result = run_session("scripted transport", self.link, self.session);
        assert!(
            !self
                .ids
                .lock()
                .is_set(NonZeroU8::new(CONNECTION_ID).unwrap())
        );
        let capture = self.capture.lock().clone();
        (result, capture)
    }
}

mod link {
    use super::*;

    #[test]
    fn wire_oracle_matches_fixed_crc_and_dle_vectors() {
        assert_eq!(
            wire(b"123456789"),
            hex("10 02 31 32 33 34 35 36 37 38 39 10 03 3d bb")
        );
        assert_eq!(wire(&[0x10]), hex("10 02 10 10 10 03 01 cc"));
    }

    #[test]
    fn negotiates_rfc_and_answers_idle_pings() {
        let mut exchange = Conversation::new();
        exchange.handshake();
        exchange.ping();
        exchange.ping();
        exchange.disconnect();
        exchange.run();
    }

    #[test]
    fn disconnect_stops_before_reading_more_input() {
        let mut exchange = Conversation::new();
        exchange.handshake();
        exchange.disconnect();
        let disconnect_end = exchange.incoming.len();
        exchange.receive(&[0xff]);
        let capture = exchange.run();
        assert_eq!(capture.input_position, disconnect_end);
    }

    #[test]
    fn eof_closes_an_idle_link_and_releases_its_id() {
        let mut exchange = Conversation::new();
        exchange.handshake();
        exchange.run();
    }

    #[test]
    fn partial_reads_and_writes_preserve_the_wire_exchange() {
        let mut exchange = Conversation::new();
        exchange.handshake();
        exchange.sign_on();
        exchange.ping();
        exchange.disconnect();
        for (read, write) in [(1, 1), (2, 3), (7, 5)] {
            let mut harness = Harness::new(exchange.incoming.clone());
            harness.link.transport_mut().max_read = read;
            harness.link.transport_mut().max_write = write;
            let (result, capture) = harness.run();
            result.expect("serve with partial I/O");
            assert_eq!(
                capture.bytes, exchange.expected,
                "read={read}, write={write}"
            );
        }
    }

    #[test]
    fn invalid_crc_never_reaches_sign_on() {
        let mut exchange = Conversation::new();
        exchange.handshake();
        let mut corrupted = wire(&data(0x1d, &hex(LOG_SIGN_ON)));
        *corrupted.last_mut().unwrap() ^= 1;
        exchange.incoming.extend(corrupted);
        let harness = Harness::new(exchange.incoming);
        let (result, capture) = harness.run();
        let error = result.expect_err("CRC failure");
        assert!(matches!(
            error.get_ref().unwrap().downcast_ref::<FrameError>(),
            Some(FrameError::InvalidCrc { .. })
        ));
        assert_eq!(capture.bytes, exchange.expected);
    }

    #[test]
    fn read_failure_is_returned_and_releases_the_connection_id() {
        let mut exchange = Conversation::new();
        exchange.handshake();
        let mut harness = Harness::new(exchange.incoming.clone());
        harness.link.transport_mut().read_error =
            Some((exchange.incoming.len(), io::ErrorKind::ConnectionReset));
        let (result, capture) = harness.run();
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::ConnectionReset);
        assert_eq!(capture.bytes, exchange.expected);
    }

    #[test]
    fn write_failure_returns_error_and_preserves_written_prefix() {
        let mut exchange = Conversation::new();
        exchange.handshake();
        let failure_at = exchange.expected.len() + wire(&ack(0x1d)).len() + 3;
        exchange.sign_on();
        let mut harness = Harness::new(exchange.incoming);
        harness.link.transport_mut().write_error = Some((failure_at, io::ErrorKind::BrokenPipe));
        let (result, capture) = harness.run();
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(capture.bytes, exchange.expected[..failure_at]);
    }

    #[test]
    fn each_response_is_written_before_its_ack_is_read() {
        let mut exchange = Conversation::new();
        exchange.handshake();
        let (first_start, first_ack_end) = exchange
            .request(&hex(LOG_SIGN_ON), Some(&sign_on_reply(0)))
            .unwrap();
        let (second_start, _) = exchange
            .request(&hex(LOG_CONNECT_VFS), Some(&hex("02 00 01 00 41 65 00 00")))
            .unwrap();
        exchange.disconnect();
        let capture = exchange.run();
        assert!(capture.input_position_at_output(first_start) < first_ack_end);
        assert!(capture.input_position_at_output(second_start) >= first_ack_end);
    }

    #[test]
    fn multiple_mail_responses_are_sent_in_order_with_an_ack_between_them() {
        let mut exchange = Conversation::new();
        exchange.handshake();
        exchange.sign_on();
        exchange.request(&hex(LOG_CONNECT_VFS), Some(&hex("02 00 01 00 41 65 00 00")));
        exchange.receive_sequence = exchange.receive_sequence.wrapping_add(1);
        exchange.receive(&data(
            exchange.receive_sequence,
            &hex("00 00 41 65 01 00 44 74 07 00 11 00
                00 05 00 00 fd 0a 00 53 01 00 01 00 00 00 00 00 00"),
        ));
        exchange.expect(&ack(exchange.receive_sequence));

        let mut response_starts = Vec::new();
        let mut ack_ends = Vec::new();
        for (id, flags, subject) in [(1, 1, b"first"), (2, 0, b"other")] {
            // Two fixed-size list entries: b header, k sender, s subject, z.
            let mut reply = hex("00 00 01 00 41 65 44 74 07 80 2b 00");
            reply.extend([flags, 5, 0, 0]);
            reply.extend([0xfd, 12, 0, b'b', id, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
            reply.extend(hex("fd 08 00 6b 53 79 73 75 73 65 72 fd 06 00 73"));
            reply.extend(subject);
            reply.extend(hex("fd 01 00 7a"));
            exchange.send_sequence = exchange.send_sequence.wrapping_add(1);
            response_starts.push(exchange.expected.len());
            exchange.expect(&data(exchange.send_sequence, &reply));
            ack_ends.push(exchange.receive(&ack(exchange.send_sequence)));
        }
        exchange.disconnect();

        let harness = Harness::new(exchange.incoming);
        for subject in ["first", "other"] {
            db::mailbox::insert(
                &harness.conn.get_conn(),
                &db::mailbox::NewMessage {
                    sender_id: 1,
                    recipient_id: 1,
                    subject,
                    body: "body",
                    attachment_path: None,
                },
            )
            .expect("seed a mailbox entry");
        }
        let (result, capture) = harness.run();
        result.expect("serve a mail list with two responses");
        assert_eq!(capture.bytes, exchange.expected);
        assert!(capture.input_position_at_output(response_starts[0]) < ack_ends[0]);
        assert!(capture.input_position_at_output(response_starts[1]) >= ack_ends[0]);
        assert!(capture.input_position_at_output(response_starts[1]) < ack_ends[1]);
    }
}

mod session {
    use super::*;

    #[test]
    fn accepts_the_logged_sign_on_and_connect_exchange() {
        let mut exchange = Conversation::new();
        exchange.handshake();
        exchange.sign_on();
        exchange.request(&hex(LOG_CONNECT_VFS), Some(&hex("02 00 01 00 41 65 00 00")));
        exchange.disconnect();
        exchange.run();
    }

    #[test]
    fn routes_logged_vfs_messages_and_preserves_note_and_path_ids() {
        let mut exchange = Conversation::new();
        exchange.handshake();
        exchange.sign_on();
        exchange.request(&hex(LOG_CONNECT_VFS), Some(&hex("02 00 01 00 41 65 00 00")));
        exchange.request(
            &hex(LOG_ATTACH_RESOURCES),
            Some(&hex(
                "00 00 01 00 41 65 53 00 01 00 08 00 08 80 01 00 45 65 00 00",
            )),
        );
        exchange.request(
            &hex(LOG_SET_STATUS),
            Some(&hex(
                "00 00 01 00 41 65 53 00 02 00 08 00 14 80 01 00 45 65 00 00",
            )),
        );
        exchange.request(
            &hex("00 00 41 65 01 00 53 00 03 00 08 00 1d 00 45 65 01 00 01 00"),
            Some(&hex("00 00 01 00 41 65 53 00 03 00 24 00
                1d 80 01 00 45 65 00 00 1a 00
                00 00 00 00 1a 00 00 00 11
                55 73 65 72 20 53 75 62 6a 65 63 74 73 7e 46 53 7e")),
        );
        exchange.request(
            &hex("00 00 41 65 01 00 53 00 04 00 08 00 1d 00 45 65 01 00 01 00"),
            Some(&hex("00 00 01 00 41 65 53 00 04 00 25 00
                1d 80 01 00 45 65 00 00 1b 00
                00 00 00 00 1b 00 00 00 12
                47 72 6f 75 70 20 53 75 62 6a 65 63 74 73 7e 46 53 7e")),
        );
        exchange.request(
            &hex("00 00 41 65 01 00 53 00 05 00 06 00 09 00 45 65 01 00"),
            Some(&hex(
                "00 00 01 00 41 65 53 00 05 00 08 00 09 80 01 00 45 65 00 00",
            )),
        );
        exchange.request(
            &hex("00 00 41 65 01 00 53 00 06 00 08 00 1d 00 45 65 01 00 01 00"),
            Some(&hex(
                "00 00 01 00 41 65 53 00 06 00 0a 00 1d 80 01 00 45 65 dd 00 00 00",
            )),
        );
        exchange.disconnect();
        exchange.run();
    }

    #[test]
    fn routes_the_logged_sentry_query_and_resource_disconnect() {
        let mut exchange = Conversation::new();
        exchange.handshake();
        exchange.sign_on();
        exchange.request(&hex("01 00 25 7e 00 00 20 60 76 6b 6c 61 63 68 6b 6f 76 20 73 65 72 76 65 72 3a 41 64 6d 69 6e 7e 4d 61 6e 61 67 65 72 7e"), Some(&hex("02 00 01 00 25 7e 00 00")));
        exchange.request(&hex(LOG_SENTRY_QUERY), Some(&hex(LOG_SENTRY_REPLY)));
        exchange.request(
            &hex("03 00 25 7e 01 00 90 61"),
            Some(&hex("04 00 01 00 25 7e")),
        );
        exchange.disconnect();
        exchange.run();
    }

    #[test]
    fn refuses_connect_before_sign_on_but_keeps_the_link_alive() {
        let mut exchange = Conversation::new();
        exchange.handshake();
        exchange.request(&hex(LOG_CONNECT_VFS), Some(&hex("02 00 01 00 41 65 21 03")));
        exchange.ping();
        exchange.sign_on();
        exchange.request(&hex(LOG_CONNECT_VFS), Some(&hex("02 00 01 00 41 65 00 00")));
        exchange.disconnect();
        exchange.run();
    }

    #[test]
    fn failed_sign_on_does_not_open_access_to_resources() {
        let mut sign_on = hex(LOG_SIGN_ON);
        let password = sign_on
            .windows(2)
            .position(|bytes| bytes == [0x0a, 7])
            .unwrap()
            + 2;
        sign_on[password..password + 7].copy_from_slice(b"invalid");
        let mut exchange = Conversation::new();
        exchange.handshake();
        exchange.request(&sign_on, Some(&sign_on_reply(1003)));
        exchange.request(&hex(LOG_CONNECT_VFS), Some(&hex("02 00 01 00 41 65 21 03")));
        exchange.sign_on();
        exchange.request(&hex(LOG_CONNECT_VFS), Some(&hex("02 00 01 00 41 65 00 00")));
        exchange.request(&hex(LOG_SENTRY_QUERY), Some(&hex(LOG_SENTRY_REPLY)));
        exchange.disconnect();
        exchange.run();
    }

    #[test]
    fn truncated_session_request_is_acknowledged_then_returns_a_parse_error() {
        for payload in [&[19, 0, 7][..], &[0, 0, 0x41][..]] {
            let mut exchange = Conversation::new();
            exchange.handshake();
            exchange.request(payload, None);
            let request_end = exchange.incoming.len();
            exchange.sign_on();

            let (result, capture) = Harness::new(exchange.incoming).run();
            let error = result.expect_err("truncated session request");
            assert!(matches!(
                error.get_ref().unwrap().downcast_ref::<FrameError>(),
                Some(FrameError::UnexpectedEof)
            ));
            let mut expected = wire(&[1, 1, 1, INITIAL_SEQUENCE, CONNECTION_ID, b'0', b'2']);
            expected.extend(wire(&ack(INITIAL_SEQUENCE + 1)));
            assert_eq!(capture.bytes, expected);
            assert_eq!(capture.input_position, request_end);
        }
    }

    #[test]
    fn sign_off_clears_authentication_without_closing_the_link() {
        let mut exchange = Conversation::new();
        exchange.handshake();
        exchange.sign_on();
        exchange.request(&[7, 0], None);
        exchange.request(&hex(LOG_CONNECT_VFS), Some(&hex("02 00 01 00 41 65 21 03")));
        exchange.ping();
        exchange.sign_on();
        exchange.request(&hex(LOG_SENTRY_QUERY), Some(&hex(LOG_SENTRY_REPLY)));
        exchange.disconnect();
        exchange.run();
    }

    #[test]
    fn message_before_sign_on_is_acknowledged_but_not_dispatched() {
        let mut exchange = Conversation::new();
        exchange.handshake();
        exchange.request(&hex(LOG_SENTRY_QUERY), None);
        exchange.sign_on();
        exchange.request(&hex(LOG_SENTRY_QUERY), Some(&hex(LOG_SENTRY_REPLY)));
        exchange.disconnect();
        exchange.run();
    }

    #[test]
    fn duplicate_data_does_not_allocate_a_second_vfs_attachment() {
        let mut exchange = Conversation::new();
        exchange.handshake();
        exchange.sign_on();
        exchange.request(&hex(LOG_CONNECT_VFS), Some(&hex("02 00 01 00 41 65 00 00")));
        let attach = hex(LOG_ATTACH_RESOURCES);
        exchange.request(
            &attach,
            Some(&hex(
                "00 00 01 00 41 65 53 00 01 00 08 00 08 80 01 00 45 65 00 00",
            )),
        );
        exchange.receive(&data(exchange.receive_sequence, &attach));
        exchange.request(
            &attach,
            Some(&hex(
                "00 00 01 00 41 65 53 00 01 00 08 00 08 80 02 00 45 65 00 00",
            )),
        );
        exchange.disconnect();

        let (result, capture) = Harness::new(exchange.incoming).run();
        result.expect("serve a duplicate DATA");
        // Do not freeze the missing duplicate ACK as correct behavior: compare
        // application responses only, so a future duplicate ACK remains valid.
        let application_frames = |bytes: &[u8]| {
            let mut input = Cursor::new(bytes);
            let mut frames = Vec::new();
            while input.position() < bytes.len() as u64 {
                let raw = RawFrame::read_from_io(&mut input).expect("outgoing frame");
                if raw.data[0] == 5 {
                    frames.push(raw.data);
                }
            }
            frames
        };
        assert_eq!(
            application_frames(&capture.bytes),
            application_frames(&exchange.expected)
        );
    }
}
