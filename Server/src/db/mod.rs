pub mod mailbox;
pub mod users;

use anyhow::Context;
use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};

pub use users::*;

/// The schema is one migration chain shared by every table: each module
/// contributes its own steps, and the order here is the order they are applied,
/// so a module's steps may only depend on those of the modules ahead of it.
fn migrations() -> Vec<M<'static>> {
    [users::MIGRATIONS, mailbox::MIGRATIONS].concat()
}

/// WAL is what makes the frontend's own connection safe alongside the session
/// threads: readers do not block the writer and vice versa. `busy_timeout`
/// covers the remaining case of two writers meeting.
pub fn open(path: &str) -> anyhow::Result<Connection> {
    let mut conn = Connection::open(path).with_context(|| format!("open database {path}"))?;

    conn.pragma_update(None, "journal_mode", "WAL")
        .context("enable WAL")?;
    conn.pragma_update(None, "busy_timeout", 5000)
        .context("set busy timeout")?;
    conn.pragma_update(None, "foreign_keys", true)
        .context("enable foreign keys")?;

    migrate(&mut conn)?;

    Ok(conn)
}

pub fn migrate(conn: &mut Connection) -> anyhow::Result<()> {
    Migrations::new(migrations())
        .to_latest(conn)
        .context("apply database migrations")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_valid() {
        Migrations::new(migrations())
            .validate()
            .expect("migrations should be valid");
    }
}
