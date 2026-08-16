use log::{debug, warn};

use super::protocol::app::{MORE, TAG_TERMINATOR, TRANSPORT_HEADER_LEN};

/// Session frames use the same `0xfe` marker as Mail's channel 0.
const SESSION_MARKER: u8 = 0xfe;

/// The one frame the client sends: `0xfe`, length 4, then `'a'` and the word 11500.
/// Byte for byte this is Mail's initialization frame with 11400 bumped to 11500,
/// which matches GRiD's habit of numbering server components in blocks of 100.
const INITIALIZE: [u8; 7] = [SESSION_MARKER, 4, 0, b'a', 0xec, 0x2c, 1];

pub struct BroadcastServer;

impl BroadcastServer {
    pub fn new() -> Self {
        Self
    }

    /// Answers the session-open handshake and nothing else. Continuation fragments and
    /// unknown frames are dropped, exactly as before, so the only behaviour that changes
    /// is that the handshake now gets a reply.
    pub fn process(&mut self, payload: &[u8]) -> Option<Vec<u8>> {
        let [flags, connection_id, 0, 0, data @ ..] = payload else {
            warn!(target: "broadcast", "ignored a malformed frame");
            return None;
        };

        if flags & MORE != 0 || data != INITIALIZE {
            warn!(target: "broadcast", "ignored an unsupported frame {data:02x?}");
            return None;
        }

        debug!(target: "broadcast", "opened session {connection_id}");

        Some(transport(
            *connection_id,
            &app_frame(SESSION_MARKER, &[TAG_TERMINATOR]),
        ))
    }
}

fn app_frame(marker: u8, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(payload.len() + 3);
    frame.push(marker);
    frame.extend((payload.len() as u16).to_le_bytes());
    frame.extend_from_slice(payload);
    frame
}

fn transport(connection_id: u8, data: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(data.len() + TRANSPORT_HEADER_LEN);
    payload.extend([0, connection_id, 0, 0]);
    payload.extend_from_slice(data);
    payload
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(flags: u8, data: &[u8]) -> Vec<u8> {
        let mut payload = vec![flags, 1, 0, 0];
        payload.extend_from_slice(data);
        payload
    }

    #[test]
    fn answers_initialization() {
        let mut server = BroadcastServer::new();

        let response = server.process(&request(0, &INITIALIZE));

        assert_eq!(response, Some(vec![0, 1, 0, 0, 0xfe, 1, 0, b'z']));
    }

    #[test]
    fn keeps_the_connection_id_of_the_request() {
        let mut server = BroadcastServer::new();
        let mut payload = vec![0, 7, 0, 0];
        payload.extend_from_slice(&INITIALIZE);

        let response = server.process(&payload).unwrap();

        assert_eq!(response[1], 7);
    }

    #[test]
    fn ignores_continuation_fragments() {
        let mut server = BroadcastServer::new();

        assert_eq!(server.process(&request(MORE, &INITIALIZE)), None);
    }

    #[test]
    fn ignores_frames_reporting_an_error() {
        let mut server = BroadcastServer::new();
        let mut payload = vec![0, 1, 5, 0];
        payload.extend_from_slice(&INITIALIZE);

        assert_eq!(server.process(&payload), None);
    }

    #[test]
    fn ignores_unknown_frames() {
        let mut server = BroadcastServer::new();

        assert_eq!(server.process(&request(0, &[0xfd, 1, 0, b'z'])), None);
        assert_eq!(server.process(&request(0, &[])), None);
        assert_eq!(server.process(&[]), None);
    }
}
