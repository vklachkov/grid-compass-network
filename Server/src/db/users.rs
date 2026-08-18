use rusqlite::{Connection, OptionalExtension, params};
use rusqlite_migration::M;

/// The wire form has no level field — the client tells a company from a group
/// from a user by which of the three names repeat — so the level is
/// reconstructed when rows are flattened for the listing.
pub const LEVEL_COMPANY: i64 = 0;
/// A group row is the one level the server never has to recognise by number:
/// it is whatever is neither a company nor a user.
#[cfg(test)]
pub const LEVEL_GROUP: i64 = 1;
pub const LEVEL_USER: i64 = 2;

/// `COLLATE NOCASE` is what makes the client's case insensitive view of names
/// the database's own: it governs the unique indexes, the lookups and the
/// listing order alike. It applies to `TEXT` only, which is why names are not
/// `BLOB`.
pub const MIGRATIONS: &[M<'static>] = &[M::up(
    r#"
CREATE TABLE companies (
    id    INTEGER PRIMARY KEY,
    name  TEXT    NOT NULL COLLATE NOCASE UNIQUE,
    quota INTEGER NOT NULL,
    used  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE groups (
    id         INTEGER PRIMARY KEY,
    company_id INTEGER NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    name       TEXT    NOT NULL COLLATE NOCASE UNIQUE,
    quota      INTEGER NOT NULL,
    used       INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE users (
    id        INTEGER PRIMARY KEY,
    group_id  INTEGER NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    name      TEXT    NOT NULL COLLATE NOCASE,
    password  TEXT    NOT NULL,
    authority INTEGER NOT NULL,
    quota     INTEGER NOT NULL,
    used      INTEGER NOT NULL DEFAULT 0,
    UNIQUE (group_id, name)
);

INSERT INTO companies (id, name, quota) VALUES (1, 'GRiD Systems', 4294967295);
INSERT INTO groups (id, company_id, name, quota) VALUES (1, 1, 'Default Group', 4294967295);
INSERT INTO users (group_id, name, password, authority, quota)
    VALUES (1, 'Sysuser', 'Sysuser', 40, 4294967295);
"#,
)];

/// One row of the flattened directory. The levels that do not apply are empty,
/// unlike the wire form, where a company repeats its name at all three.
pub struct Account {
    pub level: i64,
    pub company: String,
    pub group: String,
    pub user: String,
    pub password: String,
    pub authority: u16,
    pub quota: u32,
    pub used: u32,
}

/// The order is the one the client walks: a company, then each of its groups
/// followed by that group's users. Sorting by the level within a group is what
/// keeps the group's own row ahead of its users.
///
/// Only a user carries an authority of its own — the client sends none when it
/// creates a company or a group — so the two upper levels report the fixed
/// authority their level implies: 30 company administrator, 20 group.
const LOAD_SQL: &str = r#"
SELECT level, company, grp, usr, password, authority, quota, used FROM (
    SELECT 0 AS level, c.name AS company, '' AS grp, '' AS usr, '' AS password,
           30 AS authority, c.quota, c.used
      FROM companies c
    UNION ALL
    SELECT 1, c.name, g.name, '', '', 20, g.quota, g.used
      FROM groups g JOIN companies c ON c.id = g.company_id
    UNION ALL
    SELECT 2, c.name, g.name, u.name, u.password, u.authority, u.quota, u.used
      FROM users u
      JOIN groups g ON g.id = u.group_id
      JOIN companies c ON c.id = g.company_id
)
ORDER BY company COLLATE NOCASE, grp COLLATE NOCASE, level, usr COLLATE NOCASE
"#;

/// The same walk as `LOAD_SQL`, ordered by the ids instead of the names: the
/// wire order is the client's business, but a reader wants the directory in the
/// order it grew.
const LOAD_BY_AGE_SQL: &str = r#"
SELECT level, company, grp, usr, password, authority, quota, used FROM (
    SELECT 0 AS level, c.name AS company, '' AS grp, '' AS usr, '' AS password,
           30 AS authority, c.quota, c.used,
           c.id AS company_id, 0 AS group_id, 0 AS user_id
      FROM companies c
    UNION ALL
    SELECT 1, c.name, g.name, '', '', 20, g.quota, g.used, c.id, g.id, 0
      FROM groups g JOIN companies c ON c.id = g.company_id
    UNION ALL
    SELECT 2, c.name, g.name, u.name, u.password, u.authority, u.quota, u.used,
           c.id, g.id, u.id
      FROM users u
      JOIN groups g ON g.id = u.group_id
      JOIN companies c ON c.id = g.company_id
)
ORDER BY company_id, group_id, level, user_id
"#;

pub fn load(conn: &Connection) -> rusqlite::Result<Vec<Account>> {
    query_accounts(conn, LOAD_SQL)
}

pub fn load_by_age(conn: &Connection) -> rusqlite::Result<Vec<Account>> {
    query_accounts(conn, LOAD_BY_AGE_SQL)
}

fn query_accounts(conn: &Connection, sql: &str) -> rusqlite::Result<Vec<Account>> {
    conn.prepare(sql)?
        .query_map([], |row| {
            Ok(Account {
                level: row.get(0)?,
                company: row.get(1)?,
                group: row.get(2)?,
                user: row.get(3)?,
                password: row.get(4)?,
                authority: row.get(5)?,
                quota: row.get(6)?,
                used: row.get(7)?,
            })
        })?
        .collect()
}

pub fn find_user(
    conn: &Connection,
    company: &str,
    group: &str,
    user: &str,
) -> rusqlite::Result<Option<Account>> {
    conn.query_row(
        "SELECT c.name, g.name, u.name, u.password, u.authority, u.quota, u.used
           FROM users u
           JOIN groups g ON g.id = u.group_id
           JOIN companies c ON c.id = g.company_id
          WHERE c.name = ?1 AND g.name = ?2 AND u.name = ?3",
        params![company, group, user],
        |row| {
            Ok(Account {
                level: LEVEL_USER,
                company: row.get(0)?,
                group: row.get(1)?,
                user: row.get(2)?,
                password: row.get(3)?,
                authority: row.get(4)?,
                quota: row.get(5)?,
                used: row.get(6)?,
            })
        },
    )
    .optional()
}

pub fn find_company(conn: &Connection, company: &str) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT id FROM companies WHERE name = ?1",
        params![company],
        |row| row.get(0),
    )
    .optional()
}

pub fn find_group(conn: &Connection, company: &str, group: &str) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT g.id
           FROM groups g JOIN companies c ON c.id = g.company_id
          WHERE c.name = ?1 AND g.name = ?2",
        params![company, group],
        |row| row.get(0),
    )
    .optional()
}

/// Groups and users are addressed by the whole name chain rather than by their
/// own name alone, so the statements below reach them through a subquery instead
/// of asking the caller to resolve an id first.
const GROUP_ID: &str = "SELECT g.id
      FROM groups g JOIN companies c ON c.id = g.company_id
      WHERE c.name = ?1 AND g.name = ?2";

const USER_ID: &str = "SELECT u.id
      FROM users u
      JOIN groups g ON g.id = u.group_id
      JOIN companies c ON c.id = g.company_id
      WHERE c.name = ?1 AND g.name = ?2 AND u.name = ?3";

pub fn insert_company(conn: &Connection, name: &str, quota: u32) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO companies (name, quota) VALUES (?1, ?2)",
        params![name, quota],
    )?;

    Ok(())
}

pub fn insert_group(
    conn: &Connection,
    company_id: i64,
    name: &str,
    quota: u32,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO groups (company_id, name, quota) VALUES (?1, ?2, ?3)",
        params![company_id, name, quota],
    )?;

    Ok(())
}

pub fn insert_user(
    conn: &Connection,
    group_id: i64,
    name: &str,
    password: &str,
    authority: u16,
    quota: u32,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO users (group_id, name, password, authority, quota)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![group_id, name, password, authority, quota],
    )?;

    Ok(())
}

pub fn update_company(conn: &Connection, company: &str, quota: u32) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE companies SET quota = ?2 WHERE name = ?1",
        params![company, quota],
    )
}

pub fn update_group(
    conn: &Connection,
    company: &str,
    group: &str,
    quota: u32,
) -> rusqlite::Result<usize> {
    conn.execute(
        &format!("UPDATE groups SET quota = ?3 WHERE id = ({GROUP_ID})"),
        params![company, group, quota],
    )
}

pub fn update_user(
    conn: &Connection,
    company: &str,
    group: &str,
    user: &str,
    authority: u16,
    quota: u32,
) -> rusqlite::Result<usize> {
    conn.execute(
        &format!("UPDATE users SET authority = ?4, quota = ?5 WHERE id = ({USER_ID})"),
        params![company, group, user, authority, quota],
    )
}

pub fn set_password(
    conn: &Connection,
    company: &str,
    group: &str,
    user: &str,
    password: &str,
) -> rusqlite::Result<usize> {
    conn.execute(
        &format!("UPDATE users SET password = ?4 WHERE id = ({USER_ID})"),
        params![company, group, user, password],
    )
}

/// The cascade is spelled out instead of left to `ON DELETE CASCADE`: the pragma
/// enabling it is set per connection, and a directory that half deletes itself
/// because one connection forgot the pragma is worse than a redundant statement.
pub fn delete_company(conn: &Connection, company: &str) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM users WHERE group_id IN
             (SELECT g.id FROM groups g JOIN companies c ON c.id = g.company_id
               WHERE c.name = ?1)",
        params![company],
    )?;
    conn.execute(
        "DELETE FROM groups WHERE company_id IN (SELECT id FROM companies WHERE name = ?1)",
        params![company],
    )?;
    conn.execute("DELETE FROM companies WHERE name = ?1", params![company])
}

pub fn delete_group(conn: &Connection, company: &str, group: &str) -> rusqlite::Result<usize> {
    conn.execute(
        &format!("DELETE FROM users WHERE group_id = ({GROUP_ID})"),
        params![company, group],
    )?;
    conn.execute(
        &format!("DELETE FROM groups WHERE id = ({GROUP_ID})"),
        params![company, group],
    )
}

pub fn delete_user(
    conn: &Connection,
    company: &str,
    group: &str,
    user: &str,
) -> rusqlite::Result<usize> {
    conn.execute(
        &format!("DELETE FROM users WHERE id = ({USER_ID})"),
        params![company, group, user],
    )
}

/// The seeded administrator alone cannot exercise the listing walk, which needs
/// a company holding more than one group and a group holding more than one user
/// — so the tests replace the seed with a directory that has both.
#[cfg(test)]
pub fn open_in_memory() -> Connection {
    let mut conn = Connection::open_in_memory().expect("open in-memory database");
    super::migrate(&mut conn).expect("migrate in-memory database");

    conn.execute_batch(
        r#"
DELETE FROM users;
DELETE FROM groups;
DELETE FROM companies;

INSERT INTO companies (id, name, quota) VALUES (1, 'GRiD', 4294967295);
INSERT INTO groups (id, company_id, name, quota) VALUES
    (1, 1, 'Demo', 4294967295),
    (2, 1, 'Systems', 4294967295);
INSERT INTO users (group_id, name, password, authority, quota, used) VALUES
    (1, 'GUEST', 'GUEST', 0, 1048576, 262144),
    (1, 'OPERATOR', 'OPERATOR', 20, 4294967295, 1048576),
    (2, 'MANAGER', 'MANAGER', 40, 4294967295, 4194304);
"#,
    )
    .expect("seed the test directory");

    conn
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh database has to be one an administrator can sign on to: every
    /// other account is created through the Sentry, which refuses every command
    /// below `SYSTEM_ADMIN`.
    #[test]
    fn a_fresh_database_holds_only_the_seeded_administrator() {
        let mut conn = Connection::open_in_memory().unwrap();
        super::super::migrate(&mut conn).unwrap();

        let rows: Vec<_> = load(&conn)
            .unwrap()
            .into_iter()
            .map(|account| (account.level, account.company, account.group, account.user))
            .collect();

        assert_eq!(
            rows,
            [
                (
                    LEVEL_COMPANY,
                    "GRiD Systems".to_owned(),
                    String::new(),
                    String::new()
                ),
                (
                    LEVEL_GROUP,
                    "GRiD Systems".into(),
                    "Default Group".into(),
                    String::new()
                ),
                (
                    LEVEL_USER,
                    "GRiD Systems".into(),
                    "Default Group".into(),
                    "Sysuser".into()
                ),
            ]
        );

        let sysuser = find_user(&conn, "GRiD Systems", "Default Group", "Sysuser")
            .unwrap()
            .expect("the seeded administrator should be found");
        assert_eq!(sysuser.password, "Sysuser");
        assert_eq!(sysuser.authority, 40);
    }

    #[test]
    fn loads_the_demo_directory_in_client_order() {
        let conn = open_in_memory();

        let names: Vec<_> = load(&conn)
            .unwrap()
            .into_iter()
            .map(|account| (account.level, account.group, account.user))
            .collect();

        assert_eq!(
            names,
            [
                (LEVEL_COMPANY, String::new(), String::new()),
                (LEVEL_GROUP, "Demo".into(), String::new()),
                (LEVEL_USER, "Demo".into(), "GUEST".into()),
                (LEVEL_USER, "Demo".into(), "OPERATOR".into()),
                (LEVEL_GROUP, "Systems".into(), String::new()),
                (LEVEL_USER, "Systems".into(), "MANAGER".into()),
            ]
        );
    }

    #[test]
    fn finds_a_user_ignoring_case() {
        let conn = open_in_memory();

        let account = find_user(&conn, "grid", "DEMO", "guest")
            .unwrap()
            .expect("GUEST should be found whatever the case");

        assert_eq!(account.password, "GUEST");
        assert_eq!(account.authority, 0);
    }

    #[test]
    fn does_not_find_an_unknown_user() {
        let conn = open_in_memory();

        assert!(
            find_user(&conn, "GRiD", "Demo", "NOBODY")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn inserted_rows_appear_in_order() {
        let conn = open_in_memory();
        let group = find_group(&conn, "grid", "demo").unwrap().unwrap();

        insert_user(&conn, group, "BOB", "PW", 0, 1024).unwrap();

        let users: Vec<_> = load(&conn)
            .unwrap()
            .into_iter()
            .filter(|account| account.group == "Demo" && account.level == LEVEL_USER)
            .map(|account| account.user)
            .collect();

        assert_eq!(users, ["BOB", "GUEST", "OPERATOR"]);
    }

    #[test]
    fn refuses_a_duplicate_whatever_the_case() {
        let conn = open_in_memory();

        assert!(insert_company(&conn, "grid", 1024).is_err());
        assert!(insert_group(&conn, 1, "DEMO", 1024).is_err());
        assert!(insert_user(&conn, 1, "guest", "PW", 0, 1024).is_err());
    }
}
