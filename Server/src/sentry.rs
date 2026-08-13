use bstr::BStr;

use crate::gridlink::vipc::MessageType;

// AdministratorSentry attaches `Admin~Manager~` with mode 10 (process connect) and then
// sends every command with `OsSend(conn, class = 0xffff, note = 0xffff, ...)`, so the
// Admin Manager traffic arrives with both VIPC class and note set to 0xffff.
pub const MESSAGE_TYPE: MessageType = MessageType(0xffff);

const COMMAND_STATUS: u8 = 0x02;
/// Generic reply container: carries the variant record for `0x04` and one
/// directory row for `0x1e`/`0x1f`. The client checks for it in
/// `Sentry_ValidateResponseCommand` before looking at the records.
const COMMAND_REPLY: u8 = 0x03;
const COMMAND_VARIANT_QUERY: u8 = 0x04;
const COMMAND_ADD_USER: u8 = 0x06;
const COMMAND_LIST_FIRST: u8 = 0x1e;
const COMMAND_LIST_NEXT: u8 = 0x1f;

const TAG_COMPANY: u8 = 0x07;
const TAG_GROUP: u8 = 0x08;
const TAG_USER: u8 = 0x09;
const TAG_PASSWORD: u8 = 0x0a;
const TAG_AUTHORITY: u8 = 0x1a;
const TAG_DISK_SPACE_TEXT: u8 = 0x1b;
const TAG_VARIANT: u8 = 0x25;
const TAG_QUOTA: u8 = 0x26;
const TAG_CURSOR: u8 = 0x27;
const TAG_DEVICE: u8 = 0x28;
const TAG_DISK_USED: u8 = 0x29;
const TAG_CREATED: u8 = 0x2a;
const TAG_MODIFIED: u8 = 0x2b;
const TAG_LOCKED: u8 = 0x2c;

/// Variant byte reported to the client. Anything but `1` keeps the full
/// administrator menu, `1` replaces it with a "not supported" notice.
const VARIANT: u8 = b'3';

const STATUS_OK: u16 = 0;
/// The listing loop clears this status silently instead of showing an error
/// dialog, so it is how an enumeration ends.
const STATUS_END_OF_LISTING: u16 = 1005;

const AUTHORITY_NORMAL: u16 = 0;
const AUTHORITY_GROUP_ADMIN: u16 = 20;
const AUTHORITY_COMPANY_ADMIN: u16 = 30;
const AUTHORITY_SYSTEM_ADMIN: u16 = 40;

const QUOTA_UNLIMITED: u32 = u32::MAX;

/// Filter fields are 20 byte wide: all zeros mean "before the first record",
/// all `0xff` mean "past everything below this level".
const FILTER_SKIP: u8 = 0xff;

const DEVICE: &[u8] = b"Hard Disk";
const CREATED: &[u8] = b"82/03/28 09:00:00";
const MODIFIED: &[u8] = b"86/03/01 09:15:00";

const UNLOCKED: [u8; 2] = [0, 0];

const MEGABYTE: u32 = 1024 * 1024;

/// One row of the demo directory, in the order the client walks it: a company,
/// then each of its groups followed by that group's users.
const DEMO_DIRECTORY: &[Row] = &[
    Row::company(b"GRiD"),
    Row::group(b"GRiD", b"Demo"),
    Row::user(
        b"GRiD",
        b"Demo",
        b"GUEST",
        AUTHORITY_NORMAL,
        MEGABYTE,
        262144,
    ),
    Row::user(
        b"GRiD",
        b"Demo",
        b"OPERATOR",
        AUTHORITY_GROUP_ADMIN,
        QUOTA_UNLIMITED,
        MEGABYTE,
    ),
    Row::group(b"GRiD", b"Systems"),
    Row::user(
        b"GRiD",
        b"Systems",
        b"MANAGER",
        AUTHORITY_SYSTEM_ADMIN,
        QUOTA_UNLIMITED,
        4 * MEGABYTE,
    ),
];

/// The client tells the three record levels apart by comparing the names:
/// `company == group` marks a company, `group == user` a group, anything else
/// a user.
struct Row {
    company: &'static [u8],
    group: &'static [u8],
    user: &'static [u8],
    authority: u16,
    quota: u32,
    used: u32,
}

impl Row {
    const fn company(company: &'static [u8]) -> Self {
        Self {
            company,
            group: company,
            user: company,
            authority: AUTHORITY_COMPANY_ADMIN,
            quota: QUOTA_UNLIMITED,
            used: 0,
        }
    }

    const fn group(company: &'static [u8], group: &'static [u8]) -> Self {
        Self {
            company,
            group,
            user: group,
            authority: AUTHORITY_GROUP_ADMIN,
            quota: QUOTA_UNLIMITED,
            used: 0,
        }
    }

    const fn user(
        company: &'static [u8],
        group: &'static [u8],
        user: &'static [u8],
        authority: u16,
        quota: u32,
        used: u32,
    ) -> Self {
        Self {
            company,
            group,
            user,
            authority,
            quota,
            used,
        }
    }

    /// Whether the row sits under the given company / group / user prefix.
    fn matches(&self, prefix: &[&[u8]]) -> bool {
        [self.company, self.group, self.user]
            .iter()
            .zip(prefix)
            .all(|(name, wanted)| name.eq_ignore_ascii_case(wanted))
    }
}

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
            COMMAND_LIST_FIRST => Some(listing_response(seek(&records))),
            COMMAND_LIST_NEXT => Some(listing_response(resume(&records))),
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
    let mut payload = vec![COMMAND_REPLY];
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

/// One name out of the `0x1e` filter triple.
enum Filter<'a> {
    /// Zero filled: the listing has not started yet.
    Start,
    /// `0xff` filled: everything below this level is already listed.
    Skip,
    Name(&'a [u8]),
}

fn filter<'a>(records: &[(u8, &'a [u8])], tag: u8) -> Filter<'a> {
    let Some(value) = record(records, tag) else {
        return Filter::Start;
    };

    if value.iter().all(|byte| *byte == FILTER_SKIP) && !value.is_empty() {
        return Filter::Skip;
    }

    let name = value
        .iter()
        .rposition(|byte| *byte != 0)
        .map_or(&value[..0], |end| &value[..=end]);

    if name.is_empty() {
        Filter::Start
    } else {
        Filter::Name(name)
    }
}

/// The client repeats the previous row as the filter of the next `0x1e`, so the
/// three names address a record instead of starting a search: the answer is the
/// row right after the last one matching them.
fn seek(records: &[(u8, &[u8])]) -> Option<usize> {
    let prefix: &[&[u8]] = &match (
        filter(records, TAG_COMPANY),
        filter(records, TAG_GROUP),
        filter(records, TAG_USER),
    ) {
        (Filter::Start, _, _) => return Some(0),
        (Filter::Skip, _, _) => return None,
        (Filter::Name(company), Filter::Start | Filter::Skip, _) => vec![company],
        (Filter::Name(company), Filter::Name(group), Filter::Start | Filter::Skip) => {
            vec![company, group]
        }
        (Filter::Name(company), Filter::Name(group), Filter::Name(user)) => {
            vec![company, group, user]
        }
    };

    let last = DEMO_DIRECTORY.iter().rposition(|row| row.matches(prefix))?;

    Some(last + 1)
}

/// `0x1f` continues a user listing by echoing the cursor of the previous row.
fn resume(records: &[(u8, &[u8])]) -> Option<usize> {
    let cursor = record(records, TAG_CURSOR)?;
    let index: usize = std::str::from_utf8(cursor).ok()?.parse().ok()?;

    Some(index + 1)
}

/// Answers one step of a listing. Running past the last row ends the
/// enumeration with the status the client clears without complaining.
fn listing_response(index: Option<usize>) -> Vec<u8> {
    let Some((index, row)) = index.and_then(|index| Some((index, DEMO_DIRECTORY.get(index)?)))
    else {
        info!("sentry: end of listing");
        return status_response(STATUS_END_OF_LISTING);
    };

    info!(
        "sentry: listing row {index}: company={:?}, group={:?}, user={:?}, authority={} ({})",
        BStr::new(row.company),
        BStr::new(row.group),
        BStr::new(row.user),
        row.authority,
        authority_name(row.authority),
    );

    let mut payload = vec![COMMAND_REPLY];
    payload.extend(tagged(TAG_CURSOR, index.to_string().as_bytes()));
    payload.extend(tagged(TAG_COMPANY, row.company));
    payload.extend(tagged(TAG_GROUP, row.group));
    payload.extend(tagged(TAG_USER, row.user));
    payload.extend(tagged(TAG_AUTHORITY, &row.authority.to_le_bytes()));
    payload.extend(tagged(TAG_DEVICE, DEVICE));
    payload.extend(tagged(TAG_QUOTA, &row.quota.to_le_bytes()));
    payload.extend(tagged(TAG_DISK_USED, &row.used.to_le_bytes()));
    payload.extend(tagged(TAG_CREATED, CREATED));
    payload.extend(tagged(TAG_MODIFIED, MODIFIED));
    payload.extend(tagged(TAG_LOCKED, &UNLOCKED));
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

    /// Builds the `<tag><20 bytes>` filter the client sends: names are padded
    /// with zeros, an absent level is all zeros, a finished one all `0xff`.
    fn list_filter(tag: u8, name: &[u8]) -> Vec<u8> {
        let mut value = name.to_vec();
        value.resize(20, 0);
        tagged(tag, &value)
    }

    fn list_first(company: &[u8], group: &[u8], user: &[u8]) -> Vec<u8> {
        let mut request = vec![COMMAND_LIST_FIRST];
        request.extend(list_filter(TAG_COMPANY, company));
        request.extend(list_filter(TAG_GROUP, group));
        request.extend(list_filter(TAG_USER, user));
        request
    }

    #[test]
    fn answers_first_listing_request_with_the_first_row() {
        let mut sentry = SentryServer::new();

        let response = sentry.process(&list_first(b"", b"", b"")).unwrap();

        assert_eq!(
            response,
            [
                0x03, // Reply
                0x27, 0x01, b'0', // Cursor
                0x07, 0x04, b'G', b'R', b'i', b'D', // Company
                0x08, 0x04, b'G', b'R', b'i', b'D', // Group
                0x09, 0x04, b'G', b'R', b'i', b'D', // User
                0x1a, 0x02, 0x1e, 0x00, // Company administrator
                0x28, 0x09, b'H', b'a', b'r', b'd', b' ', b'D', b'i', b's', b'k', // Device
                0x26, 0x04, 0xff, 0xff, 0xff, 0xff, // MaxDisk
                0x29, 0x04, 0x00, 0x00, 0x00, 0x00, // DiskUsed
                0x2a, 0x11, b'8', b'2', b'/', b'0', b'3', b'/', b'2', b'8', b' ', b'0', b'9', b':',
                b'0', b'0', b':', b'0', b'0', // Created
                0x2b, 0x11, b'8', b'6', b'/', b'0', b'3', b'/', b'0', b'1', b' ', b'0', b'9', b':',
                b'1', b'5', b':', b'0', b'0', // Modified
                0x2c, 0x02, 0x00, 0x00, // Unlocked
            ]
        );
    }

    #[test]
    fn walks_the_directory_row_by_row() {
        let mut sentry = SentryServer::new();
        let mut rows = Vec::new();

        let mut request = list_first(b"", b"", b"");
        while let Some(response) = sentry.process(&request) {
            if response[0] == COMMAND_STATUS {
                assert_eq!(response, status_response(STATUS_END_OF_LISTING));
                break;
            }

            let records = records(&response[1..]).unwrap();
            let text = |tag| record(&records, tag).unwrap().to_vec();
            rows.push((text(TAG_COMPANY), text(TAG_GROUP), text(TAG_USER)));

            // The client echoes the row it just parsed as the next filter.
            request = list_first(&text(TAG_COMPANY), &text(TAG_GROUP), &text(TAG_USER));
        }

        assert_eq!(
            rows,
            [
                (b"GRiD".to_vec(), b"GRiD".to_vec(), b"GRiD".to_vec()),
                (b"GRiD".to_vec(), b"Demo".to_vec(), b"Demo".to_vec()),
                (b"GRiD".to_vec(), b"Demo".to_vec(), b"GUEST".to_vec()),
                (b"GRiD".to_vec(), b"Demo".to_vec(), b"OPERATOR".to_vec()),
                (b"GRiD".to_vec(), b"Systems".to_vec(), b"Systems".to_vec()),
                (b"GRiD".to_vec(), b"Systems".to_vec(), b"MANAGER".to_vec()),
            ]
        );
    }

    #[test]
    fn skips_the_rest_of_a_subtree() {
        let mut sentry = SentryServer::new();
        let mut request = vec![COMMAND_LIST_FIRST];
        request.extend(list_filter(TAG_COMPANY, b"GRiD"));
        request.extend(list_filter(TAG_GROUP, b"Demo"));
        request.extend(tagged(TAG_USER, &[FILTER_SKIP; 20]));

        let response = sentry.process(&request).unwrap();

        let records = records(&response[1..]).unwrap();
        assert_eq!(record(&records, TAG_GROUP), Some(b"Systems".as_slice()));
        assert_eq!(record(&records, TAG_USER), Some(b"Systems".as_slice()));
    }

    #[test]
    fn continues_a_user_listing_from_the_cursor() {
        let mut sentry = SentryServer::new();
        let mut request = vec![COMMAND_LIST_NEXT];
        request.extend(tagged(TAG_CURSOR, b"2"));

        let response = sentry.process(&request).unwrap();

        let records = records(&response[1..]).unwrap();
        assert_eq!(record(&records, TAG_CURSOR), Some(b"3".as_slice()));
        assert_eq!(record(&records, TAG_USER), Some(b"OPERATOR".as_slice()));
    }

    #[test]
    fn ends_the_listing_after_the_last_row() {
        let mut sentry = SentryServer::new();
        let last = list_first(b"GRiD", b"Systems", b"MANAGER");

        let response = sentry.process(&last).unwrap();

        assert_eq!(response, [0x02, 0xed, 0x03]);
    }

    #[test]
    fn ends_the_listing_when_the_whole_company_is_skipped() {
        let mut sentry = SentryServer::new();
        let mut request = vec![COMMAND_LIST_FIRST];
        request.extend(tagged(TAG_COMPANY, &[FILTER_SKIP; 20]));
        request.extend(tagged(TAG_GROUP, &[FILTER_SKIP; 20]));
        request.extend(tagged(TAG_USER, &[FILTER_SKIP; 20]));

        let response = sentry.process(&request).unwrap();

        assert_eq!(response, status_response(STATUS_END_OF_LISTING));
    }

    #[test]
    fn ends_the_listing_for_an_unknown_filter() {
        let mut sentry = SentryServer::new();

        let response = sentry.process(&list_first(b"MISSING", b"", b"")).unwrap();

        assert_eq!(response, status_response(STATUS_END_OF_LISTING));
    }

    /// Every mandatory record must fit the destination the client allocates,
    /// otherwise `sub_3332` silently truncates the value.
    #[test]
    fn listing_records_fit_the_client_buffers() {
        let mut sentry = SentryServer::new();

        let response = sentry.process(&list_first(b"", b"", b"")).unwrap();

        let records = records(&response[1..]).unwrap();
        let capacities = [
            (TAG_CURSOR, 100),
            (TAG_COMPANY, 20),
            (TAG_GROUP, 20),
            (TAG_USER, 20),
            (TAG_AUTHORITY, 2),
            (TAG_DEVICE, 40),
            (TAG_QUOTA, 4),
            (TAG_DISK_USED, 4),
            (TAG_CREATED, 18),
            (TAG_MODIFIED, 18),
            (TAG_LOCKED, 2),
        ];

        for (tag, capacity) in capacities {
            let value = record(&records, tag).unwrap_or_else(|| panic!("missing tag {tag:#04x}"));
            assert!(
                value.len() <= capacity,
                "tag {tag:#04x} overflows the client"
            );
        }
    }
}
