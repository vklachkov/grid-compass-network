use std::{io::Write, rc::Rc};

use bstr::BStr;
use log::{debug, error, warn};
use rusqlite::Connection;

use super::protocol::{property, status};
use crate::{
    db,
    shared::{
        FrameError, Tlv, TlvEntry,
        io::{WriteExt, u8_len},
    },
};

/// Update and Delete are the two commands the client answers to with a bare
/// acknowledgement; anything else it reads as the error path.
const COMMAND_ACK: u8 = 0x01;
const COMMAND_STATUS: u8 = 0x02;
const COMMAND_REPLY: u8 = 0x03;
const COMMAND_VARIANT_QUERY: u8 = 0x04;
const COMMAND_CHANGE_PASSWORD: u8 = 0x05;
const COMMAND_ADD_USER: u8 = 0x06;
const COMMAND_QUERY: u8 = 0x0e;
const COMMAND_DELETE: u8 = 0x12;
const COMMAND_UPDATE: u8 = 0x16;
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
const TAG_QUERY_LEVEL: u8 = 0x34;

const QUERY_COMPANY: u8 = 1;
const QUERY_GROUP: u8 = 2;
const QUERY_USER: u8 = 3;

/// The listing loop clears `1005: eUnknownUser` silently instead of showing an
/// error dialog, so it is how an enumeration ends.
const STATUS_END_OF_LISTING: u16 = 1005;
const STATUS_INVALID_AUTHORITY: u16 = 1006; // eInvalidAuthority
const STATUS_COMPANY_NOT_DEFINED: u16 = 1015; // eCompanyNotDefined
/// The company exists but the group does not.
const STATUS_ACCOUNT_NOT_DEFINED: u16 = 1016; // eAccountNotDefined
const STATUS_ALREADY_DEFINED: u16 = 1017; // eAlreadyDefined
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
    /// Expands a stored row back into the wire form, repeating the name above
    /// into the levels that do not apply.
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

    fn level(&self) -> Level {
        if self.company == self.group && self.group == self.user {
            Level::Company
        } else if self.group == self.user {
            Level::Group
        } else {
            Level::User
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Level {
    Company,
    Group,
    User,
}

impl Level {
    /// The threshold a write at this level needs, matching the add commands.
    fn required(self) -> Authority {
        match self {
            Self::Company => Authority::SYSTEM_ADMIN,
            Self::Group => Authority::COMPANY_ADMIN,
            Self::User => Authority::GROUP_ADMIN,
        }
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
                error!(target: "sentry", "failed to build a response: {err}");
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
            COMMAND_LIST_FIRST => self.list(&records, seek)?,
            COMMAND_LIST_NEXT => self.list(&records, |_, records| resume(records))?,
            COMMAND_QUERY => self.query(&records)?,
            COMMAND_UPDATE => self.update(&records),
            COMMAND_DELETE => self.delete(&records),
            COMMAND_CHANGE_PASSWORD => self.change_password(&records),
            command => {
                warn!(target: "sentry", "ignored unsupported command {command:#04x} with {records:?}");
                return Ok(None);
            }
        };

        Ok(Some(response))
    }

    /// A name the request leaves out is taken from the signed-on account, which
    /// is what makes UserSentry's "Query User Information" work: it sends the
    /// level alone, meaning "the account I am", while AdministratorSentry fills
    /// its form fields in.
    fn query(&self, records: &[TlvEntry]) -> Result<Vec<u8>, FrameError> {
        let level = match record(records, TAG_QUERY_LEVEL) {
            Some([level]) => *level,
            _ => QUERY_USER,
        };

        let company = record(records, property::COMPANY)
            .filter(|name| !name.is_empty())
            .unwrap_or(self.actor.company.as_bytes());
        let group = record(records, property::GROUP)
            .filter(|name| !name.is_empty())
            .unwrap_or(self.actor.group.as_bytes());
        let user = record(records, property::USER)
            .filter(|name| !name.is_empty())
            .unwrap_or(self.actor.user.as_bytes());

        // Addressed by the full triple rather than a prefix: repeating the name
        // above is what picks a company row out of its own subtree.
        let names: [&[u8]; 3] = match level {
            QUERY_COMPANY => [company, company, company],
            QUERY_GROUP => [company, group, group],
            QUERY_USER => [company, group, user],
            level => {
                warn!(target: "sentry", "refused the unknown query level {level}");
                return Ok(status_response(STATUS_ACCOUNT_NOT_DEFINED));
            }
        };

        debug!(
            target: "sentry",
            "query level {level} for {:?}/{:?}/{:?}",
            BStr::new(names[0]),
            BStr::new(names[1]),
            BStr::new(names[2]),
        );

        let directory = match self.directory() {
            Ok(directory) => directory,
            Err(response) => return Ok(response),
        };

        let Some(index) = directory.find(&names) else {
            let status = if level == QUERY_COMPANY {
                STATUS_COMPANY_NOT_DEFINED
            } else {
                STATUS_ACCOUNT_NOT_DEFINED
            };
            warn!(target: "sentry", "the queried record is not defined");
            return Ok(status_response(status));
        };

        if let Some(denied) = self.deny_outside(&names) {
            return Ok(denied);
        }

        row_response(&directory, index)
    }

    /// Walking the directory is an administrator's business, and each step
    /// skips the rows outside the walker's subtree rather than stopping at
    /// them: the client starts from the very first row, so an administrator
    /// whose own block sorts after somebody else's has to be carried past it.
    fn list(
        &self,
        records: &[TlvEntry],
        start: impl FnOnce(&Directory, &[TlvEntry]) -> Option<usize>,
    ) -> Result<Vec<u8>, FrameError> {
        if let Some(denied) = self.deny_below(Authority::GROUP_ADMIN) {
            return Ok(denied);
        }

        let directory = match self.directory() {
            Ok(directory) => directory,
            Err(response) => return Ok(response),
        };

        let index = start(&directory, records).map(|index| {
            (index..directory.rows.len()).find(|index| {
                directory
                    .get(*index)
                    .is_some_and(|row| !self.outside(&row.names()))
            })
        });

        listing_response(&directory, index.flatten())
    }

    /// The request carries no names, only the cursor of the preceding query, so
    /// an update can never reach a row the client was not allowed to read.
    fn update(&mut self, records: &[TlvEntry]) -> Vec<u8> {
        let directory = match self.directory() {
            Ok(directory) => directory,
            Err(response) => return response,
        };

        let Some(row) = cursor(records).and_then(|index| directory.get(index)) else {
            warn!(target: "sentry", "refused an update without a valid cursor");
            return status_response(STATUS_ACCOUNT_NOT_DEFINED);
        };

        let authority = record(records, TAG_AUTHORITY)
            .and_then(Authority::read)
            .unwrap_or(row.authority);
        let quota = quota_field(records).unwrap_or(row.quota);

        debug!(
            target: "sentry",
            "update {:?}/{:?}/{:?} to authority={authority} ({}), quota={}",
            BStr::new(&row.company),
            BStr::new(&row.group),
            BStr::new(&row.user),
            authority.name(),
            quota_name(quota),
        );

        if let Some(denied) = self.deny_write(row) {
            return denied;
        }

        if !authority.is_defined() {
            warn!(target: "sentry", "refused the undefined authority {authority}");
            return status_response(STATUS_INVALID_AUTHORITY);
        }

        // The same escalation guard the add path has: an account that may edit a
        // record must still not raise it above itself.
        let actor = self.authority();
        if authority > actor {
            warn!(
                target: "sentry",
                "refused to grant {authority} ({}) from {actor} ({})",
                authority.name(),
                actor.name(),
            );
            return status_response(STATUS_INSUFFICIENT_AUTHORITY);
        }

        let names = row.names();
        let Ok([company, group, user]) = stored_names(&names) else {
            return status_response(status::AUTHORIZATION_FILE);
        };

        self.write(|conn| match row.level() {
            Level::Company => db::update_company(conn, company, quota),
            Level::Group => db::update_group(conn, company, group, quota),
            Level::User => db::update_user(conn, company, group, user, authority.0, quota),
        })
    }

    /// Addressed by the cursor alone, like [`Self::update`].
    fn delete(&mut self, records: &[TlvEntry]) -> Vec<u8> {
        let directory = match self.directory() {
            Ok(directory) => directory,
            Err(response) => return response,
        };

        let Some(row) = cursor(records).and_then(|index| directory.get(index)) else {
            warn!(target: "sentry", "refused a delete without a valid cursor");
            return status_response(STATUS_ACCOUNT_NOT_DEFINED);
        };

        debug!(
            target: "sentry",
            "delete {:?}/{:?}/{:?}",
            BStr::new(&row.company),
            BStr::new(&row.group),
            BStr::new(&row.user),
        );

        if let Some(denied) = self.deny_write(row) {
            return denied;
        }

        // Deleting the account this session signed on as would leave the session
        // authenticated against a record that no longer exists.
        if row.names() == self.actor_names() {
            warn!(target: "sentry", "refused to delete the signed-on account");
            return status_response(STATUS_INSUFFICIENT_AUTHORITY);
        }

        let names = row.names();
        let Ok([company, group, user]) = stored_names(&names) else {
            return status_response(status::AUTHORIZATION_FILE);
        };

        self.write(|conn| match row.level() {
            Level::Company => db::delete_company(conn, company),
            Level::Group => db::delete_group(conn, company, group),
            Level::User => db::delete_user(conn, company, group, user),
        })
    }

    /// The three names are optional and the password is not: the names an
    /// account leaves out mean its own, which is how UserSentry changes the
    /// password of the session without naming it.
    fn change_password(&mut self, records: &[TlvEntry]) -> Vec<u8> {
        let company = self.name_or_own(records, property::COMPANY, &self.actor.company);
        let group = self.name_or_own(records, property::GROUP, &self.actor.group);
        let user = self.name_or_own(records, property::USER, &self.actor.user);
        let password = record(records, property::PASSWORD).unwrap_or_default();

        debug!(
            target: "sentry",
            "change the password of {:?}/{:?}/{:?}",
            BStr::new(company),
            BStr::new(group),
            BStr::new(user),
        );

        let names = match ascii_names(&[company, group, user, password]) {
            Ok(names) => names,
            Err(response) => return response,
        };
        let [company, group, user, password] = names[..] else {
            unreachable!()
        };

        // Changing one's own password is the one operation every account may
        // perform, so the gate applies only to somebody else's.
        if [company.as_bytes(), group.as_bytes(), user.as_bytes()] != self.actor_names() {
            if let Some(denied) = self.deny_below(Authority::GROUP_ADMIN) {
                return denied;
            }
            if let Some(denied) =
                self.deny_outside(&[company.as_bytes(), group.as_bytes(), user.as_bytes()])
            {
                return denied;
            }
        }

        match db::set_password(&self.conn, company, group, user, password) {
            Ok(0) => {
                warn!(target: "sentry", "the account whose password to change is not defined");
                status_response(STATUS_ACCOUNT_NOT_DEFINED)
            }
            Ok(_) => status_response(status::OK),
            Err(err) => {
                error!(target: "sentry", "failed to store the password: {err}");
                status_response(status::AUTHORIZATION_FILE)
            }
        }
    }

    fn name_or_own<'a>(&self, records: &[TlvEntry<'a>], tag: u8, own: &'a str) -> &'a [u8] {
        record(records, tag)
            .filter(|name| !name.is_empty())
            .unwrap_or(own.as_bytes())
    }

    fn actor_names(&self) -> [&[u8]; 3] {
        [
            self.actor.company.as_bytes(),
            self.actor.group.as_bytes(),
            self.actor.user.as_bytes(),
        ]
    }

    /// A write needs both the level's own threshold and the record being inside
    /// the actor's scope; the two are separate because an account may well hold
    /// the level and still be aiming at another company.
    fn deny_write(&self, row: &Row) -> Option<Vec<u8>> {
        let level = row.level();
        if self.authority() < level.required() {
            let actor = self.authority();
            warn!(
                target: "sentry",
                "refused a write needing {} ({}) from {actor} ({})",
                level.required(),
                level.required().name(),
                actor.name(),
            );
            return Some(status_response(STATUS_INSUFFICIENT_AUTHORITY));
        }

        self.deny_outside(&row.names())
    }

    /// How far an account may look: a system administrator over the whole
    /// directory, a company administrator over its own company, a group
    /// administrator over its own group, and an account without any of those
    /// over nothing but its own record.
    ///
    /// An administrator is matched on as many names as its level covers, so the
    /// company and group its subtree hangs from stay readable while a sibling
    /// subtree does not. A normal user is matched on all three: its own row
    /// already names its company and group, and the rows themselves carry a
    /// quota and a disk usage that are not its business.
    fn outside(&self, names: &[&[u8]; 3]) -> bool {
        // `None` is an account that administers nothing and so has to match the
        // record whole.
        let scope = match self.authority() {
            Authority::SYSTEM_ADMIN => Some(0),
            Authority::COMPANY_ADMIN => Some(1),
            Authority::GROUP_ADMIN => Some(2),
            _ => None,
        };

        let depth = scope.map_or(3, |scope| depth(names).min(scope));
        let own = self.actor_names();
        !names
            .iter()
            .zip(own)
            .take(depth)
            .all(|(name, own)| name.eq_ignore_ascii_case(own))
    }

    fn deny_outside(&self, names: &[&[u8]; 3]) -> Option<Vec<u8>> {
        if !self.outside(names) {
            return None;
        }

        warn!(
            target: "sentry",
            "refused {} ({}) access to {:?}/{:?}/{:?}",
            self.authority(),
            self.authority().name(),
            BStr::new(names[0]),
            BStr::new(names[1]),
            BStr::new(names[2]),
        );

        Some(status_response(STATUS_INSUFFICIENT_AUTHORITY))
    }

    /// Update and Delete want a bare acknowledgement rather than the status word
    /// the add commands answer with.
    fn write(&self, apply: impl FnOnce(&Connection) -> rusqlite::Result<usize>) -> Vec<u8> {
        match apply(&self.conn) {
            Ok(0) => {
                warn!(target: "sentry", "the addressed record vanished before the write");
                status_response(STATUS_ACCOUNT_NOT_DEFINED)
            }
            Ok(_) => vec![COMMAND_ACK],
            Err(err) => {
                error!(target: "sentry", "failed to write the record: {err}");
                status_response(status::AUTHORIZATION_FILE)
            }
        }
    }

    fn add_company(&mut self, records: &[TlvEntry]) -> Vec<u8> {
        let company = record(records, property::COMPANY).unwrap_or_default();
        let quota = quota(records);

        debug!(
            target: "sentry",
            "add company {:?}, quota={}, disk space={:?}",
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

    fn add_group(&mut self, records: &[TlvEntry]) -> Vec<u8> {
        let company = record(records, property::COMPANY).unwrap_or_default();
        let group = record(records, property::GROUP).unwrap_or_default();
        let quota = quota(records);

        debug!(
            target: "sentry",
            "add group {:?} to company {:?}, quota={}, disk space={:?}",
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
                warn!(target: "sentry", "company {company:?} is not defined");
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

        debug!(
            target: "sentry",
            "add user {:?} to {:?}/{:?}, authority={authority} ({}), \
             quota={}, disk space={:?}",
            BStr::new(user),
            BStr::new(company),
            BStr::new(group),
            authority.name(),
            quota_name(quota),
            BStr::new(record(records, TAG_DISK_SPACE_TEXT).unwrap_or_default()),
        );

        if let Some(denied) = self.deny_below(Authority::GROUP_ADMIN) {
            return denied;
        }

        if !authority.is_defined() {
            warn!(target: "sentry", "refused the undefined authority {authority}");
            return status_response(STATUS_INVALID_AUTHORITY);
        }

        // Without this an account may mint one above itself and sign back on as
        // it, which turns the whole threshold ladder into a single step.
        let actor = self.authority();
        if authority > actor {
            warn!(
                target: "sentry",
                "refused to grant {authority} ({}) from {actor} ({})",
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
                warn!(target: "sentry", "group {group:?} is not defined");
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
            error!(target: "sentry", "failed to load the directory: {err}");
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

        warn!(
            target: "sentry",
            "refused a command needing {required} ({}) from {actor} ({})",
            required.name(),
            actor.name(),
        );

        Some(status_response(STATUS_INSUFFICIENT_AUTHORITY))
    }

    fn read_failed(&self, err: &rusqlite::Error) -> Vec<u8> {
        error!(target: "sentry", "failed to look up a directory record: {err}");
        status_response(status::AUTHORIZATION_FILE)
    }

    /// The unique indexes are case insensitive, so the insert *is* the duplicate
    /// check: a lookup first would only add a race between the two.
    fn store(&self, insert: impl FnOnce(&Connection) -> rusqlite::Result<()>) -> Vec<u8> {
        match insert(&self.conn) {
            Ok(()) => status_response(status::OK),
            Err(err) => {
                if is_constraint_violation(&err) {
                    warn!(target: "sentry", "failed to store the record: {err}");
                    status_response(STATUS_ALREADY_DEFINED)
                } else {
                    error!(target: "sentry", "failed to store the record: {err}");
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
        debug!(target: "sentry", "end of listing");
        return Ok(status_response(STATUS_END_OF_LISTING));
    };

    debug!(
        target: "sentry",
        "listing row {index}: company={:?}, group={:?}, user={:?}, authority={} ({})",
        BStr::new(&row.company),
        BStr::new(&row.group),
        BStr::new(&row.user),
        row.authority,
        row.authority.name(),
    );

    row_response(directory, index)
}

/// The eleven records a directory row carries. Listing and query share it
/// because the client parses both replies with the same routine, where a
/// missing tag raises its own error dialog, 5001 through 5011.
fn row_response(directory: &Directory, index: usize) -> Result<Vec<u8>, FrameError> {
    let Some(row) = directory.get(index) else {
        return Ok(status_response(STATUS_END_OF_LISTING));
    };

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
                    warn!(target: "sentry", "refused the non-ASCII name {:?}", BStr::new(field));
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

/// How many of the three names actually address something, given that the wire
/// form pads the unused levels by repeating the name above them.
fn depth(names: &[&[u8]; 3]) -> usize {
    match names {
        [company, group, _] if company.eq_ignore_ascii_case(group) => 1,
        [_, group, user] if group.eq_ignore_ascii_case(user) => 2,
        _ => 3,
    }
}

/// The cursor a query handed out, which is the index of the row in the loaded
/// directory.
fn cursor(records: &[TlvEntry]) -> Option<usize> {
    std::str::from_utf8(record(records, TAG_CURSOR)?)
        .ok()?
        .parse()
        .ok()
}

/// Names come back out of the store, so one that is not UTF-8 means the store
/// holds something this server never wrote.
fn stored_names<'a>(names: &[&'a [u8]; 3]) -> Result<[&'a str; 3], ()> {
    let mut text = [""; 3];
    for (slot, name) in text.iter_mut().zip(names) {
        *slot = str::from_utf8(name).map_err(|err| {
            error!(target: "sentry", "the directory holds a name that is not text: {err}");
        })?;
    }

    Ok(text)
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
    quota_field(records).unwrap_or(0)
}

/// Update has to tell "leave the quota alone" from "set it to zero", which the
/// add commands never need to.
fn quota_field(records: &[TlvEntry]) -> Option<u32> {
    record(records, TAG_QUOTA)
        .and_then(|value| <[u8; 4]>::try_from(value).ok())
        .map(u32::from_le_bytes)
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
            add_company(&mut sentry, b"Test Company"),
            status_response(status::OK)
        );

        // Rows come back sorted, so a company named after `GRiD` trails the
        // listing rather than leading it.
        let rows = names(&mut sentry);
        assert_eq!(rows.last().unwrap().0, b"Test Company");
        let directory = sentry.directory().unwrap();
        assert_eq!(directory.rows.last().unwrap().level(), Level::Company);
    }

    #[test]
    fn created_group_lands_inside_its_company() {
        let mut sentry = sentry();

        assert_eq!(
            add_group(&mut sentry, b"GRiD", b"Demo Group"),
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
            [b"Demo".to_vec(), b"Demo Group".into(), b"Systems".into()]
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
                b"Lenin",
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
                b"GUEST".into(),
                b"Lenin".into(),
                b"OPERATOR".into()
            ]
        );
    }

    #[test]
    fn refuses_an_administrative_command_from_a_normal_user() {
        let mut sentry = sentry_as(Authority::NORMAL);

        assert_eq!(
            add_company(&mut sentry, b"Test Company"),
            status_response(STATUS_INSUFFICIENT_AUTHORITY)
        );
        assert_eq!(
            add_group(&mut sentry, b"GRiD", b"Demo Group"),
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
                b"Lenin",
                b"SECRET",
                Authority::NORMAL
            ),
            status_response(status::OK)
        );
        assert_eq!(
            add_group(&mut sentry, b"GRiD", b"Demo Group"),
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
                b"Lenin",
                b"SECRET",
                Authority::NORMAL
            ),
            status_response(STATUS_INSUFFICIENT_AUTHORITY)
        );
        assert!(
            db::find_user(&sentry.conn, "GRiD", "Demo", "Lenin")
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
                b"Lenin",
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
                b"Lenin",
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
                b"Lenin",
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
                b"Lenin",
                b"SECRET",
                Authority::NORMAL
            ),
            status_response(status::OK)
        );

        let account = db::find_user(&sentry.conn, "GRiD", "Demo", "Lenin")
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
                "Lénin".as_bytes(),
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
                b"Lenin",
                b"SECRET",
                Authority::NORMAL
            ),
            status_response(STATUS_ACCOUNT_NOT_DEFINED)
        );
        assert_eq!(
            add_group(&mut sentry, b"Nowhere", b"Demo Group"),
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
                b"Lenin",
                b"SECRET",
                Authority::GROUP_ADMIN
            ),
            status_response(status::OK)
        );

        let account = crate::authenticate(
            &conn,
            &crate::sign_on_properties(b"GRiD", b"Demo", b"Lenin", b"SECRET"),
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
            b"Lenin",
            b"SECRET",
            Authority::NORMAL,
        );

        let account = db::find_user(&sentry.conn, "GRiD", "Demo", "Lenin")
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
            add_group(&mut sentry, b"MISSING", b"Demo Group"),
            status_response(STATUS_COMPANY_NOT_DEFINED)
        );
    }

    #[test]
    fn a_created_company_accepts_groups() {
        let mut sentry = sentry();

        add_company(&mut sentry, b"Test Company");

        assert_eq!(
            add_group(&mut sentry, b"test company", b"Demo Group"),
            status_response(status::OK)
        );
        let directory = sentry.directory().unwrap();
        let group = directory.find(&[b"Test Company", b"Demo Group"]).unwrap();
        assert_eq!(directory.rows[group].level(), Level::Group);
    }

    #[test]
    fn parses_add_user_fields() {
        let request = [
            0x07, 0x0c, b'T', b'e', b's', b't', b' ', b'C', b'o', b'm', b'p', b'a', b'n',
            b'y', // Company
            0x09, 0x05, b'L', b'e', b'n', b'i', b'n', // User
            0x1a, 0x02, 0x00, 0x28, // System administrator
            0x26, 0x04, 0xff, 0xff, 0xff, 0xff, // Unlimited
        ];

        let records = records(&request).unwrap();

        assert_eq!(
            record(&records, property::COMPANY),
            Some(b"Test Company".as_slice())
        );
        assert_eq!(record(&records, property::USER), Some(b"Lenin".as_slice()));
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

        assert!(sentry.process(&[0x7f]).is_none());
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

    fn query(level: u8, names: &[(u8, &[u8])]) -> Vec<u8> {
        let mut request = vec![COMMAND_QUERY];
        request.extend(tagged(TAG_QUERY_LEVEL, &[level]));
        for (tag, name) in names {
            request.extend(tagged(*tag, name));
        }
        request
    }

    /// UserSentry's "Query User Information" sends the level and nothing else,
    /// which asks about the account the session signed on as.
    #[test]
    fn answers_a_query_without_names_with_the_signed_on_account() {
        let mut sentry = sentry();

        let response = sentry.process(&query(QUERY_USER, &[])).unwrap();

        let records = records(&response[1..]).unwrap();
        assert_eq!(response[0], COMMAND_REPLY);
        assert_eq!(
            record(&records, property::COMPANY),
            Some(b"GRiD".as_slice())
        );
        assert_eq!(
            record(&records, property::GROUP),
            Some(b"Systems".as_slice())
        );
        assert_eq!(
            record(&records, property::USER),
            Some(b"MANAGER".as_slice())
        );
        assert_eq!(record(&records, TAG_AUTHORITY), Some([40, 0].as_slice()));
    }

    #[test]
    fn answers_a_query_for_each_level() {
        let mut sentry = sentry();

        let group = sentry
            .process(&query(
                QUERY_GROUP,
                &[
                    (property::COMPANY, b"GRiD"),
                    (property::GROUP, b"Demo"),
                    (property::USER, b"GUEST"),
                ],
            ))
            .unwrap();
        let group = records(&group[1..]).unwrap();
        assert_eq!(record(&group, property::GROUP), Some(b"Demo".as_slice()));
        assert_eq!(record(&group, property::USER), Some(b"Demo".as_slice()));

        let company = sentry
            .process(&query(QUERY_COMPANY, &[(property::COMPANY, b"grid")]))
            .unwrap();
        let company = records(&company[1..]).unwrap();
        assert_eq!(
            record(&company, property::COMPANY),
            Some(b"GRiD".as_slice())
        );
        assert_eq!(record(&company, property::USER), Some(b"GRiD".as_slice()));
    }

    #[test]
    fn reports_a_queried_record_that_is_not_defined() {
        let mut sentry = sentry();

        assert_eq!(
            sentry
                .process(&query(QUERY_COMPANY, &[(property::COMPANY, b"MISSING")]))
                .unwrap(),
            status_response(STATUS_COMPANY_NOT_DEFINED)
        );
        assert_eq!(
            sentry
                .process(&query(
                    QUERY_USER,
                    &[
                        (property::COMPANY, b"GRiD"),
                        (property::GROUP, b"Demo"),
                        (property::USER, b"NOBODY"),
                    ],
                ))
                .unwrap(),
            status_response(STATUS_ACCOUNT_NOT_DEFINED)
        );
    }

    /// The query result feeds the same parser as a listing row, so it has to
    /// carry the same eleven records.
    #[test]
    fn a_query_answers_with_the_records_of_a_listing_row() {
        let mut sentry = sentry();

        let queried = sentry.process(&query(QUERY_USER, &[])).unwrap();
        let listed = sentry
            .process(&list_first(b"GRiD", b"Systems", b"Systems"))
            .unwrap();

        let tags = |response: &[u8]| {
            records(&response[1..])
                .unwrap()
                .iter()
                .map(|entry| entry.tag)
                .collect::<Vec<_>>()
        };
        assert_eq!(tags(&queried), tags(&listed));
    }

    /// Update and delete address a record by the cursor of a preceding query,
    /// so a test has to walk the same two steps the client does.
    fn cursor_of(sentry: &mut SentryServer, level: u8, names: &[(u8, &[u8])]) -> Vec<u8> {
        let response = sentry.process(&query(level, names)).unwrap();
        assert_eq!(response[0], COMMAND_REPLY, "the query should find the row");
        let records = records(&response[1..]).unwrap();
        record(&records, TAG_CURSOR).unwrap().to_vec()
    }

    fn update(cursor: &[u8], authority: Authority, quota: u32) -> Vec<u8> {
        let mut request = vec![COMMAND_UPDATE];
        request.extend(tagged(TAG_CURSOR, cursor));
        request.extend(tagged(TAG_AUTHORITY, &authority.0.to_be_bytes()));
        request.extend(tagged(TAG_QUOTA, &quota.to_le_bytes()));
        request
    }

    fn delete(cursor: &[u8]) -> Vec<u8> {
        let mut request = vec![COMMAND_DELETE];
        request.extend(tagged(TAG_CURSOR, cursor));
        request
    }

    fn change_password(names: &[(u8, &[u8])], password: &[u8]) -> Vec<u8> {
        let mut request = vec![COMMAND_CHANGE_PASSWORD];
        for (tag, name) in names {
            request.extend(tagged(*tag, name));
        }
        request.extend(tagged(property::PASSWORD, password));
        request
    }

    #[test]
    fn updates_the_record_the_cursor_addresses() {
        let mut sentry = sentry();
        let cursor = cursor_of(
            &mut sentry,
            QUERY_USER,
            &[
                (property::COMPANY, b"GRiD"),
                (property::GROUP, b"Demo"),
                (property::USER, b"GUEST"),
            ],
        );

        let response = sentry
            .process(&update(&cursor, Authority::GROUP_ADMIN, MEGABYTE))
            .unwrap();

        assert_eq!(response, [COMMAND_ACK]);
        let account = db::find_user(&sentry.conn, "GRiD", "Demo", "GUEST")
            .unwrap()
            .unwrap();
        assert_eq!(account.authority, Authority::GROUP_ADMIN.0);
        assert_eq!(account.quota, MEGABYTE);
    }

    /// A company and a group carry no authority of their own, so the update
    /// reaches nothing but the quota.
    #[test]
    fn updates_the_quota_of_a_group() {
        let mut sentry = sentry();
        let cursor = cursor_of(
            &mut sentry,
            QUERY_GROUP,
            &[(property::COMPANY, b"GRiD"), (property::GROUP, b"Demo")],
        );

        assert_eq!(
            sentry
                .process(&update(&cursor, Authority::GROUP_ADMIN, MEGABYTE))
                .unwrap(),
            [COMMAND_ACK]
        );

        let directory = sentry.directory().unwrap();
        let group = directory.find(&[b"GRiD", b"Demo", b"Demo"]).unwrap();
        assert_eq!(directory.get(group).unwrap().quota, MEGABYTE);
    }

    #[test]
    fn deletes_the_record_the_cursor_addresses() {
        let mut sentry = sentry();
        let cursor = cursor_of(
            &mut sentry,
            QUERY_USER,
            &[
                (property::COMPANY, b"GRiD"),
                (property::GROUP, b"Demo"),
                (property::USER, b"GUEST"),
            ],
        );

        assert_eq!(sentry.process(&delete(&cursor)).unwrap(), [COMMAND_ACK]);

        assert!(
            db::find_user(&sentry.conn, "GRiD", "Demo", "GUEST")
                .unwrap()
                .is_none()
        );
    }

    /// A group is deleted with everything under it, so the users it held do not
    /// outlive the group they belong to.
    #[test]
    fn deleting_a_group_takes_its_users_with_it() {
        let mut sentry = sentry();
        let cursor = cursor_of(
            &mut sentry,
            QUERY_GROUP,
            &[(property::COMPANY, b"GRiD"), (property::GROUP, b"Demo")],
        );

        assert_eq!(sentry.process(&delete(&cursor)).unwrap(), [COMMAND_ACK]);

        let rows = names(&mut sentry);
        assert!(rows.iter().all(|(_, group, _)| group != b"Demo"));
    }

    /// The session holds the account it signed on as, so deleting that account
    /// would leave the session pointing at nothing.
    #[test]
    fn refuses_to_delete_the_signed_on_account() {
        let mut sentry = sentry();
        let cursor = cursor_of(&mut sentry, QUERY_USER, &[]);

        assert_eq!(
            sentry.process(&delete(&cursor)).unwrap(),
            status_response(STATUS_INSUFFICIENT_AUTHORITY)
        );
        assert!(
            db::find_user(&sentry.conn, "GRiD", "Systems", "MANAGER")
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn refuses_an_update_or_delete_without_a_valid_cursor() {
        let mut sentry = sentry();

        assert_eq!(
            sentry
                .process(&update(b"99", Authority::NORMAL, MEGABYTE))
                .unwrap(),
            status_response(STATUS_ACCOUNT_NOT_DEFINED)
        );
        assert_eq!(
            sentry.process(&[COMMAND_DELETE]).unwrap(),
            status_response(STATUS_ACCOUNT_NOT_DEFINED)
        );
    }

    /// The group row above the users is out of reach for want of the level,
    /// even though it is on the administrator's own path.
    #[test]
    fn a_group_administrator_may_edit_only_its_own_group() {
        let mut admin = sentry_as(Authority::GROUP_ADMIN);
        let guest = cursor_of(
            &mut admin,
            QUERY_USER,
            &[
                (property::COMPANY, b"GRiD"),
                (property::GROUP, b"Demo"),
                (property::USER, b"GUEST"),
            ],
        );

        assert_eq!(
            admin
                .process(&update(&guest, Authority::NORMAL, MEGABYTE))
                .unwrap(),
            [COMMAND_ACK]
        );

        let group = cursor_of(
            &mut admin,
            QUERY_GROUP,
            &[(property::COMPANY, b"GRiD"), (property::GROUP, b"Demo")],
        );
        assert_eq!(
            admin
                .process(&update(&group, Authority::NORMAL, MEGABYTE))
                .unwrap(),
            status_response(STATUS_INSUFFICIENT_AUTHORITY)
        );
    }

    /// The scope check is what keeps an administrator inside its own subtree:
    /// the level alone would let a group administrator reach a sibling group.
    #[test]
    fn refuses_to_read_or_write_outside_the_own_subtree() {
        let mut sentry = sentry();
        add_group(&mut sentry, b"GRiD", b"Demo Group");
        add_user(
            &mut sentry,
            b"GRiD",
            b"Demo Group",
            b"Lenin",
            b"SECRET",
            Authority::GROUP_ADMIN,
        );

        let actor = db::find_user(&sentry.conn, "GRiD", "Demo Group", "Lenin")
            .unwrap()
            .unwrap();
        let mut admin = SentryServer::new(sentry.conn.clone(), actor);

        assert_eq!(
            admin
                .process(&query(
                    QUERY_USER,
                    &[
                        (property::COMPANY, b"GRiD"),
                        (property::GROUP, b"Demo"),
                        (property::USER, b"GUEST"),
                    ],
                ))
                .unwrap(),
            status_response(STATUS_INSUFFICIENT_AUTHORITY)
        );
    }

    /// A normal user may look at its own record and at nothing else — not even
    /// the company and group rows it sits under, which carry a quota and a disk
    /// usage of their own. Its own row names them anyway.
    #[test]
    fn a_normal_user_may_query_only_its_own_record() {
        let mut sentry = sentry_as(Authority::NORMAL);

        let response = sentry.process(&query(QUERY_USER, &[])).unwrap();
        assert_eq!(response[0], COMMAND_REPLY);
        let records = records(&response[1..]).unwrap();
        assert_eq!(record(&records, property::COMPANY).unwrap(), b"GRiD");
        assert_eq!(record(&records, property::GROUP).unwrap(), b"Demo");

        assert_eq!(
            sentry
                .process(&query(QUERY_COMPANY, &[(property::COMPANY, b"GRiD")]))
                .unwrap(),
            status_response(STATUS_INSUFFICIENT_AUTHORITY)
        );
        assert_eq!(
            sentry
                .process(&query(
                    QUERY_GROUP,
                    &[(property::COMPANY, b"GRiD"), (property::GROUP, b"Demo")],
                ))
                .unwrap(),
            status_response(STATUS_INSUFFICIENT_AUTHORITY)
        );
        assert_eq!(
            sentry
                .process(&query(
                    QUERY_USER,
                    &[
                        (property::COMPANY, b"GRiD"),
                        (property::GROUP, b"Systems"),
                        (property::USER, b"MANAGER"),
                    ],
                ))
                .unwrap(),
            status_response(STATUS_INSUFFICIENT_AUTHORITY)
        );
    }

    /// The listing is how AdministratorSentry fills its account browser, and it
    /// hands out every row whole — authority, quota, disk usage. An account with
    /// nothing to administer has no business walking it.
    #[test]
    fn refuses_the_listing_to_an_account_that_administers_nothing() {
        let mut sentry = sentry_as(Authority::NORMAL);

        assert_eq!(
            sentry.process(&list_first(b"", b"", b"")).unwrap(),
            status_response(STATUS_INSUFFICIENT_AUTHORITY)
        );
        assert_eq!(
            sentry.process(&[COMMAND_LIST_NEXT]).unwrap(),
            status_response(STATUS_INSUFFICIENT_AUTHORITY)
        );
    }

    /// Walking from the very start is the client's own first step, so the skip
    /// has to happen inside the walk rather than by refusing the request.
    #[test]
    fn the_listing_shows_an_administrator_only_its_own_subtree() {
        let mut sentry = sentry();
        add_company(&mut sentry, b"Test Company");
        add_group(&mut sentry, b"Test Company", b"Demo Group");
        add_user(
            &mut sentry,
            b"Test Company",
            b"Demo Group",
            b"Lenin",
            b"SECRET",
            Authority::GROUP_ADMIN,
        );

        let actor = db::find_user(&sentry.conn, "Test Company", "Demo Group", "Lenin")
            .unwrap()
            .unwrap();
        let mut admin = SentryServer::new(sentry.conn.clone(), actor);

        // The company and the group the actor hangs from stay in the listing —
        // they are its own path, and the browser needs them to draw the tree —
        // while the demo company beside them is gone entirely.
        let rows = names(&mut admin);
        assert!(
            rows.iter().all(|(company, ..)| company == b"Test Company"),
            "the listing leaked rows outside the company: {rows:?}"
        );
        assert!(
            rows.iter()
                .all(|(_, group, _)| group == b"Test Company" || group == b"Demo Group"),
            "the listing leaked rows outside the group: {rows:?}"
        );
        assert!(rows.iter().any(|(.., user)| user == b"Lenin"));
    }

    #[test]
    fn refuses_an_update_that_grants_more_than_the_actor_holds() {
        let mut admin = sentry_as(Authority::GROUP_ADMIN);
        let cursor = cursor_of(
            &mut admin,
            QUERY_USER,
            &[
                (property::COMPANY, b"GRiD"),
                (property::GROUP, b"Demo"),
                (property::USER, b"GUEST"),
            ],
        );

        assert_eq!(
            admin
                .process(&update(&cursor, Authority::SYSTEM_ADMIN, MEGABYTE))
                .unwrap(),
            status_response(STATUS_INSUFFICIENT_AUTHORITY)
        );
    }

    /// UserSentry sends no names at all, meaning the account of the session.
    #[test]
    fn every_account_may_change_its_own_password() {
        let mut sentry = sentry_as(Authority::NORMAL);

        assert_eq!(
            sentry.process(&change_password(&[], b"NEW")).unwrap(),
            status_response(status::OK)
        );

        let account = db::find_user(&sentry.conn, "GRiD", "Demo", "GUEST")
            .unwrap()
            .unwrap();
        assert_eq!(account.password, "NEW");
    }

    #[test]
    fn an_administrator_may_change_the_password_of_another_account() {
        let mut sentry = sentry();

        assert_eq!(
            sentry
                .process(&change_password(
                    &[
                        (property::COMPANY, b"GRiD"),
                        (property::GROUP, b"Demo"),
                        (property::USER, b"GUEST"),
                    ],
                    b"NEW",
                ))
                .unwrap(),
            status_response(status::OK)
        );

        let account = db::find_user(&sentry.conn, "GRiD", "Demo", "GUEST")
            .unwrap()
            .unwrap();
        assert_eq!(account.password, "NEW");
    }

    #[test]
    fn refuses_to_change_the_password_of_another_account_without_the_authority() {
        let mut sentry = sentry_as(Authority::NORMAL);

        assert_eq!(
            sentry
                .process(&change_password(
                    &[
                        (property::COMPANY, b"GRiD"),
                        (property::GROUP, b"Systems"),
                        (property::USER, b"MANAGER"),
                    ],
                    b"NEW",
                ))
                .unwrap(),
            status_response(STATUS_INSUFFICIENT_AUTHORITY)
        );

        let account = db::find_user(&sentry.conn, "GRiD", "Systems", "MANAGER")
            .unwrap()
            .unwrap();
        assert_eq!(account.password, "MANAGER");
    }

    #[test]
    fn reports_a_password_change_for_an_account_that_is_not_defined() {
        let mut sentry = sentry();

        assert_eq!(
            sentry
                .process(&change_password(
                    &[
                        (property::COMPANY, b"GRiD"),
                        (property::GROUP, b"Demo"),
                        (property::USER, b"NOBODY"),
                    ],
                    b"NEW",
                ))
                .unwrap(),
            status_response(STATUS_ACCOUNT_NOT_DEFINED)
        );
    }

    #[test]
    fn refuses_a_password_change_with_an_empty_password() {
        let mut sentry = sentry();

        assert_eq!(
            sentry.process(&change_password(&[], b"")).unwrap(),
            status_response(status::PROPERTY_MISSING)
        );
    }

    /// The password the Sentry stores is the one sign-on then compares against,
    /// so a change has to carry through to the next sign-on.
    #[test]
    fn a_changed_password_is_the_one_sign_on_accepts() {
        let conn = Rc::new(db::open_in_memory());
        let actor = signed_on(&conn, Authority::SYSTEM_ADMIN);
        let mut sentry = SentryServer::new(conn.clone(), actor);

        sentry
            .process(&change_password(
                &[
                    (property::COMPANY, b"GRiD"),
                    (property::GROUP, b"Demo"),
                    (property::USER, b"GUEST"),
                ],
                b"NEW",
            ))
            .unwrap();

        assert!(
            crate::authenticate(
                &conn,
                &crate::sign_on_properties(b"GRiD", b"Demo", b"GUEST", b"GUEST"),
            )
            .is_err()
        );
        assert!(
            crate::authenticate(
                &conn,
                &crate::sign_on_properties(b"GRiD", b"Demo", b"GUEST", b"NEW"),
            )
            .is_ok()
        );
    }

    /// Every mandatory record must fit the destination the client allocates,
    /// which silently truncates anything longer.
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
