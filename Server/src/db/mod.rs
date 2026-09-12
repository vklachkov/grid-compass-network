pub mod mailbox;
pub mod users;

use std::sync::Arc;

use anyhow::Context;
use parking_lot::{Mutex, MutexGuard};
use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};

pub use users::*;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &str) -> anyhow::Result<Arc<Self>> {
        let conn = Connection::open(path).with_context(|| format!("open database {path}"))?;
        Self::new(conn)
    }

    #[cfg(test)]
    pub fn in_memory() -> anyhow::Result<Arc<Self>> {
        let conn = Connection::open_in_memory().context("open database in memory")?;
        Self::new(conn)
    }

    fn new(mut conn: Connection) -> anyhow::Result<Arc<Self>> {
        conn.pragma_update(None, "journal_mode", "WAL")
            .context("enable WAL")?;
        conn.pragma_update(None, "busy_timeout", 5000)
            .context("set busy timeout")?;
        conn.pragma_update(None, "foreign_keys", true)
            .context("enable foreign keys")?;

        Self::migrate(&mut conn)?;

        Ok(Arc::new(Self {
            conn: Mutex::new(conn),
        }))
    }

    fn migrate(conn: &mut Connection) -> anyhow::Result<()> {
        Migrations::new(Self::migrations())
            .to_latest(conn)
            .context("apply database migrations")
    }

    pub fn migrations() -> Vec<M<'static>> {
        [users::MIGRATIONS, mailbox::MIGRATIONS].concat()
    }

    pub fn get_conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock()
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Arc<Self> {
        Self::in_memory().expect("open and migrate in-memory database")
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn migrations_are_valid() {
        Migrations::new(Database::migrations())
            .validate()
            .expect("migrations should be valid");
    }
}
