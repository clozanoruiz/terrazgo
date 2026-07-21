// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Files the USER picks in native dialogs, on every platform.
//!
//! Desktop dialogs return plain filesystem paths. Android's dialogs are the
//! system document picker (Storage Access Framework), which creates the
//! destination itself and returns a `content://` URI — `std::fs` cannot open
//! those, which is how the first on-device exports produced 0-byte files in
//! Downloads plus an os-error-2 notification. The fs plugin resolves a
//! content URI to an ordinary file descriptor through the platform
//! `ContentResolver`, so everything here funnels through
//! [`tauri_plugin_fs::Fs::open`]: plain paths behave exactly like `std::fs`,
//! URIs come back as real `std::fs::File`s.
//!
//! Two shapes cover all callers:
//!  * writes — [`write_user_file`] (in-memory bytes), or [`stage_dest`] +
//!    [`copy_to_user_file`] when the producer needs a real path to write to
//!    (SQLite's `VACUUM INTO`);
//!  * reads — [`stage_user_source`], which passes plain paths through and
//!    stages a URI into a private temp copy (rusqlite and the GPKG reader
//!    need real paths). Staged copies are deleted on drop.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Manager};
use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};

/// `FilePath`'s parser is infallible: anything with a real scheme (like
/// `content://`) is a URI, everything else is a filesystem path.
fn parse(path: &str) -> FilePath {
    match FilePath::from_str(path) {
        Ok(file_path) => file_path,
        Err(never) => match never {},
    }
}

/// Write bytes to a save-dialog destination (overwriting is already confirmed
/// by the dialog itself). Truncates: on Android the picker pre-creates the
/// document, and a re-export over a longer previous file must not leave a
/// tail of stale bytes.
pub fn write_user_file(app: &AppHandle, dest: &str, bytes: &[u8]) -> io::Result<()> {
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    let mut file = app.fs().open(parse(dest), opts)?;
    file.write_all(bytes)
}

/// Stream an already-written local file (e.g. a verified backup snapshot) to
/// a save-dialog destination.
pub fn copy_to_user_file(app: &AppHandle, src: &Path, dest: &str) -> io::Result<u64> {
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    let mut out = app.fs().open(parse(dest), opts)?;
    let mut input = std::fs::File::open(src)?;
    io::copy(&mut input, &mut out)
}

/// A private staging file in the app cache dir; deleted on drop (best-effort
/// — the cache dir is disposable by definition, so a crash leaves no
/// precious litter).
pub struct StagingFile {
    path: PathBuf,
}

impl StagingFile {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for StagingFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// `None` when `dest` is a plain path — write it directly. `Some(staging)`
/// when it is a content URI: write to `staging.path()` (a real filesystem
/// path, which engines like `VACUUM INTO` require), then stream the result
/// out with [`copy_to_user_file`].
pub fn stage_dest(app: &AppHandle, dest: &str) -> io::Result<Option<StagingFile>> {
    match parse(dest) {
        FilePath::Path(_) => Ok(None),
        FilePath::Url(_) => Ok(Some(StagingFile {
            path: fresh_staging_path(app)?,
        })),
    }
}

/// An open-dialog source made readable through a real filesystem path.
pub enum UserSource {
    /// The dialog gave a plain path — used in place, never deleted.
    Direct(PathBuf),
    /// A content URI, streamed into a staging copy (deleted on drop). Each
    /// call stages afresh; callers that read the same URI twice (boundary
    /// list → geometry read) pay a second copy, which is fine for the
    /// file sizes involved.
    Staged(StagingFile),
}

impl UserSource {
    pub fn path(&self) -> &Path {
        match self {
            UserSource::Direct(path) => path,
            UserSource::Staged(staging) => staging.path(),
        }
    }
}

/// Make an open-dialog result readable at a real filesystem path.
pub fn stage_user_source(app: &AppHandle, src: &str) -> io::Result<UserSource> {
    match parse(src) {
        FilePath::Path(path) => Ok(UserSource::Direct(path)),
        url @ FilePath::Url(_) => {
            let staging = StagingFile {
                path: fresh_staging_path(app)?,
            };
            let mut opts = OpenOptions::new();
            opts.read(true);
            let mut input = app.fs().open(url, opts)?;
            let mut out = std::fs::File::create(staging.path())?;
            io::copy(&mut input, &mut out)?;
            Ok(UserSource::Staged(staging))
        }
    }
}

/// Process-unique path under `<cache>/staging/`. The counter (plus the pid in
/// the name) keeps concurrent commands from colliding; content is transient
/// by construction.
fn fresh_staging_path(app: &AppHandle) -> io::Result<PathBuf> {
    static STAGING_SEQ: AtomicU64 = AtomicU64::new(0);
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(io::Error::other)?
        .join("staging");
    std::fs::create_dir_all(&dir)?;
    let seq = STAGING_SEQ.fetch_add(1, Ordering::Relaxed);
    Ok(dir.join(format!("stage-{}-{seq}", std::process::id())))
}
