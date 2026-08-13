use bstr::BStr;

use crate::gridlink::vipc::MessageType;

// AdministratorSentry attaches `Admin~Manager~` with mode 10 (process connect) and then
// sends every command with `OsSend(conn, class = 0xffff, note = 0xffff, ...)`, so the
// Admin Manager traffic arrives with both VIPC class and note set to 0xffff.
pub const MESSAGE_TYPE: MessageType = MessageType(0xffff);

const COMMAND_STATUS: u8 = 0x02;
const COMMAND_VARIANT: u8 = 0x03;
const COMMAND_VARIANT_QUERY: u8 = 0x04;
const COMMAND_ADD_USER: u8 = 0x06;

const TAG_COMPANY: u8 = 0x07;
const TAG_GROUP: u8 = 0x08;
const TAG_USER: u8 = 0x09;
const TAG_PASSWORD: u8 = 0x0a;
const TAG_AUTHORITY: u8 = 0x1a;
const TAG_DISK_SPACE_TEXT: u8 = 0x1b;
const TAG_VARIANT: u8 = 0x25;
const TAG_QUOTA: u8 = 0x26;

/// Variant byte reported to the client. Anything but `1` keeps the full
/// administrator menu, `1` replaces it with a "not supported" notice.
const VARIANT: u8 = b'3';

const STATUS_OK: u16 = 0;

const QUOTA_UNLIMITED: u32 = u32::MAX;

pub struct SentryServer;

impl SentryServer {
    pub fn new() -> Self {
        Self
    }

    pub fn process(&mut self, payload: &[u8]) -> Option<Vec<u8>> {
        let [command, body @ ..] = payload else {
            return None;
        };

        let records = records(body)?;

        match *command {
            COMMAND_VARIANT_QUERY => Some(variant_response()),
            COMMAND_ADD_USER => {
                log_add_user(&records);
                // TODO: create the user for real once the account store exists.
                Some(status_response(STATUS_OK))
            }
            command => {
                info!("sentry: ignored unsupported command {command:#04x} with {records:?}");
                None
            }
        }
    }
}

/// Splits the Sentry payload into `<tag><length><value>` records.
fn records(data: &[u8]) -> Option<Vec<(u8, &[u8])>> {
    let mut records = Vec::new();

    let mut offset = 0;
    while offset < data.len() {
        let tag = data[offset];
        let length = *data.get(offset + 1)? as usize;
        let value = data.get(offset + 2..offset + 2 + length)?;
        records.push((tag, value));
        offset += 2 + length;
    }

    Some(records)
}

fn record<'a>(records: &[(u8, &'a [u8])], wanted: u8) -> Option<&'a [u8]> {
    records
        .iter()
        .find(|(tag, _)| *tag == wanted)
        .map(|(_, value)| *value)
}

fn tagged(tag: u8, value: &[u8]) -> Vec<u8> {
    let mut record = Vec::with_capacity(value.len() + 2);
    record.push(tag);
    record.push(value.len() as u8);
    record.extend_from_slice(value);
    record
}

fn variant_response() -> Vec<u8> {
    let mut payload = vec![COMMAND_VARIANT];
    payload.extend(tagged(TAG_VARIANT, &[VARIANT]));
    payload
}

/// The client reads the status word right after the command byte and reports
/// `Complete` when it is zero.
fn status_response(status: u16) -> Vec<u8> {
    let mut payload = vec![COMMAND_STATUS];
    payload.extend(status.to_le_bytes());
    payload
}

fn log_add_user(records: &[(u8, &[u8])]) {
    let text = |tag| BStr::new(record(records, tag).unwrap_or_default());
    let authority = record(records, TAG_AUTHORITY)
        .and_then(|value| <[u8; 2]>::try_from(value).ok())
        .map_or(0, u16::from_be_bytes);
    let quota = record(records, TAG_QUOTA)
        .and_then(|value| <[u8; 4]>::try_from(value).ok())
        .map_or(0, u32::from_le_bytes);

    info!(
        "sentry: add user (not applied): company={:?}, group={:?}, user={:?}, password={:?}, \
         authority={authority} ({}), quota={}, disk space={:?}",
        text(TAG_COMPANY),
        text(TAG_GROUP),
        text(TAG_USER),
        text(TAG_PASSWORD),
        authority_name(authority),
        quota_name(quota),
        text(TAG_DISK_SPACE_TEXT),
    );
}

fn authority_name(authority: u16) -> &'static str {
    match authority {
        0 => "normal user",
        20 => "group administrator",
        30 => "company administrator",
        40 => "system administrator",
        _ => "unknown authority",
    }
}

fn quota_name(quota: u32) -> String {
    match quota {
        QUOTA_UNLIMITED => "unlimited".to_owned(),
        0 => "none".to_owned(),
        bytes => format!("{bytes} bytes"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_variant_query() {
        let mut sentry = SentryServer::new();

        let response = sentry.process(&[COMMAND_VARIANT_QUERY]).unwrap();

        assert_eq!(response, [0x03, 0x25, 0x01, b'3']);
    }

    #[test]
    fn answers_add_user_with_success_status() {
        let mut sentry = SentryServer::new();
        let request = [
            0x06, // Add User
            0x09, 0x07, b'Z', b'Z', b'C', b'A', b'P', b'0', b'1', // User
            0x0a, 0x05, b'T', b'E', b'S', b'T', b'1', // Password
            0x1a, 0x02, 0x00, 0x00, // Normal user
            0x26, 0x04, 0x00, 0x04, 0x00, 0x00, // 1024 bytes
            0x1b, 0x01, b'1', // Disk space text
        ];

        let response = sentry.process(&request).unwrap();

        assert_eq!(response, [0x02, 0x00, 0x00]);
    }

    #[test]
    fn parses_add_user_fields() {
        let request = [
            0x07, 0x04, b'A', b'C', b'M', b'E', // Company
            0x09, 0x03, b'B', b'O', b'B', // User
            0x1a, 0x02, 0x00, 0x28, // System administrator
            0x26, 0x04, 0xff, 0xff, 0xff, 0xff, // Unlimited
        ];

        let records = records(&request).unwrap();

        assert_eq!(record(&records, TAG_COMPANY), Some(b"ACME".as_slice()));
        assert_eq!(record(&records, TAG_USER), Some(b"BOB".as_slice()));
        assert_eq!(record(&records, TAG_GROUP), None);
        assert_eq!(
            record(&records, TAG_AUTHORITY),
            Some([0x00, 0x28].as_slice())
        );
        assert_eq!(record(&records, TAG_QUOTA), Some([0xff; 4].as_slice()));
    }

    #[test]
    fn rejects_truncated_records() {
        assert!(records(&[0x09, 0x07, b'Z']).is_none());
        assert!(records(&[0x09]).is_none());
    }

    #[test]
    fn ignores_unsupported_commands() {
        let mut sentry = SentryServer::new();

        assert!(sentry.process(&[0x0e]).is_none());
        assert!(sentry.process(&[]).is_none());
    }
}
