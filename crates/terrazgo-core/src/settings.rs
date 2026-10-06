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
    /// `None` follows module-phytosanitary's own default (`AlertConfig::defaults`).
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
    /// `None` follows module-phytosanitary's default (`default_phi_horizon_days`).
    ///
    /// Bounds a display and not a duty: the restricted state is unaffected, and
    /// stays date-scoped across every campaign whatever this says.
    pub phi_recent_days: Option<i64>,
    /// This installation's device id: which replica wrote each row of the
    /// change log (docs/sync.md → Device identity). Not the active profile —
    /// that is a person, this is a copy of the database.
    ///
    /// Here rather than in the database *because* this file is excluded from
    /// backups: a snapshot restored on another machine must not let that
    /// machine write under this one's name. For the same reason the shell mints
    /// a fresh id whenever a backup is imported, even on this device.
    ///
    /// `None`, or a value that is not a canonical UUID, is a first launch or a
    /// lost file, and [`AppSettings::ensure_device_id`] mints a new one. The
    /// device then looks like a new peer and needs naming again — a cost, never
    /// a corruption.
    pub device_id: Option<String>,
}

impl AppSettings {
    /// What a Settings form submitted, applied over these settings: the form's
    /// choices, with the fields the MACHINE owns carried over from `self`.
    ///
    /// The form sends the whole struct back, so it also sends whatever it last
    /// read — and it may have read it before something else changed it. Two
    /// fields no person sets: `device_id`, which a backup import replaces while
    /// a Settings view can still hold the old one (saving that snapshot back
    /// would put this device's change-set numbers back under an identity the
    /// restored database already left — docs/sync.md → Device identity), and
    /// `last_integrity_check`, the verdict the weekly check writes. Everything
    /// else is a preference and the form's to decide.
    pub fn apply_form(&self, form: AppSettings) -> AppSettings {
        AppSettings {
            device_id: self.device_id.clone(),
            last_integrity_check: self.last_integrity_check.clone(),
            ..form
        }
    }

    /// The id this device writes as, minting one when there is none or the
    /// stored value is unusable. Returns the id and whether it was minted just
    /// now, so the caller knows the file needs saving.
    pub fn ensure_device_id(&mut self) -> (String, bool) {
        match &self.device_id {
            Some(id) if crate::sync::is_device_id(id) => (id.clone(), false),
            _ => {
                let id = crate::sync::mint_device_id();
                self.device_id = Some(id.clone());
                (id, true)
            }
        }
    }
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
            device_id: Some("0198b7a0-0000-7000-8000-00000000000d".into()),
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
    fn a_form_sets_preferences_and_never_what_the_machine_owns() {
        // The case that matters: a Settings view read the settings, a backup
        // import then gave this device a new id, and the view saves a lead
        // time from its stale copy. The lead time lands; the old id does not.
        let current = AppSettings {
            device_id: Some("0198b7a0-0000-7000-8000-0000000000b2".into()),
            last_integrity_check: Some(IntegrityCheck {
                at: "2026-09-18T09:00:00Z".into(),
                ok: true,
                thorough: false,
            }),
            licence_lead_days: Some(30),
            ..AppSettings::default()
        };
        let stale_form = AppSettings {
            device_id: Some("0198b7a0-0000-7000-8000-0000000000a1".into()),
            last_integrity_check: None,
            licence_lead_days: Some(90),
            active_user_id: Some("profile-1".into()),
            ..AppSettings::default()
        };

        let saved = current.apply_form(stale_form);
        assert_eq!(saved.device_id, current.device_id);
        assert_eq!(saved.last_integrity_check, current.last_integrity_check);
        assert_eq!(saved.licence_lead_days, Some(90));
        assert_eq!(saved.active_user_id.as_deref(), Some("profile-1"));
    }

    #[test]
    fn a_first_launch_mints_a_device_id_and_says_so() {
        let mut settings = AppSettings::default();
        let (id, minted) = settings.ensure_device_id();
        assert!(minted);
        assert!(crate::sync::is_device_id(&id));
        assert_eq!(settings.device_id.as_deref(), Some(id.as_str()));
    }

    #[test]
    fn a_stored_device_id_is_kept() {
        // Minting on every launch would make each launch a stranger to the
        // changes the last one wrote.
        let mut settings = AppSettings {
            device_id: Some("0198b7a0-0000-7000-8000-00000000000d".into()),
            ..AppSettings::default()
        };
        assert_eq!(
            settings.ensure_device_id(),
            ("0198b7a0-0000-7000-8000-00000000000d".to_string(), false)
        );
    }

    #[test]
    fn a_hand_mangled_device_id_is_replaced_not_used() {
        // The file is plain text a person can edit, and the id goes into every
        // log row forever after; an upper-case spelling of one UUID would name
        // the same device two ways.
        for mangled in ["laptop", "0198B7A0-0000-7000-8000-00000000000D", ""] {
            let mut settings = AppSettings {
                device_id: Some(mangled.into()),
                ..AppSettings::default()
            };
            let (id, minted) = settings.ensure_device_id();
            assert!(minted, "{mangled:?}");
            assert_ne!(id, mangled);
        }
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
