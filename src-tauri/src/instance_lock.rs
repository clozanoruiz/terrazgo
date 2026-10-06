// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! One running copy of the app per data folder.
//!
//! Two copies on one folder do not merely duplicate a window. Each keeps its
//! own copy of `settings.json` in memory and writes the whole file back on
//! every change, so the last one to save silently undoes the other. And a
//! backup restored in one of them breaks the other twice over: the restore
//! copies the backup over the live file while the second copy still has the
//! old database's write-ahead log open beside it, and the restore mints a new
//! device id that the second copy never sees — so it goes on writing under the
//! retired one, reusing change-set numbers other devices already hold
//! (docs/sync.md → "Sequence rewind").
//!
//! The guard is an operating-system lock on a file in the folder, taken before
//! anything else there is read. The system releases it when the process ends,
//! however it ends, so a crash leaves at most the file, never the lock — which
//! is why a lock and not a "running" marker the app would have to remember to
//! delete.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;

/// The file the lock is taken on, in the data folder.
///
/// It is never deleted, on purpose. The lock belongs to the open file, not to
/// its name: a copy that deleted and re-created the file would take a lock on a
/// new one while the first copy still held the old, and both would run.
pub const LOCK_FILE: &str = "instance.lock";

/// The data folder, held for as long as this value lives. The shell hands it
/// to Tauri's managed state, which is never dropped, so it lives exactly as long
/// as the process.
pub struct InstanceLock {
    _file: File,
}

/// What claiming the data folder found.
pub enum Claim {
    /// This copy holds the folder.
    Held(InstanceLock),
    /// Another copy of the app holds it. This one must open nothing in it.
    HeldElsewhere,
    /// Nothing could say either way: the lock file could not be created, or the
    /// filesystem does not support locks. The app starts unguarded rather than
    /// not at all — refusing to open would lock the farmer out of their
    /// records over a safeguard.
    Unavailable(std::io::Error),
}

/// Try to hold the data folder `data_dir`, without waiting.
pub fn claim(data_dir: &Path) -> Claim {
    // `truncate(false)`: the file's contents are nothing, but a lock file is
    // opened, never emptied — and `create` with no stated truncation is what
    // clippy asks to be spelled out.
    let file = match OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(data_dir.join(LOCK_FILE))
    {
        Ok(file) => file,
        Err(err) => return Claim::Unavailable(err),
    };
    match file.try_lock() {
        Ok(()) => Claim::Held(InstanceLock { _file: file }),
        Err(TryLockError::WouldBlock) => Claim::HeldElsewhere,
        Err(TryLockError::Error(err)) => Claim::Unavailable(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terrazgo_testkit::files::TempDir;

    // Two claims inside one test process stand for two copies of the app. That
    // is sound because the lock belongs to each OPENED file, not to the
    // process: every `claim` opens the file anew, and a second open is refused
    // by the first one's lock just as another process's would be (flock on
    // Linux and macOS, LockFileEx on Windows).

    #[test]
    fn the_first_copy_holds_the_folder() {
        let dir = TempDir::create("instance-lock-first");
        assert!(matches!(claim(dir.path()), Claim::Held(_)));
        assert!(dir.path().join(LOCK_FILE).exists());
    }

    #[test]
    fn a_second_copy_is_refused_while_the_first_runs() {
        let dir = TempDir::create("instance-lock-second");
        let _first = claim(dir.path());
        assert!(matches!(claim(dir.path()), Claim::HeldElsewhere));
    }

    #[test]
    fn the_folder_is_free_again_once_the_first_copy_has_gone() {
        let dir = TempDir::create("instance-lock-released");
        let first = claim(dir.path());
        assert!(matches!(first, Claim::Held(_)));
        drop(first);
        assert!(matches!(claim(dir.path()), Claim::Held(_)));
    }

    #[test]
    fn a_lock_file_left_behind_is_not_a_lock() {
        // What a crash leaves: the file, with nobody holding it.
        let dir = TempDir::create("instance-lock-leftover");
        dir.write(LOCK_FILE, b"");
        assert!(matches!(claim(dir.path()), Claim::Held(_)));
    }

    #[test]
    fn a_folder_that_cannot_hold_the_file_is_unavailable_not_refused() {
        // The distinction is the whole point: a refusal stops the app, and an
        // unavailable lock lets it start.
        let dir = TempDir::create("instance-lock-unavailable");
        let missing = dir.path().join("does-not-exist");
        assert!(matches!(claim(&missing), Claim::Unavailable(_)));
    }
}
