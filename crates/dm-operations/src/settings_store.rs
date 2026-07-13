//! rusqlite-backed settings store. Rust owns settings/look persistence (ADR-0019
//! defaults) — localStorage holds only ephemeral UI state. One row, columnar so
//! future migrations can add/rename fields explicitly, guarded by
//! `PRAGMA user_version`. Writes go through a transaction: read → apply → write →
//! commit, so a crash mid-write can never leave a half-applied settings row.

use std::path::Path;
use std::sync::Mutex;

use dm_contracts::{IconStyle, Language, SettingsDto, SettingsPatch, Theme};
use rusqlite::{Connection, Transaction};

use crate::error::{OperationError, Result};

/// The schema version this build understands. Bump alongside a new migration
/// step in [`migrate`]; opening a file written by a newer build is refused
/// rather than silently truncated.
const SCHEMA_VERSION: u32 = 2;

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
    ///
    /// **Resident precondition (spec 07 §2 item 2):** a patch that would set
    /// `keep_new_icons_styled = true` while store ② (saved-style) is empty is REJECTED here, not
    /// only in the UI — the toggle cannot be enabled before the user has completed one successful
    /// global Apply, so "a saved style exists" stays an invariant the reconciler can rely on
    /// rather than an edge case. Reading ② inside the same transaction keeps the check atomic
    /// against a concurrent apply.
    pub fn set(&self, patch: &SettingsPatch) -> Result<SettingsDto> {
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        if patch.keep_new_icons_styled == Some(true) {
            // The saved-style cell must be a VALID style, not merely non-NULL (codex m7b-🟡7):
            // decode it with the canonical reader so malformed JSON / `"null"` / a BLOB / an empty
            // string cannot slip the toggle on — the resident would then have no projectable
            // style. `None` → precondition error; corrupt → surfaced as Corrupt.
            let raw: rusqlite::types::Value = tx.query_row(
                "SELECT icon_style_json FROM app_settings WHERE id = 1",
                [],
                |row| row.get(0),
            )?;
            if decode_saved_style(raw)?.is_none() {
                return Err(OperationError::InvalidPayload(
                    "cannot enable auto-format before a style has been applied (spec 07 §2 precondition)"
                        .into(),
                ));
            }
        }
        let mut current = read_row(&tx)?;
        current.apply(patch);
        write_row(&tx, &current)?;
        tx.commit()?;
        Ok(current)
    }

    /// ATOMICALLY clears ② saved-style AND turns the auto-format toggle off, in ONE transaction —
    /// the reset coupling (spec 07 §10 ★, codex m7b-🟠5). Two separate autocommits could crash
    /// between them, leaving `icon_style_json=NULL, keep_new_icons_styled=true` (dormant, but the
    /// UI still claims automation is on, and a later global Apply that only restores ② would
    /// silently revive it). One transaction closes that window.
    pub fn reset_style_and_autoformat(&self) -> Result<()> {
        let conn = self.lock();
        conn.execute(
            "UPDATE app_settings SET icon_style_json = NULL, keep_new_icons_styled = 0 WHERE id = 1",
            [],
        )?;
        Ok(())
    }

    /// Reads store ② of the appearance model (spec 07 §8.2) — the single saved-style recipe.
    /// `None` means no style has been saved (system default). Fail-closed like the ledger: a
    /// present-but-invalid cell is an [`OperationError::Corrupt`], never silently `None`, so a saved
    /// style that cannot be read (bad JSON, an envelope that fails [`IconStyle`] validation, or a
    /// non-TEXT cell) is visible rather than masquerading as "never applied."
    pub fn get_saved_style(&self) -> Result<Option<IconStyle>> {
        let conn = self.lock();
        let raw: rusqlite::types::Value = conn.query_row(
            "SELECT icon_style_json FROM app_settings WHERE id = 1",
            [],
            |row| row.get(0),
        )?;
        decode_saved_style(raw)
    }

    /// Writes store ② (spec 07 §8.2/§8.4) — called ONLY on a completed global Apply, never from
    /// the settings-patch path, a draft debounce, or a single-icon edit. The recipe is
    /// pre-validated ([`IconStyle`]), so a null/garbage value can never be persisted as a style.
    /// `None` clears it (the "apply the system default" path: ② becomes NULL, which the automation
    /// layer reads as "nothing to project").
    pub fn set_saved_style(&self, style: Option<&IconStyle>) -> Result<()> {
        let text: Option<String> = match style {
            Some(s) => Some(serde_json::to_string(s.as_value())?),
            None => None,
        };
        let conn = self.lock();
        conn.execute("UPDATE app_settings SET icon_style_json = ?1 WHERE id = 1", (text,))?;
        Ok(())
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
    if version < 2 {
        // Store ② of the appearance model (spec 07 §8.2): the single saved-style blob, written
        // only on a global Apply. Nullable — NULL means "no saved style / system default". SQLite
        // has no `ADD COLUMN IF NOT EXISTS`, so guard by inspecting the schema (idempotent even if
        // a torn pre-commit run had already added it — belt-and-suspenders with the transactional
        // migration, matching the version<1 recovery discipline).
        if !column_exists(&tx, "app_settings", "icon_style_json")? {
            tx.execute_batch("ALTER TABLE app_settings ADD COLUMN icon_style_json TEXT;")?;
        }
    }
    tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    tx.commit()?;
    Ok(())
}

/// Whether `table` already has a column named `column` (via `PRAGMA table_info`), so an additive
/// `ALTER TABLE ... ADD COLUMN` migration step can be skipped when it already ran.
fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?; // table_info columns: (cid, name, type, ...)
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Decodes a raw `icon_style_json` cell into store ② — the ONE canonical reader shared by
/// `get_saved_style` and the enable precondition (codex m7b-🟡7): `Null` → `None`; valid text →
/// `Some(IconStyle)`; malformed JSON / a non-style envelope / a non-TEXT cell → `Corrupt`.
fn decode_saved_style(raw: rusqlite::types::Value) -> Result<Option<IconStyle>> {
    use rusqlite::types::Value as SqlValue;
    match raw {
        SqlValue::Null => Ok(None),
        SqlValue::Text(text) => {
            let value: serde_json::Value = serde_json::from_str(&text)
                .map_err(|e| OperationError::Corrupt(format!("saved style: {e}")))?;
            IconStyle::from_value(value)
                .map(Some)
                .map_err(|e| OperationError::Corrupt(format!("saved style: {e}")))
        }
        other => Err(OperationError::Corrupt(format!("saved style cell is not text: {other:?}"))),
    }
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

    fn style(seed: i64) -> IconStyle {
        IconStyle::from_value(
            serde_json::json!({ "config": { "seed": seed }, "kindPolicy": {}, "typeOverrides": {} }),
        )
        .unwrap()
    }

    #[test]
    fn saved_style_defaults_to_none_and_round_trips() {
        let store = SettingsStore::open_in_memory().unwrap();
        // Store ② starts empty (system default).
        assert!(store.get_saved_style().unwrap().is_none());

        store.set_saved_style(Some(&style(3))).unwrap();
        assert_eq!(store.get_saved_style().unwrap(), Some(style(3)));

        // Clearing it (the "apply the system default" path) returns to None.
        store.set_saved_style(None).unwrap();
        assert!(store.get_saved_style().unwrap().is_none());
    }

    #[test]
    fn saved_style_is_independent_of_the_settings_row_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.sqlite3");
        {
            let store = SettingsStore::open(&path).unwrap();
            store.set_saved_style(Some(&style(9))).unwrap();
            // A settings patch must NOT disturb store ② (they are orthogonal, spec 07 §8.2).
            let patch: SettingsPatch = serde_json::from_str(r#"{"theme":"Dark"}"#).unwrap();
            store.set(&patch).unwrap();
        }
        let reopened = SettingsStore::open(&path).unwrap();
        assert_eq!(reopened.get_saved_style().unwrap(), Some(style(9)));
        assert_eq!(reopened.get().unwrap().theme, Theme::Dark);
    }

    #[test]
    fn migrates_a_v1_database_by_adding_the_saved_style_column() {
        // A database written by the pre-store-② build: the four original columns, user_version 1,
        // no icon_style_json. Opening it must migrate forward (add the column), not fail.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.sqlite3");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE app_settings (
                     id                    INTEGER PRIMARY KEY CHECK (id = 1),
                     theme                 TEXT    NOT NULL,
                     language              TEXT    NOT NULL,
                     keep_new_icons_styled INTEGER NOT NULL,
                     wallpaper_coach_shown INTEGER NOT NULL
                 );
                 INSERT INTO app_settings VALUES (1, 'System', 'System', 0, 0);",
            )
            .unwrap();
            conn.pragma_update(None, "user_version", 1u32).unwrap();
        }
        let store = SettingsStore::open(&path).unwrap();
        // The new column exists and reads as None; existing settings survive the migration.
        assert!(store.get_saved_style().unwrap().is_none());
        assert_eq!(store.get().unwrap(), SettingsDto::default());
        // And it is writable post-migration.
        store.set_saved_style(Some(&style(1))).unwrap();
        assert_eq!(store.get_saved_style().unwrap(), Some(style(1)));
    }

    #[test]
    fn corrupt_saved_style_fails_closed_never_silently_none() {
        let store = SettingsStore::open_in_memory().unwrap();
        // Invalid JSON in the column → Corrupt, never silently None.
        store
            .lock()
            .execute("UPDATE app_settings SET icon_style_json = '{ not json' WHERE id = 1", [])
            .unwrap();
        assert!(matches!(store.get_saved_style(), Err(OperationError::Corrupt(_))));

        // Valid JSON that is NOT a valid style envelope (e.g. `null`) also fails closed — M7 must
        // never read it as a non-empty ② with nothing to project.
        store
            .lock()
            .execute("UPDATE app_settings SET icon_style_json = 'null' WHERE id = 1", [])
            .unwrap();
        assert!(matches!(store.get_saved_style(), Err(OperationError::Corrupt(_))));

        // A genuinely non-TEXT cell — a BLOB, which TEXT affinity does NOT coerce (an integer would
        // be coerced to TEXT "42" and only exercise the JSON-parse path) — must hit the non-text
        // branch as Corrupt, not None.
        store
            .lock()
            .execute("UPDATE app_settings SET icon_style_json = x'deadbeef' WHERE id = 1", [])
            .unwrap();
        assert!(matches!(store.get_saved_style(), Err(OperationError::Corrupt(_))));
    }

    #[test]
    fn the_resident_precondition_rejects_enabling_auto_format_before_a_style_exists() {
        use dm_contracts::SettingsPatch;
        let store = SettingsStore::open_in_memory().unwrap();
        // ② empty → enabling the toggle is rejected at the patch layer, not silently accepted.
        let err = store
            .set(&SettingsPatch { keep_new_icons_styled: Some(true), ..Default::default() })
            .unwrap_err();
        assert!(matches!(err, OperationError::InvalidPayload(_)));
        assert!(!store.get().unwrap().keep_new_icons_styled, "the toggle stayed off");

        // A patch that does NOT touch the toggle is unaffected by the guard.
        store
            .set(&SettingsPatch { wallpaper_coach_shown: Some(true), ..Default::default() })
            .unwrap();

        // Once a style is saved (a completed global Apply), enabling the toggle succeeds.
        let style = IconStyle::from_value(serde_json::json!({
            "config": { "shape": "Circle", "subject": "Original", "tint": "#FF6F5E",
                "monoStyle": "Tonal", "plateBand": "Vivid", "shortcutShape": null,
                "distinction": "None", "markStyle": "Glass", "markColor": null, "size": "Mid",
                "filter": "None", "plateColor": null, "plateFallback": "derived" },
            "kindPolicy": {}, "typeOverrides": {}
        }))
        .unwrap();
        store.set_saved_style(Some(&style)).unwrap();
        let after = store
            .set(&SettingsPatch { keep_new_icons_styled: Some(true), ..Default::default() })
            .unwrap();
        assert!(after.keep_new_icons_styled, "enabling succeeds once ② is non-empty");
    }

    #[test]
    fn the_precondition_rejects_a_corrupt_non_null_saved_style() {
        // codex m7b-🟡7: a malformed/`"null"`/non-style ② must NOT let the toggle on — it would
        // leave the resident with no projectable style. The precondition reuses the canonical
        // decoder, so corruption is caught, not slipped through as "non-NULL therefore valid".
        use dm_contracts::SettingsPatch;
        let store = SettingsStore::open_in_memory().unwrap();
        store.lock().execute("UPDATE app_settings SET icon_style_json = '{ bad json' WHERE id = 1", []).unwrap();
        assert!(matches!(
            store.set(&SettingsPatch { keep_new_icons_styled: Some(true), ..Default::default() }),
            Err(OperationError::Corrupt(_))
        ));
        assert!(!store.get().unwrap().keep_new_icons_styled, "the toggle stayed off");
    }

    #[test]
    fn reset_style_and_autoformat_clears_both_atomically() {
        use dm_contracts::SettingsPatch;
        let store = SettingsStore::open_in_memory().unwrap();
        let style = IconStyle::from_value(serde_json::json!({
            "config": { "shape": "Circle", "subject": "Original", "tint": "#FF6F5E",
                "monoStyle": "Tonal", "plateBand": "Vivid", "shortcutShape": null,
                "distinction": "None", "markStyle": "Glass", "markColor": null, "size": "Mid",
                "filter": "None", "plateColor": null, "plateFallback": "derived" },
            "kindPolicy": {}, "typeOverrides": {}
        }))
        .unwrap();
        store.set_saved_style(Some(&style)).unwrap();
        store.set(&SettingsPatch { keep_new_icons_styled: Some(true), ..Default::default() }).unwrap();

        store.reset_style_and_autoformat().unwrap();
        assert!(store.get_saved_style().unwrap().is_none(), "② cleared");
        assert!(!store.get().unwrap().keep_new_icons_styled, "toggle off — both in one transaction");
    }
}
