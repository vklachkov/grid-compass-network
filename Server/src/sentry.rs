use std::{io::Write, rc::Rc};

use bstr::BStr;
use rusqlite::Connection;

use crate::{
    db,
    gridlink::{
        FrameError, Tlv, TlvEntry,
        utils::{WriteExt, u8_len},
    },
    protocol::{property, status},
};

const COMMAND_STATUS: u8 = 0x02;
/// Generic reply container: carries the variant record for `0x04` and one
/// directory row for `0x1e`/`0x1f`. The client checks for it in
/// `Sentry_ValidateResponseCommand` before looking at the records.
const COMMAND_REPLY: u8 = 0x03;
const COMMAND_VARIANT_QUERY: u8 = 0x04;
const COMMAND_ADD_USER: u8 = 0x06;
/// Both are built by `sub_2749`, which takes the command byte as an argument
/// and sends company, group, disk space text and quota without a user name.
const COMMAND_ADD_GROUP: u8 = 0x09;
const COMMAND_ADD_COMPANY: u8 = 0x0a;
const COMMAND_LIST_FIRST: u8 = 0x1e;
const COMMAND_LIST_NEXT: u8 = 0x1f;

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

/// The listing loop clears `1005: eUnknownUser` silently instead of showing an
/// error dialog, so it is how an enumeration ends.
const STATUS_END_OF_LISTING: u16 = 1005;
const STATUS_INVALID_AUTHORITY: u16 = 1006; // eInvalidAuthority
const STATUS_COMPANY_NOT_DEFINED: u16 = 1015; // eCompanyNotDefined
/// The company exists but the group does not.
const STATUS_ACCOUNT_NOT_DEFINED: u16 = 1016; // eAccountNotDefined
const STATUS_ALREADY_DEFINED: u16 = 1017; // eAlreadyDefined
/// What an account below the level a command needs gets back.
const STATUS_INSUFFICIENT_AUTHORITY: u16 = 1030; // eInsufficientAccessAuthority
const STATUS_INVALID_NAME: u16 = 1034; // eInvalidName

/// Variant byte reported to the client. Anything but `1` keeps the full
/// administrator menu, `1` replaces it with a "not supported" notice.
const VARIANT: u8 = b'3';
const VARIANT_UNSUPPORTED: u8 = b'1';

/// The authority level of an account.
///
/// This is *not* a plain `u16`: the client sends the field big endian in an
/// add-user request but expects it back little endian in a listing row. The
/// asymmetry is real client behaviour, not a bug, so it is spelled out once
/// here — `read` and `write` are deliberately different — instead of sitting as
/// a lone `from_be_bytes` that invites a well meaning "fix".
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Authority(u16);

impl Authority {
    pub const NORMAL: Self = Self(0);
    pub const GROUP_ADMIN: Self = Self(20);
    pub const COMPANY_ADMIN: Self = Self(30);
    pub const SYSTEM_ADMIN: Self = Self(40);

    const LEVELS: [Self; 4] = [
        Self::NORMAL,
        Self::GROUP_ADMIN,
        Self::COMPANY_ADMIN,
        Self::SYSTEM_ADMIN,
    ];

    pub fn from_stored(value: u16) -> Self {
        Self(value)
    }

    /// Anything outside the four defined levels would compare against the
    /// thresholds in ways the client never intends — `0xffff` outranks a system
    /// administrator — so an unknown value is refused rather than stored.
    fn is_defined(self) -> bool {
        Self::LEVELS.contains(&self)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::NORMAL => "normal user",
            Self::GROUP_ADMIN => "group administrator",
            Self::COMPANY_ADMIN => "company administrator",
            Self::SYSTEM_ADMIN => "system administrator",
            _ => "unknown authority",
        }
    }

    /// Parses the field as the client writes it in a request: big endian.
    fn read(value: &[u8]) -> Option<Self> {
        <[u8; 2]>::try_from(value)
            .ok()
            .map(|bytes| Self(u16::from_be_bytes(bytes)))
    }

    /// Serializes the field as the client reads it in a listing row: little
    /// endian, unlike the request encoding above.
    fn write(self) -> [u8; 2] {
        self.0.to_le_bytes()
    }
}

impl std::fmt::Display for Authority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

const QUOTA_UNLIMITED: u32 = u32::MAX;

/// Filter fields are 20 byte wide: all zeros mean "before the first record",
/// all `0xff` mean "past everything below this level".
const FILTER_SKIP: u8 = 0xff;

const DEVICE: &[u8] = b"Hard Disk";
const CREATED: &[u8] = b"82/03/28 09:00:00";
const MODIFIED: &[u8] = b"86/03/01 09:15:00";

const UNLOCKED: [u8; 2] = [0, 0];

/// The client tells the three record levels apart by comparing the names:
/// `company == group` marks a company, `group == user` a group, anything else
/// a user.
struct Row {
    /// The three names of a row are frequently the same string — a company row
    /// repeats its name at all three levels — so they share one allocation
    /// instead of being copied per level.
    company: Rc<[u8]>,
    group: Rc<[u8]>,
    user: Rc<[u8]>,
    authority: Authority,
    quota: u32,
    used: u32,
}

impl Row {
    /// Expands a stored row back into the wire form, where the levels that do
    /// not apply repeat the name above them: that repetition is how the client
    /// tells a company from a group from a user.
    fn from_account(account: db::Account) -> Self {
        let company: Rc<[u8]> = Rc::from(account.company.into_bytes());
        let group: Rc<[u8]> = if account.level == db::LEVEL_COMPANY {
            Rc::clone(&company)
        } else {
            Rc::from(account.group.into_bytes())
        };
        let user: Rc<[u8]> = if account.level == db::LEVEL_USER {
            Rc::from(account.user.into_bytes())
        } else {
            Rc::clone(&group)
        };

        Self {
            company,
            group,
            user,
            authority: Authority::from_stored(account.authority),
            quota: account.quota,
            used: account.used,
        }
    }

    fn matches(&self, prefix: &[&[u8]]) -> bool {
        self.names()
            .iter()
            .zip(prefix)
            .all(|(name, wanted)| name.eq_ignore_ascii_case(wanted))
    }

    fn names(&self) -> [&[u8]; 3] {
        [&self.company, &self.group, &self.user]
    }

    /// A company row lists itself at all three levels, a group row at the last
    /// two: that is how the client tells the levels apart when rendering.
    #[cfg(test)]
    fn is_company(&self) -> bool {
        self.company == self.group && self.group == self.user
    }

    #[cfg(test)]
    fn is_group(&self) -> bool {
        self.company != self.group && self.group == self.user
    }
}

pub struct SentryServer {
    conn: Rc<Connection>,
    /// The account this session signed on as, carried whole rather than looked
    /// up per command. It cannot change while the session lives: a sign-off
    /// drops the whole server.
    actor: db::Account,
}

/// Loading the whole directory per request keeps the listing cursor a plain
/// index into a `Vec`, which is exactly what the client echoes back in `0x1f`.
struct Directory {
    rows: Vec<Row>,
}

impl Directory {
    fn load(conn: &Connection) -> rusqlite::Result<Self> {
        let rows = db::load(conn)?.into_iter().map(Row::from_account).collect();

        Ok(Self { rows })
    }

    /// The index of the last row under the given prefix, which is both the
    /// existence check and, in the stored order, the parent of a new child.
    fn find(&self, prefix: &[&[u8]]) -> Option<usize> {
        self.rows.iter().rposition(|row| row.matches(prefix))
    }

    fn get(&self, index: usize) -> Option<&Row> {
        self.rows.get(index)
    }
}

impl SentryServer {
    pub fn new(conn: Rc<Connection>, actor: db::Account) -> Self {
        Self { conn, actor }
    }

    fn authority(&self) -> Authority {
        Authority::from_stored(self.actor.authority)
    }

    pub fn process(&mut self, payload: &[u8]) -> Option<Vec<u8>> {
        match self.try_process(payload) {
            Ok(response) => response,
            Err(err) => {
                info!("sentry: failed to build a response: {err}");
                None
            }
        }
    }

    fn try_process(&mut self, payload: &[u8]) -> Result<Option<Vec<u8>>, FrameError> {
        let [command, body @ ..] = payload else {
            return Ok(None);
        };

        let Some(records) = records(body) else {
            return Ok(None);
        };

        let response = match *command {
            COMMAND_VARIANT_QUERY => variant_response(self.authority())?,
            COMMAND_ADD_USER => self.add_user(&records),
            COMMAND_ADD_GROUP => self.add_group(&records),
            COMMAND_ADD_COMPANY => self.add_company(&records),
            COMMAND_LIST_FIRST => match self.directory() {
                Ok(directory) => {
                    let index = seek(&directory, &records);
                    listing_response(&directory, index)?
                }
                Err(response) => response,
            },
            COMMAND_LIST_NEXT => match self.directory() {
                Ok(directory) => listing_response(&directory, resume(&records))?,
                Err(response) => response,
            },
            command => {
                info!("sentry: ignored unsupported command {command:#04x} with {records:?}");
                return Ok(None);
            }
        };

        Ok(Some(response))
    }

    /// `sub_172C` sends the company name, an optional disk space text and the
    /// quota, and reports `Complete` when the status word comes back zero.
    fn add_company(&mut self, records: &[TlvEntry]) -> Vec<u8> {
        let company = record(records, property::COMPANY).unwrap_or_default();
        let quota = quota(records);

        info!(
            "sentry: add company {:?}, quota={}, disk space={:?}",
            BStr::new(company),
            quota_name(quota),
            BStr::new(record(records, TAG_DISK_SPACE_TEXT).unwrap_or_default()),
        );

        if let Some(denied) = self.deny_below(Authority::SYSTEM_ADMIN) {
            return denied;
        }

        let names = match ascii_names(&[company]) {
            Ok(names) => names,
            Err(response) => return response,
        };
        let [company] = names[..] else { unreachable!() };

        self.store(|conn| db::insert_company(conn, company, quota))
    }

    /// `sub_121C` sends the same records as `add_company` plus the group name.
    fn add_group(&mut self, records: &[TlvEntry]) -> Vec<u8> {
        let company = record(records, property::COMPANY).unwrap_or_default();
        let group = record(records, property::GROUP).unwrap_or_default();
        let quota = quota(records);

        info!(
            "sentry: add group {:?} to company {:?}, quota={}, disk space={:?}",
            BStr::new(group),
            BStr::new(company),
            quota_name(quota),
            BStr::new(record(records, TAG_DISK_SPACE_TEXT).unwrap_or_default()),
        );

        if let Some(denied) = self.deny_below(Authority::COMPANY_ADMIN) {
            return denied;
        }

        let names = match ascii_names(&[company, group]) {
            Ok(names) => names,
            Err(response) => return response,
        };
        let [company, group] = names[..] else {
            unreachable!()
        };

        let company_id = match db::find_company(&self.conn, company) {
            Ok(Some(id)) => id,
            Ok(None) => {
                info!("sentry: company {company:?} is not defined");
                return status_response(STATUS_COMPANY_NOT_DEFINED);
            }
            Err(err) => return self.read_failed(&err),
        };

        self.store(|conn| db::insert_group(conn, company_id, group, quota))
    }

    fn add_user(&mut self, records: &[TlvEntry]) -> Vec<u8> {
        let company = record(records, property::COMPANY).unwrap_or_default();
        let group = record(records, property::GROUP).unwrap_or_default();
        let user = record(records, property::USER).unwrap_or_default();
        let password = record(records, property::PASSWORD).unwrap_or_default();
        let quota = quota(records);
        let authority = record(records, TAG_AUTHORITY)
            .and_then(Authority::read)
            .unwrap_or(Authority::NORMAL);

        info!(
            "sentry: add user {:?} to {:?}/{:?}, password={:?}, authority={authority} ({}), \
             quota={}, disk space={:?}",
            BStr::new(user),
            BStr::new(company),
            BStr::new(group),
            BStr::new(password),
            authority.name(),
            quota_name(quota),
            BStr::new(record(records, TAG_DISK_SPACE_TEXT).unwrap_or_default()),
        );

        if let Some(denied) = self.deny_below(Authority::GROUP_ADMIN) {
            return denied;
        }

        if !authority.is_defined() {
            info!("sentry: refused the undefined authority {authority}");
            return status_response(STATUS_INVALID_AUTHORITY);
        }

        // Without this an account may mint one above itself and sign back on as
        // it, which turns the whole threshold ladder into a single step.
        let actor = self.authority();
        if authority > actor {
            info!(
                "sentry: refused to grant {authority} ({}) from {actor} ({})",
                authority.name(),
                actor.name(),
            );
            return status_response(STATUS_INSUFFICIENT_AUTHORITY);
        }

        let names = match ascii_names(&[company, group, user, password]) {
            Ok(names) => names,
            Err(response) => return response,
        };
        let [company, group, user, password] = names[..] else {
            unreachable!()
        };

        let group_id = match db::find_group(&self.conn, company, group) {
            Ok(Some(id)) => id,
            Ok(None) => {
                info!("sentry: group {group:?} is not defined");
                return status_response(STATUS_ACCOUNT_NOT_DEFINED);
            }
            Err(err) => return self.read_failed(&err),
        };

        self.store(|conn| db::insert_user(conn, group_id, user, password, authority.0, quota))
    }

    /// The error arm is already the response: an unreadable store is not an
    /// empty one, and answering as if it were would invite the client to
    /// recreate accounts that are still there.
    fn directory(&self) -> Result<Directory, Vec<u8>> {
        Directory::load(&self.conn).map_err(|err| {
            info!("sentry: failed to read the directory: {err}");
            status_response(status::AUTHORIZATION_FILE)
        })
    }

    /// `None` means the command may proceed. The refusal is the whole
    /// enforcement — the variant byte only hides the menu, it does not stop a
    /// client that sends the command anyway.
    fn deny_below(&self, required: Authority) -> Option<Vec<u8>> {
        let actor = self.authority();
        if actor >= required {
            return None;
        }

        info!(
            "sentry: refused a command needing {required} ({}) from {actor} ({})",
            required.name(),
            actor.name(),
        );

        Some(status_response(STATUS_INSUFFICIENT_AUTHORITY))
    }

    fn read_failed(&self, err: &rusqlite::Error) -> Vec<u8> {
        info!("sentry: failed to read the directory: {err}");
        status_response(status::AUTHORIZATION_FILE)
    }

    /// The unique indexes are case insensitive, so the insert *is* the duplicate
    /// check: a lookup first would only add a race between the two.
    fn store(&self, insert: impl FnOnce(&Connection) -> rusqlite::Result<()>) -> Vec<u8> {
        match insert(&self.conn) {
            Ok(()) => status_response(status::OK),
            Err(err) => {
                info!("sentry: failed to store the record: {err}");
                if is_constraint_violation(&err) {
                    status_response(STATUS_ALREADY_DEFINED)
                } else {
                    status_response(status::AUTHORIZATION_FILE)
                }
            }
        }
    }
}

/// The client repeats the previous row as the filter of the next `0x1e`, so
/// the three names address a record instead of starting a search: the answer
/// is the row right after the last one matching them.
fn seek(directory: &Directory, records: &[TlvEntry]) -> Option<usize> {
    let prefix: &[&[u8]] = &match (
        filter(records, property::COMPANY),
        filter(records, property::GROUP),
        filter(records, property::USER),
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

    Some(directory.find(prefix)? + 1)
}

/// Answers one step of a listing. Running past the last row ends the
/// enumeration with the status the client clears without complaining.
fn listing_response(directory: &Directory, index: Option<usize>) -> Result<Vec<u8>, FrameError> {
    let Some((index, row)) = index.and_then(|index| Some((index, directory.get(index)?))) else {
        info!("sentry: end of listing");
        return Ok(status_response(STATUS_END_OF_LISTING));
    };

    info!(
        "sentry: listing row {index}: company={:?}, group={:?}, user={:?}, authority={} ({})",
        BStr::new(&row.company),
        BStr::new(&row.group),
        BStr::new(&row.user),
        row.authority,
        row.authority.name(),
    );

    let mut payload = vec![COMMAND_REPLY];
    write_tagged(&mut payload, TAG_CURSOR, index.to_string().as_bytes())?;
    write_tagged(&mut payload, property::COMPANY, &row.company)?;
    write_tagged(&mut payload, property::GROUP, &row.group)?;
    write_tagged(&mut payload, property::USER, &row.user)?;
    write_tagged(&mut payload, TAG_AUTHORITY, &row.authority.write())?;
    write_tagged(&mut payload, TAG_DEVICE, DEVICE)?;
    write_tagged(&mut payload, TAG_QUOTA, &row.quota.to_le_bytes())?;
    write_tagged(&mut payload, TAG_DISK_USED, &row.used.to_le_bytes())?;
    write_tagged(&mut payload, TAG_CREATED, CREATED)?;
    write_tagged(&mut payload, TAG_MODIFIED, MODIFIED)?;
    write_tagged(&mut payload, TAG_LOCKED, &UNLOCKED)?;
    Ok(payload)
}

fn is_constraint_violation(err: &rusqlite::Error) -> bool {
    matches!(
        err,
        rusqlite::Error::SqliteFailure(error, _)
            if error.code == rusqlite::ErrorCode::ConstraintViolation
    )
}

/// Names are stored as text so the database can compare them the way the client
/// does — case insensitively, which SQLite offers over `TEXT` only. The GRiD
/// keyboard cannot produce anything outside ASCII, so a name that is not ASCII
/// did not come from the client this server speaks to.
///
/// An empty field is refused here too: an absent record reads as an empty value,
/// and an empty name would create an account that sign-on then matches against
/// its own empty properties.
fn ascii_names<'a>(fields: &[&'a [u8]]) -> Result<Vec<&'a str>, Vec<u8>> {
    fields
        .iter()
        .map(|field| {
            if field.is_empty() {
                return Err(status_response(status::PROPERTY_MISSING));
            }

            str::from_utf8(field)
                .ok()
                .filter(|name| name.is_ascii())
                .ok_or_else(|| {
                    info!("sentry: refused the non-ASCII name {:?}", BStr::new(field));
                    status_response(STATUS_INVALID_NAME)
                })
        })
        .collect()
}

fn records(data: &[u8]) -> Option<Vec<TlvEntry<'_>>> {
    Tlv::tag_u8(data).collect_all().ok()
}

fn record<'a>(records: &[TlvEntry<'a>], wanted: u8) -> Option<&'a [u8]> {
    records
        .iter()
        .find(|entry| entry.tag == wanted)
        .map(|entry| entry.value)
}

/// Every value written here is a fixed width field or a name the client already
/// bounds, so an oversized one is a bug in this server rather than something a
/// client can trigger: it is reported instead of truncated.
fn write_tagged(dst: &mut Vec<u8>, tag: u8, value: &[u8]) -> Result<(), FrameError> {
    dst.reserve(value.len() + 2);
    dst.write_u8(tag)?;
    dst.write_u8(u8_len(value.len(), "Sentry record")?)?;
    dst.write_all(value)?;
    Ok(())
}

#[cfg(test)]
fn tagged(tag: u8, value: &[u8]) -> Vec<u8> {
    let mut record = Vec::new();
    write_tagged(&mut record, tag, value).unwrap();
    record
}

/// A normal user is told the variant is `1`, which makes the client replace the
/// administrator menu with a "not supported" notice — the client's own way of
/// hiding what the account may not do, and cheaper than refusing each command.
fn variant_response(actor: Authority) -> Result<Vec<u8>, FrameError> {
    let variant = if actor > Authority::NORMAL {
        VARIANT
    } else {
        VARIANT_UNSUPPORTED
    };

    let mut payload = vec![COMMAND_REPLY];
    write_tagged(&mut payload, TAG_VARIANT, &[variant])?;
    Ok(payload)
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

fn filter<'a>(records: &[TlvEntry<'a>], tag: u8) -> Filter<'a> {
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

/// The quota is the only numeric field the client writes little endian.
fn quota(records: &[TlvEntry]) -> u32 {
    record(records, TAG_QUOTA)
        .and_then(|value| <[u8; 4]>::try_from(value).ok())
        .map_or(0, u32::from_le_bytes)
}

/// `0x1f` continues a user listing by echoing the cursor of the previous row.
fn resume(records: &[TlvEntry]) -> Option<usize> {
    let cursor = record(records, TAG_CURSOR)?;
    let index: usize = std::str::from_utf8(cursor).ok()?.parse().ok()?;

    Some(index + 1)
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

    const MEGABYTE: u32 = 1024 * 1024;

    /// Signed on as the only account entitled to every administrative command;
    /// tests that care about lower authority build their own.
    fn sentry() -> SentryServer {
        sentry_as(Authority::SYSTEM_ADMIN)
    }

    fn sentry_as(authority: Authority) -> SentryServer {
        let conn = Rc::new(db::open_in_memory());
        let actor = signed_on(&conn, authority);

        SentryServer::new(conn, actor)
    }

    /// The actor is a whole account now, so a test signs on as one of the demo
    /// users rather than conjuring an authority with nobody behind it.
    fn signed_on(conn: &Connection, authority: Authority) -> db::Account {
        let (group, user) = match authority {
            Authority::NORMAL => ("Demo", "GUEST"),
            Authority::GROUP_ADMIN => ("Demo", "OPERATOR"),
            Authority::SYSTEM_ADMIN => ("Systems", "MANAGER"),
            _ => panic!("the demo directory holds no {authority} account"),
        };

        db::find_user(conn, "GRiD", group, user)
            .expect("read the demo directory")
            .expect("the demo account should exist")
    }

    #[test]
    fn answers_variant_query() {
        let mut sentry = sentry();

        let response = sentry.process(&[COMMAND_VARIANT_QUERY]).unwrap();

        assert_eq!(response, [0x03, 0x25, 0x01, b'3']);
    }

    #[test]
    fn answers_add_user_with_success_status() {
        let mut sentry = sentry();
        let request = [
            0x06, // Add User
            0x07, 0x04, b'G', b'R', b'i', b'D', // Company
            0x08, 0x04, b'D', b'e', b'm', b'o', // Group
            0x09, 0x07, b'Z', b'Z', b'C', b'A', b'P', b'0', b'1', // User
            0x0a, 0x05, b'T', b'E', b'S', b'T', b'1', // Password
            0x1a, 0x02, 0x00, 0x00, // Normal user
            0x26, 0x04, 0x00, 0x04, 0x00, 0x00, // 1024 bytes
            0x1b, 0x01, b'1', // Disk space text
        ];

        let response = sentry.process(&request).unwrap();

        assert_eq!(response, [0x02, 0x00, 0x00]);
    }

    fn add_company(sentry: &mut SentryServer, company: &[u8]) -> Vec<u8> {
        let mut request = vec![COMMAND_ADD_COMPANY];
        request.extend(tagged(property::COMPANY, company));
        request.extend(tagged(TAG_QUOTA, &QUOTA_UNLIMITED.to_le_bytes()));
        sentry.process(&request).unwrap()
    }

    fn add_group(sentry: &mut SentryServer, company: &[u8], group: &[u8]) -> Vec<u8> {
        let mut request = vec![COMMAND_ADD_GROUP];
        request.extend(tagged(property::COMPANY, company));
        request.extend(tagged(property::GROUP, group));
        request.extend(tagged(TAG_QUOTA, &QUOTA_UNLIMITED.to_le_bytes()));
        sentry.process(&request).unwrap()
    }

    fn add_user(
        sentry: &mut SentryServer,
        company: &[u8],
        group: &[u8],
        user: &[u8],
        password: &[u8],
        authority: Authority,
    ) -> Vec<u8> {
        let mut request = vec![COMMAND_ADD_USER];
        request.extend(tagged(property::COMPANY, company));
        request.extend(tagged(property::GROUP, group));
        request.extend(tagged(property::USER, user));
        request.extend(tagged(property::PASSWORD, password));
        request.extend(tagged(TAG_AUTHORITY, &authority.0.to_be_bytes()));
        request.extend(tagged(TAG_QUOTA, &MEGABYTE.to_le_bytes()));
        sentry.process(&request).unwrap()
    }

    fn names(sentry: &mut SentryServer) -> Vec<(Vec<u8>, Vec<u8>, Vec<u8>)> {
        let mut rows = Vec::new();
        let mut request = list_first(b"", b"", b"");

        while let Some(response) = sentry.process(&request) {
            if response[0] == COMMAND_STATUS {
                break;
            }

            let records = records(&response[1..]).unwrap();
            let text = |tag| record(&records, tag).unwrap().to_vec();
            rows.push((
                text(property::COMPANY),
                text(property::GROUP),
                text(property::USER),
            ));
            request = list_first(
                &text(property::COMPANY),
                &text(property::GROUP),
                &text(property::USER),
            );
        }

        rows
    }

    #[test]
    fn created_company_appears_in_the_listing() {
        let mut sentry = sentry();

        assert_eq!(
            add_company(&mut sentry, b"ACME"),
            status_response(status::OK)
        );

        // Rows come back sorted, so a company named before `GRiD` leads the
        // listing rather than trailing it.
        let rows = names(&mut sentry);
        assert_eq!(rows.first().unwrap().0, b"ACME");
        assert!(sentry.directory().unwrap().rows[0].is_company());
    }

    #[test]
    fn created_group_lands_inside_its_company() {
        let mut sentry = sentry();

        assert_eq!(
            add_group(&mut sentry, b"GRiD", b"Payroll"),
            status_response(status::OK)
        );

        // The group sorts into its company's block rather than onto the end, so
        // the block stays contiguous and the client keeps walking it in order.
        let rows = names(&mut sentry);
        let groups: Vec<_> = rows
            .iter()
            .filter(|(company, group, user)| group == user && company != group)
            .map(|(_, group, _)| group.clone())
            .collect();
        assert_eq!(
            groups,
            [b"Demo".to_vec(), b"Payroll".into(), b"Systems".into()]
        );
        assert!(rows.iter().all(|(company, _, _)| company == b"GRiD"));
    }

    #[test]
    fn created_user_lands_inside_its_group() {
        let mut sentry = sentry();

        assert_eq!(
            add_user(
                &mut sentry,
                b"GRiD",
                b"Demo",
                b"CLERK",
                b"SECRET",
                Authority::NORMAL
            ),
            status_response(status::OK)
        );

        let rows = names(&mut sentry);
        let demo: Vec<_> = rows
            .iter()
            .filter(|(_, group, _)| group == b"Demo")
            .map(|(_, _, user)| user.clone())
            .collect();
        // Users sort within their group, so the new one is not necessarily last
        // — what matters is that it stays inside `Demo`.
        assert_eq!(
            demo,
            [
                b"Demo".to_vec(),
                b"CLERK".into(),
                b"GUEST".into(),
                b"OPERATOR".into()
            ]
        );
    }

    #[test]
    fn refuses_an_administrative_command_from_a_normal_user() {
        let mut sentry = sentry_as(Authority::NORMAL);

        assert_eq!(
            add_company(&mut sentry, b"ACME"),
            status_response(STATUS_INSUFFICIENT_AUTHORITY)
        );
        assert_eq!(
            add_group(&mut sentry, b"GRiD", b"Payroll"),
            status_response(STATUS_INSUFFICIENT_AUTHORITY)
        );
    }

    /// Each command has its own threshold, so a group administrator may add
    /// users but not the groups or companies above them.
    #[test]
    fn allows_only_what_the_authority_covers() {
        let mut sentry = sentry_as(Authority::GROUP_ADMIN);

        assert_eq!(
            add_user(
                &mut sentry,
                b"GRiD",
                b"Demo",
                b"CLERK",
                b"SECRET",
                Authority::NORMAL
            ),
            status_response(status::OK)
        );
        assert_eq!(
            add_group(&mut sentry, b"GRiD", b"Payroll"),
            status_response(STATUS_INSUFFICIENT_AUTHORITY)
        );
    }

    #[test]
    fn tells_a_normal_user_the_variant_is_unsupported() {
        let mut sentry = sentry_as(Authority::NORMAL);

        let response = sentry.process(&[COMMAND_VARIANT_QUERY]).unwrap();

        assert_eq!(response, [COMMAND_REPLY, TAG_VARIANT, 0x01, b'1']);
    }

    /// The password arrives with the add-user request and is stored as sent —
    /// in the clear, which is what the client's sign-on compares against.
    /// The gate is the only enforcement — the variant byte merely hides the
    /// menu, so a client that sends the command anyway must still be refused.
    #[test]
    fn refuses_to_create_a_user_from_a_normal_account() {
        let mut sentry = sentry_as(Authority::NORMAL);

        assert_eq!(
            add_user(
                &mut sentry,
                b"GRiD",
                b"Demo",
                b"CLERK",
                b"SECRET",
                Authority::NORMAL
            ),
            status_response(STATUS_INSUFFICIENT_AUTHORITY)
        );
        assert!(
            db::find_user(&sentry.conn, "GRiD", "Demo", "CLERK")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn refuses_to_grant_an_authority_above_the_creators_own() {
        let mut sentry = sentry_as(Authority::GROUP_ADMIN);

        assert_eq!(
            add_user(
                &mut sentry,
                b"GRiD",
                b"Demo",
                b"CLERK",
                b"SECRET",
                Authority::SYSTEM_ADMIN
            ),
            status_response(STATUS_INSUFFICIENT_AUTHORITY)
        );
    }

    #[test]
    fn refuses_an_authority_outside_the_defined_levels() {
        let mut sentry = sentry();

        assert_eq!(
            add_user(
                &mut sentry,
                b"GRiD",
                b"Demo",
                b"CLERK",
                b"SECRET",
                Authority(25)
            ),
            status_response(STATUS_INVALID_AUTHORITY)
        );
    }

    #[test]
    fn refuses_the_mandatory_fields_when_empty() {
        let mut sentry = sentry();

        assert_eq!(
            add_company(&mut sentry, b""),
            status_response(status::PROPERTY_MISSING)
        );
        assert_eq!(
            add_group(&mut sentry, b"GRiD", b""),
            status_response(status::PROPERTY_MISSING)
        );
        assert_eq!(
            add_user(
                &mut sentry,
                b"GRiD",
                b"Demo",
                b"",
                b"SECRET",
                Authority::NORMAL
            ),
            status_response(status::PROPERTY_MISSING)
        );
        assert_eq!(
            add_user(
                &mut sentry,
                b"GRiD",
                b"Demo",
                b"CLERK",
                b"",
                Authority::NORMAL
            ),
            status_response(status::PROPERTY_MISSING)
        );
    }

    /// A child is attached to its parent by row id, so the listing reads the
    /// parent's name back from the parent's own row: however the client spelled
    /// it, every child reports the one stored spelling.
    #[test]
    fn stores_the_parent_spelling_the_directory_already_uses() {
        let mut sentry = sentry();

        assert_eq!(
            add_user(
                &mut sentry,
                b"grid",
                b"demo",
                b"CLERK",
                b"SECRET",
                Authority::NORMAL
            ),
            status_response(status::OK)
        );

        let account = db::find_user(&sentry.conn, "GRiD", "Demo", "CLERK")
            .unwrap()
            .expect("the created user should be stored");
        assert_eq!(account.company, "GRiD");
        assert_eq!(account.group, "Demo");

        let rows = names(&mut sentry);
        assert!(rows.iter().all(|(company, _, _)| company == b"GRiD"));
    }

    /// The client cannot produce a name outside ASCII, and the store keeps names
    /// as text, so such a name is refused rather than transliterated or stored.
    #[test]
    fn refuses_a_name_that_is_not_ascii() {
        let mut sentry = sentry();

        assert_eq!(
            add_user(
                &mut sentry,
                b"GRiD",
                b"Demo",
                "КЛЕРК".as_bytes(),
                b"SECRET",
                Authority::NORMAL
            ),
            status_response(STATUS_INVALID_NAME)
        );
    }

    #[test]
    fn reports_a_missing_group_apart_from_a_missing_company() {
        let mut sentry = sentry();

        assert_eq!(
            add_user(
                &mut sentry,
                b"GRiD",
                b"Nowhere",
                b"CLERK",
                b"SECRET",
                Authority::NORMAL
            ),
            status_response(STATUS_ACCOUNT_NOT_DEFINED)
        );
        assert_eq!(
            add_group(&mut sentry, b"Nowhere", b"Payroll"),
            status_response(STATUS_COMPANY_NOT_DEFINED)
        );
    }

    /// The Sentry and sign-on only meet through the database, so a user the one
    /// created has to be a user the other then accepts.
    #[test]
    fn a_created_user_can_sign_on() {
        let conn = Rc::new(db::open_in_memory());
        let actor = signed_on(&conn, Authority::SYSTEM_ADMIN);
        let mut sentry = SentryServer::new(conn.clone(), actor);

        assert_eq!(
            add_user(
                &mut sentry,
                b"GRiD",
                b"Demo",
                b"CLERK",
                b"SECRET",
                Authority::GROUP_ADMIN
            ),
            status_response(status::OK)
        );

        let account = crate::authenticate(
            &conn,
            &crate::sign_on_properties(b"GRiD", b"Demo", b"CLERK", b"SECRET"),
        )
        .expect("the created user should be able to sign on");

        assert_eq!(
            Authority::from_stored(account.authority),
            Authority::GROUP_ADMIN
        );
    }

    #[test]
    fn stores_the_password_of_a_created_user() {
        let mut sentry = sentry();
        add_user(
            &mut sentry,
            b"GRiD",
            b"Demo",
            b"CLERK",
            b"SECRET",
            Authority::NORMAL,
        );

        let account = db::find_user(&sentry.conn, "GRiD", "Demo", "CLERK")
            .unwrap()
            .expect("the created user should be stored");
        assert_eq!(account.password, "SECRET");
    }

    #[test]
    fn rejects_a_duplicate_company() {
        let mut sentry = sentry();

        assert_eq!(
            add_company(&mut sentry, b"GRiD"),
            status_response(STATUS_ALREADY_DEFINED)
        );
    }

    #[test]
    fn rejects_a_group_without_its_company() {
        let mut sentry = sentry();

        assert_eq!(
            add_group(&mut sentry, b"MISSING", b"Payroll"),
            status_response(STATUS_COMPANY_NOT_DEFINED)
        );
    }

    #[test]
    fn a_created_company_accepts_groups() {
        let mut sentry = sentry();

        add_company(&mut sentry, b"ACME");

        assert_eq!(
            add_group(&mut sentry, b"acme", b"Sales"),
            status_response(status::OK)
        );
        let directory = sentry.directory().unwrap();
        let sales = directory.find(&[b"ACME", b"Sales"]).unwrap();
        assert!(directory.rows[sales].is_group());
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

        assert_eq!(
            record(&records, property::COMPANY),
            Some(b"ACME".as_slice())
        );
        assert_eq!(record(&records, property::USER), Some(b"BOB".as_slice()));
        assert_eq!(record(&records, property::GROUP), None);
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
        let mut sentry = sentry();

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
        request.extend(list_filter(property::COMPANY, company));
        request.extend(list_filter(property::GROUP, group));
        request.extend(list_filter(property::USER, user));
        request
    }

    #[test]
    fn answers_first_listing_request_with_the_first_row() {
        let mut sentry = sentry();

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
        let mut sentry = sentry();
        let mut rows = Vec::new();

        let mut request = list_first(b"", b"", b"");
        while let Some(response) = sentry.process(&request) {
            if response[0] == COMMAND_STATUS {
                assert_eq!(response, status_response(STATUS_END_OF_LISTING));
                break;
            }

            let records = records(&response[1..]).unwrap();
            let text = |tag| record(&records, tag).unwrap().to_vec();
            rows.push((
                text(property::COMPANY),
                text(property::GROUP),
                text(property::USER),
            ));

            // The client echoes the row it just parsed as the next filter.
            request = list_first(
                &text(property::COMPANY),
                &text(property::GROUP),
                &text(property::USER),
            );
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
        let mut sentry = sentry();
        let mut request = vec![COMMAND_LIST_FIRST];
        request.extend(list_filter(property::COMPANY, b"GRiD"));
        request.extend(list_filter(property::GROUP, b"Demo"));
        request.extend(tagged(property::USER, &[FILTER_SKIP; 20]));

        let response = sentry.process(&request).unwrap();

        let records = records(&response[1..]).unwrap();
        assert_eq!(
            record(&records, property::GROUP),
            Some(b"Systems".as_slice())
        );
        assert_eq!(
            record(&records, property::USER),
            Some(b"Systems".as_slice())
        );
    }

    #[test]
    fn continues_a_user_listing_from_the_cursor() {
        let mut sentry = sentry();
        let mut request = vec![COMMAND_LIST_NEXT];
        request.extend(tagged(TAG_CURSOR, b"2"));

        let response = sentry.process(&request).unwrap();

        let records = records(&response[1..]).unwrap();
        assert_eq!(record(&records, TAG_CURSOR), Some(b"3".as_slice()));
        assert_eq!(
            record(&records, property::USER),
            Some(b"OPERATOR".as_slice())
        );
    }

    #[test]
    fn ends_the_listing_after_the_last_row() {
        let mut sentry = sentry();
        let last = list_first(b"GRiD", b"Systems", b"MANAGER");

        let response = sentry.process(&last).unwrap();

        assert_eq!(response, [0x02, 0xed, 0x03]);
    }

    #[test]
    fn ends_the_listing_when_the_whole_company_is_skipped() {
        let mut sentry = sentry();
        let mut request = vec![COMMAND_LIST_FIRST];
        request.extend(tagged(property::COMPANY, &[FILTER_SKIP; 20]));
        request.extend(tagged(property::GROUP, &[FILTER_SKIP; 20]));
        request.extend(tagged(property::USER, &[FILTER_SKIP; 20]));

        let response = sentry.process(&request).unwrap();

        assert_eq!(response, status_response(STATUS_END_OF_LISTING));
    }

    #[test]
    fn ends_the_listing_for_an_unknown_filter() {
        let mut sentry = sentry();

        let response = sentry.process(&list_first(b"MISSING", b"", b"")).unwrap();

        assert_eq!(response, status_response(STATUS_END_OF_LISTING));
    }

    /// Every mandatory record must fit the destination the client allocates,
    /// otherwise `sub_3332` silently truncates the value.
    #[test]
    fn listing_records_fit_the_client_buffers() {
        let mut sentry = sentry();

        let response = sentry.process(&list_first(b"", b"", b"")).unwrap();

        let records = records(&response[1..]).unwrap();
        let capacities = [
            (TAG_CURSOR, 100),
            (property::COMPANY, 20),
            (property::GROUP, 20),
            (property::USER, 20),
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
