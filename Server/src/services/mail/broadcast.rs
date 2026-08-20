use log::{debug, warn};

use super::protocol::{
    BROADCAST_SERVICE_ID, MORE, PROTOCOL_VERSION, SESSION_MARKER, SessionInitialize,
    TAG_TERMINATOR, app_frame, transport,
};

/// Broadcast remains a handshake stub: without this reply the original client
/// waits about 47 seconds. Other broadcast behavior has not been implemented yet.
pub struct MailBroadcastServer;

impl MailBroadcastServer {
    pub fn new() -> Self {
        Self
    }

    pub fn process(&mut self, payload: &[u8]) -> Option<Vec<u8>> {
        let [flags, connection_id, 0, 0, data @ ..] = payload else {
            warn!(target: "broadcast", "ignored a malformed frame");
            return None;
        };

        let is_initialize = SessionInitialize::parse(data)
            != Some(SessionInitialize {
                service_id: BROADCAST_SERVICE_ID,
                protocol_version: PROTOCOL_VERSION,
            });

        if flags & MORE != 0 || is_initialize {
            warn!(target: "broadcast", "ignored an unsupported frame {data:02x?}");
            return None;
        }

        debug!(target: "broadcast", "opened session {connection_id}");

        Some(transport(
            0,
            *connection_id,
            &app_frame(SESSION_MARKER, &[TAG_TERMINATOR]),
        ))
    }
}
