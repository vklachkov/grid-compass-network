use rusqlite::{Connection, OptionalExtension, params};
use rusqlite_migration::M;

/// Mail bodies are stored as they arrived, in the clear: the GRiD client sends
/// no encryption of its own, and a readable mailbox is what makes the
/// reimplementation inspectable.
///
/// Correspondents are rows of `users` rather than the names the client wrote, so
/// a mailbox cannot name an account that does not exist and a rename carries
/// through to the mail already stored.
///
/// `mail_id` is the six byte key the client addresses a message by, of which
/// only the lower four bytes ever carry a value. It counts per mailbox, so two
/// recipients may hold the same id.
pub const MIGRATIONS: &[M<'static>] = &[M::up(
    r#"
CREATE TABLE messages (
    id              INTEGER PRIMARY KEY,
    sender_id       INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    recipient_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    mail_id         INTEGER NOT NULL,
    subject         TEXT    NOT NULL,
    body            TEXT    NOT NULL,
    attachment_path TEXT,
    is_read         INTEGER NOT NULL DEFAULT 0,
    UNIQUE (recipient_id, mail_id)
);
"#,
)];

pub struct Message {
    pub id: i64,
    pub mail_id: u32,
    pub sender: String,
    pub recipient: String,
    pub subject: String,
    pub body: String,
    pub attachment_path: Option<String>,
    pub is_read: bool,
}

pub struct NewMessage<'a> {
    pub sender_id: i64,
    pub recipient_id: i64,
    pub subject: &'a str,
    pub body: &'a str,
    /// Where the attachment lives, relative to the directory holding the
    /// database, so a mailbox stays valid when the server is moved.
    pub attachment_path: Option<&'a str>,
}

const SELECT: &str = r#"
SELECT m.id, m.mail_id, s.name, r.name, m.subject, m.body, m.attachment_path, m.is_read
  FROM messages m
  JOIN users s ON s.id = m.sender_id
  JOIN users r ON r.id = m.recipient_id
"#;

/// A message may only be addressed to an account the sender shares a group with:
/// nothing in the protocol carries a company or a group alongside the recipient,
/// so a bare name can only be resolved within the sender's own group.
pub fn find_recipient(
    conn: &Connection,
    sender_id: i64,
    recipient: &str,
) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT id FROM users
          WHERE name = ?2
            AND group_id = (SELECT group_id FROM users WHERE id = ?1)",
        params![sender_id, recipient],
        |row| row.get(0),
    )
    .optional()
}

/// The id is assigned by the same statement that inserts the row, so two
/// sessions writing to one mailbox cannot settle on the same one.
pub fn insert(conn: &Connection, message: &NewMessage<'_>) -> rusqlite::Result<u32> {
    conn.query_row(
        "INSERT INTO messages
             (sender_id, recipient_id, mail_id, subject, body, attachment_path)
         SELECT ?1, ?2,
                COALESCE((SELECT MAX(mail_id) FROM messages WHERE recipient_id = ?2), 0) + 1,
                ?3, ?4, ?5
         RETURNING mail_id",
        params![
            message.sender_id,
            message.recipient_id,
            message.subject,
            message.body,
            message.attachment_path,
        ],
        |row| row.get(0),
    )
}

/// The client walks a mailbox oldest first, which is the order the ids grew in.
pub fn list(conn: &Connection, recipient_id: i64) -> rusqlite::Result<Vec<Message>> {
    conn.prepare(&format!(
        "{SELECT} WHERE m.recipient_id = ?1 ORDER BY m.mail_id"
    ))?
    .query_map(params![recipient_id], read_message)?
    .collect()
}

pub fn find(
    conn: &Connection,
    recipient_id: i64,
    mail_id: u32,
) -> rusqlite::Result<Option<Message>> {
    conn.query_row(
        &format!("{SELECT} WHERE m.recipient_id = ?1 AND m.mail_id = ?2"),
        params![recipient_id, mail_id],
        read_message,
    )
    .optional()
}

pub fn first_unread(conn: &Connection, recipient_id: i64) -> rusqlite::Result<Option<Message>> {
    conn.query_row(
        &format!("{SELECT} WHERE m.recipient_id = ?1 AND m.is_read = 0 ORDER BY m.mail_id LIMIT 1"),
        params![recipient_id],
        read_message,
    )
    .optional()
}

pub fn mark_read(conn: &Connection, id: i64) -> rusqlite::Result<usize> {
    conn.execute("UPDATE messages SET is_read = 1 WHERE id = ?1", params![id])
}

fn read_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
    Ok(Message {
        id: row.get(0)?,
        mail_id: row.get(1)?,
        sender: row.get(2)?,
        recipient: row.get(3)?,
        subject: row.get(4)?,
        body: row.get(5)?,
        attachment_path: row.get(6)?,
        is_read: row.get(7)?,
    })
}
