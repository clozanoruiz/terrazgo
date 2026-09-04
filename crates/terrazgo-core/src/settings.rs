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
    /// The device's active user profile (`user_profile.id`). Device-local by
    /// design — "who is using THIS device" is not farm data. Tolerated when
    /// dangling (profile deleted, backup from another install): the shell
    /// degrades to "no active profile", never errors.
    pub active_user_id: Option<String>,
    /// The last time this device checked its database for corruption, and what
    /// it found. Device-local because it is about THIS copy of the file, not
    /// about the farm — and deliberately not in the database, so a database too
    /// damaged to read still has a readable verdict beside it.
    ///
    /// `None` means never checked, which is the normal state on first run.
    pub last_integrity_check: Option<IntegrityCheck>,
    /// How many days before an operator licence expires its alert opens.
    /// `None` follows module-cue's own default (`AlertConfig::defaults`).
    ///
    /// A setting rather than a constant because the right answer is something
    /// the farmer knows and the app cannot: renewing a carné de aplicador means
    /// getting onto a training course with limited dates, and how far ahead one
    /// has to book varies by province and season.
    pub licence_lead_days: Option<i64>,
    /// The same, for a machine's next ITV inspection. Separate from the licence
    /// lead time because they are paced by different things — booking a station
    /// is not booking a course.
    pub itv_lead_days: Option<i64>,
    /// How far back the map's PHI tint keeps showing a plot as treated-and-clear.
    /// `None` follows module-cue's default (`default_phi_horizon_days`).
    ///
    /// Bounds a display and not a duty: the restricted state is unaffected, and
    /// stays date-scoped across every campaign whatever this says.
    pub phi_recent_days: Option<i64>,
}

/// The outcome of one corruption check. One struct rather than two parallel
/// `Option`s so a timestamp and a verdict cannot disagree about whether a check
/// happened.
///
/// `Default` exists only so `#[serde(default)]` can fill a missing field; the
/// derived one is right — an empty instant, not ok, not thorough — and nothing
/// constructs a verdict that way.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct IntegrityCheck {
    /// ISO 8601 UTC instant.
    pub at: String,
    /// Whether the check passed. False means the file is damaged and the farmer
    /// should restore a backup.
    pub ok: bool,
    /// Which check produced this verdict: `false` is the weekly automatic
    /// `quick_check` (structural page damage), `true` the `integrity_check` the
    /// farmer can ask for from Settings, which also verifies indexes against
    /// their tables and the UNIQUE/NOT NULL/CHECK constraints.
    ///
    /// It has to be recorded, or "checked three days ago, fine" means two
    /// different things. `#[serde(default)]` on the struct makes a file written
    /// before this field load as the weekly check it in fact was.
    pub thorough: bool,
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
            active_user_id: Some("0198b7a0-0000-7000-8000-000000000000".into()),
            last_integrity_check: Some(IntegrityCheck {
                at: "2026-08-25T09:00:00Z".into(),
                ok: true,
                thorough: true,
            }),
            licence_lead_days: Some(90),
            itv_lead_days: Some(45),
            phi_recent_days: Some(180),
        };
        save_settings(&path, &settings).unwrap();
        assert_eq!(load_settings(&path), settings);
        // The atomic-write temp file must not linger after a successful save.
        assert!(!path.with_file_name("settings.json.tmp").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_integrity_verdict_survives_the_round_trip() {
        // It has to: the verdict is what a database too corrupt to read still
        // has beside it, and what `get_status` reports on a launch where the
        // weekly check was not due.
        let dir = test_dir("integrity");
        let path = dir.join("settings.json");
        let failed = AppSettings {
            last_integrity_check: Some(IntegrityCheck {
                at: "2026-08-18T09:00:00Z".into(),
                ok: false,
                thorough: false,
            }),
            ..AppSettings::default()
        };
        save_settings(&path, &failed).unwrap();
        let loaded = load_settings(&path);
        assert_eq!(loaded, failed);
        assert!(!loaded.last_integrity_check.unwrap().ok);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_verdict_written_before_thoroughness_was_recorded_loads_as_the_weekly_check() {
        // `thorough` arrived with the Settings button (2026-08-26). Every
        // verdict written before it came from the weekly `quick_check`, so
        // defaulting to false is not a fallback — it is the true answer, and
        // reading it as thorough would overstate what the file was checked for.
        let dir = test_dir("thorough-default");
        let path = dir.join("settings.json");
        std::fs::write(
            &path,
            br#"{ "last_integrity_check": { "at": "2026-08-18T09:00:00Z", "ok": true } }"#,
        )
        .unwrap();

        let check = load_settings(&path).last_integrity_check.unwrap();
        assert_eq!(check.at, "2026-08-18T09:00:00Z");
        assert!(check.ok);
        assert!(!check.thorough, "an old verdict was the quick check");
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
                ..AppSettings::default()
            },
        )
        .unwrap();
        // Back to "never chose": the None must genuinely replace the old value.
        save_settings(&path, &AppSettings::default()).unwrap();
        assert_eq!(load_settings(&path).tile_cache_max_bytes, None);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
