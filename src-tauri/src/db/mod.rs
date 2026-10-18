//! SQLite storage: connection setup, versioned migrations, and queries.

pub mod backup;
pub mod categories;
pub mod feed;
pub mod queue;
pub mod screenshots;
pub mod search;

use std::path::Path;

use rusqlite::{Connection, OpenFlags};

use crate::error::Result;

/// Migrations are append-only. Never edit a shipped migration; add a new one.
const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("../../migrations/0001_init.sql")),
    (2, include_str!("../../migrations/0002_soft_delete.sql")),
    (3, include_str!("../../migrations/0003_ai_results.sql")),
    (4, include_str!("../../migrations/0004_deadlines.sql")),
    (5, include_str!("../../migrations/0005_signature_colors.sql")),
    (6, include_str!("../../migrations/0006_multi_category.sql")),
    (7, include_str!("../../migrations/0007_text_locked.sql")),
];

/// Latest schema version this build knows about.
pub fn latest_version() -> i64 {
    MIGRATIONS.last().map_or(0, |(v, _)| *v)
}

/// Opens (creating if needed) the database at `path` and brings it to the latest schema.
pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    configure(&conn)?;
    migrate(&conn)?;
    Ok(conn)
}

/// In-memory database with the full schema, for tests.
#[cfg(test)]
pub fn open_in_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    configure(&conn)?;
    migrate(&conn)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> Result<()> {
    // WAL keeps UI reads from blocking on the background worker's writes.
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(())
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
             version    INTEGER PRIMARY KEY,
             applied_at INTEGER NOT NULL
         );",
    )?;
    let current = schema_version(conn)?;

    for (version, sql) in MIGRATIONS.iter().filter(|(v, _)| *v > current) {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        tx.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
            (version, crate::util::now_ms()),
        )?;
        tx.commit()?;
        tracing::info!(version, "applied migration");
    }
    Ok(())
}

/// Current schema version recorded in the database.
pub fn schema_version(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COALESCE(MAX(version), 0) FROM schema_migrations", [], |r| r.get(0))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_apply_and_are_idempotent() {
        let conn = open_in_memory().unwrap();
        assert_eq!(schema_version(&conn).unwrap(), latest_version());
        migrate(&conn).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), latest_version());
    }

    #[test]
    fn upgrading_an_existing_library_keeps_categories_and_tags() {
        // A library as it was before migration 6 (one category per screenshot).
        let conn = Connection::open_in_memory().unwrap();
        configure(&conn).unwrap();
        conn.execute_batch(
            "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at INTEGER NOT NULL);",
        )
        .unwrap();
        for (version, sql) in MIGRATIONS.iter().filter(|(v, _)| *v < 6) {
            conn.execute_batch(sql).unwrap();
            conn.execute("INSERT INTO schema_migrations VALUES (?1, 0)", [version]).unwrap();
        }
        conn.execute(
            "INSERT INTO screenshots (category_id, image_path, thumb_path, width, height, tags_json, created_at, updated_at)
             VALUES (2, 'i.png', 't.webp', 1, 1, '[\"Fonts\", \" design \", \"\"]', 0, 0)",
            [],
        )
        .unwrap();

        migrate(&conn).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), latest_version());
        assert_eq!(super::screenshots::category_ids(&conn, 1).unwrap(), [2]);
        let tags: Vec<String> = conn
            .prepare("SELECT tag FROM screenshot_tags ORDER BY tag")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(tags, ["design", "fonts"]);
        let card = super::screenshots::get(&conn, 1).unwrap();
        assert_eq!(card.categories.len(), 1);
        assert_eq!(card.categories[0].name, "Learning");
    }

    #[test]
    fn fts_stays_in_sync_with_screenshots() {
        let conn = open_in_memory().unwrap();
        conn.execute(
            "INSERT INTO screenshots (category_id, image_path, thumb_path, width, height, user_note, ocr_text,
                                      created_at, updated_at)
             VALUES (1, 'a.png', 'a.webp', 10, 10, 'register for hackathon', 'Deadline 12/11', 0, 0)",
            [],
        )
        .unwrap();
        let hits = |q: &str| -> i64 {
            conn.query_row("SELECT count(*) FROM screenshots_fts WHERE screenshots_fts MATCH ?1", [q], |r| r.get(0))
                .unwrap()
        };
        assert_eq!(hits("hack*"), 1);
        assert_eq!(hits("deadline"), 1);

        conn.execute("UPDATE screenshots SET user_note = 'watch the talk' WHERE id = 1", []).unwrap();
        assert_eq!(hits("hack*"), 0);
        assert_eq!(hits("talk"), 1);

        conn.execute("DELETE FROM screenshots WHERE id = 1", []).unwrap();
        assert_eq!(hits("talk"), 0);
    }

    #[test]
    fn category_with_screenshots_cannot_be_hard_deleted() {
        // The app must move items to Miscellaneous first; the FK guards against silent loss.
        let conn = open_in_memory().unwrap();
        conn.execute(
            "INSERT INTO screenshots (category_id, image_path, thumb_path, width, height, created_at, updated_at)
             VALUES (1, 'a.png', 'a.webp', 10, 10, 0, 0)",
            [],
        )
        .unwrap();
        assert!(conn.execute("DELETE FROM categories WHERE id = 1", []).is_err());
    }
}
