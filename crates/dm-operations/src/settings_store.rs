//! rusqlite-backed settings store. Rust owns settings/look persistence (ADR-0019
//! defaults) — localStorage holds only ephemeral UI state. One row, columnar so
//! future migrations can add/rename fields explicitly, guarded by
//! `PRAGMA user_version`. Writes go through a transaction: read → apply → write →
//! commit, so a crash mid-write can never leave a half-applied settings row.

use std::path::Path;
use std::sync::Mutex;

use dm_contracts::{Language, SettingsDto, SettingsPatch, Theme};
use rusqlite::{Connection, Transaction};

use crate::error::{OperationError, Result};

/// The schema version this build understands. Bump alongside a new migration
/// step in [`migrate`]; opening a file written by a newer build is refused
/// rather than silently truncated.
const SCHEMA_VERSION: u32 = 1;

pub struct SettingsStore {
    conn: Mutex<Connection>,
}

impl SettingsStore {
    /// Open (creating if needed) the settings database at `path` and migrate it
    /// forward to [`SCHEMA_VERSION`].
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::from_conn(conn)
    }

    /// In-memory store for tests and headless tooling.
    pub fn open_in_memory() -> Result<Self> {
        Self::from_conn(Connection::open_in_memory()?)
    }

    fn from_conn(mut conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", true)?;
        migrate(&mut conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Read the persisted settings row.
    pub fn get(&self) -> Result<SettingsDto> {
        let conn = self.lock();
        read_row(&conn)
    }

    /// Apply a sparse patch transactionally and return the new full row.
    /// Absent patch fields are left untouched.
    pub fn set(&self, patch: &SettingsPatch) -> Result<SettingsDto> {
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        let mut current = read_row(&tx)?;
        current.apply(patch);
        write_row(&tx, &current)?;
        tx.commit()?;
        Ok(current)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        // A poisoned lock means a prior writer panicked mid-transaction; the
        // uncommitted transaction has already rolled back, so the connection is
        // still consistent to reuse.
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn migrate(conn: &mut Connection) -> Result<()> {
    let version: u32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version > SCHEMA_VERSION {
        return Err(OperationError::SchemaTooNew {
            found: version,
            expected: SCHEMA_VERSION,
        });
    }
    if version == SCHEMA_VERSION {
        return Ok(());
    }

    // Run the whole migration — schema, seed row, and the `user_version` bump — as ONE
    // transaction, so a crash can never commit a half-migrated database. Belt-and-suspenders,
    // `CREATE TABLE IF NOT EXISTS` + `INSERT OR IGNORE` also recover a database a pre-fix crash
    // already left torn (table present, `user_version` still 0): re-running is a no-op instead of
    // a fatal "table already exists" that permanently bricked the file (P2-2).
    let tx = conn.transaction()?;
    if version < 1 {
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS app_settings (
                 id                    INTEGER PRIMARY KEY CHECK (id = 1),
                 theme                 TEXT    NOT NULL,
                 language              TEXT    NOT NULL,
                 keep_new_icons_styled INTEGER NOT NULL,
                 wallpaper_coach_shown INTEGER NOT NULL
             );",
        )?;
        let defaults = SettingsDto::default();
        tx.execute(
            "INSERT OR IGNORE INTO app_settings
                 (id, theme, language, keep_new_icons_styled, wallpaper_coach_shown)
             VALUES (1, ?1, ?2, ?3, ?4)",
            (
                tag(&defaults.theme)?,
                tag(&defaults.language)?,
                defaults.keep_new_icons_styled as i64,
                defaults.wallpaper_coach_shown as i64,
            ),
        )?;
    }
    tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    tx.commit()?;
    Ok(())
}

fn read_row(conn: &Connection) -> Result<SettingsDto> {
    conn.query_row(
        "SELECT theme, language, keep_new_icons_styled, wallpaper_coach_shown
         FROM app_settings WHERE id = 1",
        [],
        |row| {
            let theme: String = row.get(0)?;
            let language: String = row.get(1)?;
            let keep: i64 = row.get(2)?;
            let shown: i64 = row.get(3)?;
            Ok((theme, language, keep, shown))
        },
    )
    .map_err(OperationError::from)
    .and_then(|(theme, language, keep, shown)| {
        Ok(SettingsDto {
            theme: from_tag::<Theme>(&theme)?,
            language: from_tag::<Language>(&language)?,
            keep_new_icons_styled: keep != 0,
            wallpaper_coach_shown: shown != 0,
        })
    })
}

fn write_row(tx: &Transaction<'_>, dto: &SettingsDto) -> Result<()> {
    tx.execute(
        "UPDATE app_settings SET
             theme = ?1, language = ?2,
             keep_new_icons_styled = ?3, wallpaper_coach_shown = ?4
         WHERE id = 1",
        (
            tag(&dto.theme)?,
            tag(&dto.language)?,
            dto.keep_new_icons_styled as i64,
            dto.wallpaper_coach_shown as i64,
        ),
    )?;
    Ok(())
}

/// Serialize an enum to its canonical serde tag (the same string the TS union
/// uses) so the stored text can never drift from the contract.
fn tag<T: serde::Serialize>(value: &T) -> Result<String> {
    match serde_json::to_value(value).map_err(|e| OperationError::Corrupt(e.to_string()))? {
        serde_json::Value::String(s) => Ok(s),
        other => Err(OperationError::Corrupt(format!(
            "expected a string enum tag, got {other}"
        ))),
    }
}

fn from_tag<T: serde::de::DeserializeOwned>(s: &str) -> Result<T> {
    serde_json::from_value(serde_json::Value::String(s.to_owned()))
        .map_err(|e| OperationError::Corrupt(format!("unknown tag {s:?}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_store_returns_defaults() {
        let store = SettingsStore::open_in_memory().unwrap();
        assert_eq!(store.get().unwrap(), SettingsDto::default());
    }

    #[test]
    fn set_is_sparse_and_returns_full_row() {
        let store = SettingsStore::open_in_memory().unwrap();
        let patch: SettingsPatch =
            serde_json::from_str(r#"{"theme":"Dark","wallpaperCoachShown":true}"#).unwrap();
        let after = store.set(&patch).unwrap();
        assert_eq!(after.theme, Theme::Dark);
        assert!(after.wallpaper_coach_shown);
        // Untouched fields keep their defaults.
        assert_eq!(after.language, Language::System);
        assert!(!after.keep_new_icons_styled);
    }

    #[test]
    fn writes_persist_across_reopen() {
        let dir = std::env::temp_dir().join(format!("dm-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.sqlite3");
        {
            let store = SettingsStore::open(&path).unwrap();
            let patch: SettingsPatch =
                serde_json::from_str(r#"{"language":"zh-Hans"}"#).unwrap();
            store.set(&patch).unwrap();
        }
        let reopened = SettingsStore::open(&path).unwrap();
        assert_eq!(reopened.get().unwrap().language, Language::ZhHans);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn interrupted_migration_recovers_on_next_open() {
        // P2-2: reproduce a crash mid-migration — the table was created but `user_version` was
        // never bumped. The next open must recover, not fail with "table already exists" and
        // permanently brick the database.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.sqlite3");
        {
            // A raw connection left in exactly the torn state: table present, user_version = 0.
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE app_settings (
                     id                    INTEGER PRIMARY KEY CHECK (id = 1),
                     theme                 TEXT    NOT NULL,
                     language              TEXT    NOT NULL,
                     keep_new_icons_styled INTEGER NOT NULL,
                     wallpaper_coach_shown INTEGER NOT NULL
                 );",
            )
            .unwrap();
            let version: u32 =
                conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
            assert_eq!(version, 0, "the crash was before the user_version bump");
        }
        // The next open must succeed and yield usable defaults, not brick on the existing table.
        let store = SettingsStore::open(&path).unwrap();
        assert_eq!(store.get().unwrap(), SettingsDto::default());
    }

    #[test]
    fn refuses_newer_schema() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
        assert!(matches!(
            SettingsStore::from_conn(conn),
            Err(OperationError::SchemaTooNew { .. })
        ));
    }
}
