// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! A temp path that cleans up after itself.
//!
//! Promoted from `terrazgo-geo/tests/import.rs`, which had already settled this
//! once: the system temp directory plus an RAII guard, no `tempfile`
//! dev-dependency. What the other two temp-file test files lacked was the
//! guard — they removed their files with explicit calls at the end of each
//! test, which a failing assertion skips, so a red run left litter behind.

use std::path::{Path, PathBuf};

/// A path in the system temp directory, removed when the guard drops.
///
/// Uniqueness comes from the process id, so `name` only has to be unique within
/// one test binary — separate crates are separate processes.
pub struct TempFile(PathBuf);

impl TempFile {
    /// Reserve a path without creating anything, and clear any leftover from an
    /// earlier run.
    ///
    /// Producers that create their own destination need exactly this:
    /// `VACUUM INTO` refuses a file that already exists.
    pub fn reserve(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("terrazgo-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        Self(path)
    }

    /// Reserve a path and write `contents` to it.
    pub fn written(name: &str, contents: impl AsRef<[u8]>) -> Self {
        let file = Self::reserve(name);
        std::fs::write(&file.0, contents).unwrap();
        file
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        // SQLite in WAL mode writes two sidecars beside the database, and the
        // backup tests open one. Sweeping them here is why a guard beats a
        // `remove_file` call per test: nobody has to remember they exist.
        for suffix in ["-wal", "-shm"] {
            let mut sidecar = self.0.clone().into_os_string();
            sidecar.push(suffix);
            let _ = std::fs::remove_file(PathBuf::from(sidecar));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_and_its_wal_sidecars_are_gone_after_the_guard_drops() {
        let path = {
            let file = TempFile::written("guard.db", b"not really a database");
            let path = file.path().to_path_buf();
            let mut wal = path.clone().into_os_string();
            wal.push("-wal");
            std::fs::write(&wal, b"sidecar").unwrap();

            assert!(path.exists());
            assert!(Path::new(&wal).exists());
            path
        };

        let mut wal = path.clone().into_os_string();
        wal.push("-wal");
        assert!(!path.exists());
        assert!(!Path::new(&wal).exists());
    }

    #[test]
    fn reserve_creates_nothing_so_vacuum_into_can_have_the_path() {
        let file = TempFile::reserve("reserved.db");
        assert!(!file.path().exists());
    }
}
