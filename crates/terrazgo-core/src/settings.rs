// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! App settings: a small typed struct persisted as `settings.json` in the app
//! data directory.
//!
//! Deliberately NOT in the database: settings are device-local preferences
//! with a different lifecycle from farm data — no audit trail, no sync, and
//! excluded from backups (a backup exists so regulatory records survive a
//! lost device; it must not impose the old device's cache cap on a new one).
//! The same lifecycle reasoning that keeps `geo-cache.db` a separate file
//! (docs/architecture.md → Data lifecycles).
//!
//! Defaults live in code, not in the file: a missing file or a missing field
//! means "use the default" (`#[serde(default)]` fills it), so a new setting
//! is just a new struct field — old files keep loading, no migrations. An
//! unreadable or unparseable file falls back to defaults: settings are the
//! one store where self-healing beats surfacing corruption, because losing
//! them costs the user a minute of clicking (the geo-cache philosophy, not
//! the app-database one).
//!
//! Secrets never go in this file. It is plain text in the data directory;
//! future credentials (e.g. CDSE) need their own storage decision.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Every app setting, as one flat struct. Fields are `Option` where "unset"
/// must keep following the owning code's default across upgrades — a `None`
/// is "the user never chose", not "the default at the time of writing".
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    /// Tile-cache ceiling override in bytes. `None` follows the default owned
    /// by `terrazgo-geo` (`TILE_CACHE_MAX_BYTES`); the shell resolves it at
    /// startup and on change. Range-validated by the owner, not here.
    pub tile_cache_max_bytes: Option<i64>,
}

/// Read settings from `path`, falling back to defaults on ANY failure —
/// missing file (the normal first run), unreadable file, or invalid JSON.
/// Unknown fields are ignored (a downgrade reads a newer file fine); missing
/// fields take their defaults (an upgrade reads an older file fine).
pub fn load_settings(path: &Path) -> AppSettings {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => AppSettings::default(),
    }
}

/// Write settings to `path` atomically: serialize to a sibling temp file,
/// then rename over the target. A crash mid-write leaves either the old file
/// or the new one, never a torn half-write (rename within one directory is
/// atomic on every target filesystem).
pub fn save_settings(path: &Path, settings: &AppSettings) -> Result<()> {
    let json = serde_json::to_vec_pretty(settings)?;
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    std::fs::write(&tmp, &json)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// Fresh per-test directory; std-only, mirroring the geo-cache tests.
    fn test_dir(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("terrazgo-settings-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_file_yields_defaults() {
        let dir = test_dir("missing");
        let settings = load_settings(&dir.join("settings.json"));
        assert_eq!(settings, AppSettings::default());
        assert_eq!(
            settings.tile_cache_max_bytes, None,
            "unset follows the owner's default"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn round_trip_preserves_values_and_leaves_no_temp_file() {
        let dir = test_dir("roundtrip");
        let path = dir.join("settings.json");
        let settings = AppSettings {
            tile_cache_max_bytes: Some(256 * 1024 * 1024),
        };
        save_settings(&path, &settings).unwrap();
        assert_eq!(load_settings(&path), settings);
        // The atomic-write temp file must not linger after a successful save.
        assert!(!path.with_file_name("settings.json.tmp").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults() {
        let dir = test_dir("corrupt");
        let path = dir.join("settings.json");
        std::fs::write(&path, b"{ not json").unwrap();
        assert_eq!(load_settings(&path), AppSettings::default());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unknown_and_missing_fields_are_tolerated() {
        let dir = test_dir("fields");
        let path = dir.join("settings.json");
        // A file written by a newer version (unknown field) that also predates
        // some current field (missing field): both directions must load.
        std::fs::write(&path, br#"{ "from_the_future": true }"#).unwrap();
        assert_eq!(load_settings(&path), AppSettings::default());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn save_overwrites_previous_settings() {
        let dir = test_dir("overwrite");
        let path = dir.join("settings.json");
        save_settings(
            &path,
            &AppSettings {
                tile_cache_max_bytes: Some(1024 * 1024 * 1024),
            },
        )
        .unwrap();
        // Back to "never chose": the None must genuinely replace the old value.
        save_settings(&path, &AppSettings::default()).unwrap();
        assert_eq!(load_settings(&path).tile_cache_max_bytes, None);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
