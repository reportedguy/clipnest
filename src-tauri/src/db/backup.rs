//! Keeping the library safe: a daily snapshot of the database, and automatic recovery if
//! it's ever unreadable (spec 3.7). Images live as separate files and are never touched.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::error::Result;

const KEEP: usize = 3;

fn backup_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("backups")
}

fn healthy(conn: &Connection) -> bool {
    conn.query_row("PRAGMA quick_check(1)", [], |r| r.get::<_, String>(0)).is_ok_and(|s| s == "ok")
}

/// Opens the library, recovering if it's damaged: the broken file is set aside (never
/// deleted) and the newest healthy backup is restored, or a fresh library is started.
/// Returns the connection and, if recovery happened, a message for the user.
pub fn open_with_recovery(data_dir: &Path) -> Result<(Connection, Option<String>)> {
    let path = data_dir.join("library.db");
    if let Ok(conn) = super::open(&path) {
        if healthy(&conn) {
            return Ok((conn, None));
        }
    }
    tracing::error!(path = %path.display(), "library database is damaged; recovering");

    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let kept = data_dir.join(format!("library.db.damaged-{stamp}"));
    let _ = fs::rename(&path, &kept);
    for ext in ["-wal", "-shm"] {
        let mut side = path.as_os_str().to_owned();
        side.push(ext);
        let _ = fs::remove_file(PathBuf::from(side));
    }

    for backup in backups(data_dir) {
        if fs::copy(&backup, &path).is_ok() {
            if let Ok(conn) = super::open(&path) {
                if healthy(&conn) {
                    let name = backup.file_stem().and_then(|s| s.to_str()).unwrap_or("a backup").to_owned();
                    tracing::warn!(backup = %name, "restored library from backup");
                    return Ok((
                        conn,
                        Some(format!(
                            "Your library couldn't be read, so it was restored from {}. The damaged file was kept as {}.",
                            name.replace("library-", "the backup of "),
                            kept.file_name().and_then(|s| s.to_str()).unwrap_or_default()
                        )),
                    ));
                }
            }
            let _ = fs::remove_file(&path);
        }
    }

    let conn = super::open(&path)?;
    Ok((
        conn,
        Some(format!(
            "Your library couldn't be read and no backup was usable, so a new one was started. Your images are still in the images folder, and the damaged file was kept as {}.",
            kept.file_name().and_then(|s| s.to_str()).unwrap_or_default()
        )),
    ))
}

/// Backups, newest first.
fn backups(data_dir: &Path) -> Vec<PathBuf> {
    let mut all: Vec<PathBuf> = fs::read_dir(backup_dir(data_dir))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "db"))
        .collect();
    all.sort();
    all.reverse();
    all
}

/// Writes today's snapshot if there isn't one yet and keeps the newest few.
pub fn daily_backup(conn: &Connection, data_dir: &Path) -> Result<()> {
    let dir = backup_dir(data_dir);
    fs::create_dir_all(&dir)?;
    let today = dir.join(format!("library-{}.db", chrono::Local::now().format("%Y-%m-%d")));
    if !today.exists() {
        // VACUUM INTO writes a consistent, compact copy even while the app is running.
        conn.execute("VACUUM INTO ?1", [today.to_string_lossy()])?;
        tracing::info!(file = %today.display(), "library backed up");
    }
    for old in backups(data_dir).into_iter().skip(KEEP) {
        let _ = fs::remove_file(old);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rs-backup-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn note_count(conn: &Connection) -> i64 {
        conn.query_row("SELECT count(*) FROM categories WHERE name = 'Fonts'", [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn corrupt_library_is_restored_from_backup() {
        let dir = tmp("restore");
        {
            let (conn, note) = open_with_recovery(&dir).unwrap();
            assert!(note.is_none());
            conn.execute(
                "INSERT INTO categories (name, color, sort_order, created_at) VALUES ('Fonts', '#000000', 9, 0)",
                [],
            )
            .unwrap();
            daily_backup(&conn, &dir).unwrap();
            daily_backup(&conn, &dir).unwrap(); // idempotent per day
        }
        assert_eq!(backups(&dir).len(), 1);

        fs::write(dir.join("library.db"), b"this is not a database at all").unwrap();
        let (conn, note) = open_with_recovery(&dir).unwrap();
        assert!(note.unwrap().contains("restored"));
        assert_eq!(note_count(&conn), 1); // data came back
        assert!(fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().starts_with("library.db.damaged-")));
    }

    #[test]
    fn without_a_backup_a_fresh_library_starts_and_the_old_file_is_kept() {
        let dir = tmp("fresh");
        fs::write(dir.join("library.db"), b"garbage").unwrap();
        let (conn, note) = open_with_recovery(&dir).unwrap();
        assert!(note.unwrap().contains("new one was started"));
        assert_eq!(note_count(&conn), 0);
        assert_eq!(conn.query_row("SELECT count(*) FROM categories", [], |r| r.get::<_, i64>(0)).unwrap(), 6);
    }

    #[test]
    fn only_the_newest_backups_are_kept() {
        let dir = tmp("prune");
        let (conn, _) = open_with_recovery(&dir).unwrap();
        fs::create_dir_all(backup_dir(&dir)).unwrap();
        for day in ["2026-01-01", "2026-01-02", "2026-01-03", "2026-01-04"] {
            fs::write(backup_dir(&dir).join(format!("library-{day}.db")), b"x").unwrap();
        }
        daily_backup(&conn, &dir).unwrap();
        let names: Vec<String> =
            backups(&dir).iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names.len(), KEEP);
        assert!(names.contains(&"library-2026-01-04.db".to_string()));
        assert!(!names.contains(&"library-2026-01-01.db".to_string()));
    }
}
