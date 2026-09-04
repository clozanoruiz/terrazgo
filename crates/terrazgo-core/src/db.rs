// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Core-owned migrations, embedded in the binary, and [`Database`] — the
//! handle every long-lived connection in the app is held by.

use crate::error::Result;
use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// The ordered migration steps the core contributes to the single global sequence.
/// They run FIRST — before every module's steps — so module tables may reference
/// core tables (`farm`, `plot`, `country`, `record_change`). SQL is embedded with
/// `include_str!` so the binary needs no files at runtime (offline-first).
///
/// Pre-release the set is squashed freely (dev databases are recreated). The moment
/// any database holds real data, the composed global sequence becomes append-only
/// as a whole (docs/architecture.md → Migrations: one global sequence).
pub fn migration_set() -> Vec<M<'static>> {
    vec![
        M::up(include_str!("../migrations/0001_core_schema.sql")),
        M::up(include_str!("../migrations/0002_seed_countries.sql")),
    ]
}

/// The core's migrations as a runnable set, for this crate's own tests. The app
/// composes `migration_set()` with every module's into the global sequence instead.
pub fn migrations() -> Migrations<'static> {
    Migrations::new(migration_set())
}

/// Open an in-memory database with foreign keys enforced and the CORE migrations
/// applied — enough for testing the core repository in isolation. The app opens
/// its database through the shell's composed runner.
pub fn open_in_memory() -> Result<Connection> {
    let mut conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "foreign_keys", true)?;
    harden(&conn)?;
    migrations().to_latest(&mut conn)?;
    Ok(conn)
}

/// Lock down the SQL features a database file can use against us.
///
/// Applied to **every** connection the app opens, ours and imported alike, and
/// it costs us nothing: the schema defines no views and no triggers, and the
/// app registers no custom SQL functions, collations or virtual tables. Each
/// flag was measured rather than assumed (see the tests below):
///
/// * `DEFENSIVE` — refuses `writable_schema` and `PRAGMA journal_mode=OFF`, the
///   two ways ordinary SQL can deliberately corrupt a file. It also means
///   nothing running in SQL can drop us out of WAL.
/// * `ENABLE_TRIGGER = false` — a trigger in an imported file never fires.
///   **Foreign-key actions are unaffected**, which matters: the schema has 33
///   `ON DELETE CASCADE` clauses and they keep working.
/// * `ENABLE_VIEW = false` — reading a view is refused.
/// * `trusted_schema = OFF` — expressions embedded in a schema are not trusted.
///
/// Two neighbours in the same enum that must NOT be set here, both measured:
/// `NO_CKPT_ON_CLOSE` silently reinstates the WAL-sidecar leak that
/// [`Database::close`] exists to fix, and `cell_size_check` was considered and
/// deliberately left off.
///
/// Note that `CREATE VIEW` and `CREATE TRIGGER` still *succeed* under these
/// flags — only using them is refused. That is why `src-tauri/tests` carries a
/// contract test asserting the composed schema defines neither: without it, a
/// migration adding one would apply cleanly and fail at read time instead.
pub fn harden(conn: &Connection) -> Result<()> {
    use rusqlite::config::DbConfig;
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)?;
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, false)?;
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_VIEW, false)?;
    conn.pragma_update(None, "trusted_schema", false)?;
    Ok(())
}

/// Why there is no connection to hand out: it was closed (at shutdown, or while
/// a backup import swaps the file underneath it), or a panic poisoned the lock.
///
/// Both mean the same thing to a caller — there is no database right now — so
/// each boundary maps this onto whatever it already says in that situation:
/// `terrazgo-geo` degrades to its offline path, the shell raises a command
/// error. Kept out of `CoreError` on purpose: [`Database::lock`] can fail in
/// exactly these two ways, and a crate-wide enum would claim it might also fail
/// as an invalid date or a bad catalogue file.
#[derive(Debug, thiserror::Error)]
pub enum Unavailable {
    #[error("the database is closed")]
    Closed,
    #[error("the database lock is poisoned")]
    Poisoned,
}

/// A SQLite connection, the lock that serialises access to it, and the ability
/// to close it on purpose.
///
/// The lock is not ceremony: a `Connection` is `!Sync`, and Tauri runs commands
/// on a thread pool. The `Option` is what makes the connection *closable* —
/// `Connection::close` consumes the connection, so it has to be possible to
/// take it out of the mutex, and closing is the only thing that deletes the
/// `-wal`/`-shm` sidecars SQLite writes beside a database in WAL mode.
///
/// Closing has to be explicit rather than left to `Drop` for two reasons. The
/// app never drops this value — the platform event loop ends the process with
/// `std::process::exit`, so the shutdown hook is the last chance to close (see
/// docs/architecture.md → Shutdown). And `Connection`'s own `Drop` discards the
/// close error, while `Connection::close` flushes the prepared-statement cache
/// and reports what went wrong.
pub struct Database(Mutex<Option<Connection>>);

impl Database {
    /// Take ownership of an open connection, [`harden`]ing it on the way in.
    ///
    /// Hardening here rather than leaving it to the caller is the point: these
    /// settings live on the connection and not in the file, so every open site
    /// has to apply them and any one of them could forget. Doing it in the
    /// constructor means a `Database` cannot hold an unhardened connection —
    /// the compiler carries the rule instead of the reader.
    ///
    /// Callers may still harden earlier, and two do: `open_app_db` and the geo
    /// cache both run migrations before wrapping, and those should run under
    /// the same restrictions the app queries under. `harden` is idempotent.
    pub fn new(conn: Connection) -> Result<Self> {
        harden(&conn)?;
        Ok(Self(Mutex::new(Some(conn))))
    }

    /// Lock the database for as long as the returned guard lives.
    ///
    /// This is the only place a poisoned lock can be observed; the guard's
    /// accessors are the only place a closed one can be.
    pub fn lock(&self) -> std::result::Result<DbGuard<'_>, Unavailable> {
        #[cfg(debug_assertions)]
        reentrancy::check(self);
        self.0
            .lock()
            .map(|guard| DbGuard::new(self, guard))
            .map_err(|_| Unavailable::Poisoned)
    }

    /// Close the connection, which is what deletes its `-wal`/`-shm` sidecars.
    /// Idempotent — closing an already-closed database is not an error.
    ///
    /// A poisoned lock is recovered from rather than refused: poisoning means a
    /// panic happened while some other thread held the connection, and at
    /// shutdown there is nothing left to protect. Refusing would let one
    /// panicked command be the reason a 6 MB write-ahead log is left on disk.
    pub fn close(&self) -> rusqlite::Result<()> {
        #[cfg(debug_assertions)]
        reentrancy::check(self);
        let guard = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        DbGuard::new(self, guard).close()
    }
}

/// Turns a re-entrant lock — which would hang — into an immediate panic naming
/// the mistake. **Debug builds only**: every item here is behind
/// `#[cfg(debug_assertions)]`, so a release build has no thread-local, no field
/// on [`DbGuard`], no `Drop`, and no check.
///
/// The hazard is real and older than this module. Functions that take a
/// `&Database` lock it themselves — `terrazgo_geo::fetch` does it on every call,
/// because its contract is that the lock is *released* across network I/O — so a
/// caller already holding a guard deadlocks on re-entry. `std::sync::Mutex` is
/// not reentrant, and a deadlock does not fail a test, it hangs it: one such
/// mistake cost 53 minutes of a test run before anyone looked at what was
/// actually stuck. This makes it fail in the first millisecond instead.
///
/// A `Database` is identified by its address. That is sound precisely while it
/// matters: `lock` returns a guard borrowing `&self`, so the value cannot move
/// while any guard is alive, and once no guard is alive there is nothing
/// registered under that address to confuse.
#[cfg(debug_assertions)]
mod reentrancy {
    use super::Database;
    use std::cell::RefCell;

    thread_local! {
        /// Addresses of the `Database`s this thread currently holds a guard for.
        /// At most two in this app (the record book and the geo cache).
        static HELD: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    }

    fn identity(db: &Database) -> usize {
        std::ptr::from_ref(db) as usize
    }

    pub(super) fn check(db: &Database) {
        let key = identity(db);
        HELD.with(|held| {
            assert!(
                !held.borrow().contains(&key),
                "this thread already holds this database's lock, and \
                 std::sync::Mutex is not reentrant — locking again here would \
                 deadlock. Release the guard first (scope it in a block, or \
                 drop it) before calling anything that takes the lock itself."
            );
        });
    }

    pub(super) fn register(db: &Database) -> usize {
        let key = identity(db);
        HELD.with(|held| held.borrow_mut().push(key));
        key
    }

    pub(super) fn release(key: usize) {
        HELD.with(|held| {
            let mut held = held.borrow_mut();
            if let Some(at) = held.iter().rposition(|&k| k == key) {
                held.remove(at);
            }
        });
    }
}

/// The database, locked.
///
/// `conn` / `conn_mut` follow the standard library's pairing (`get`/`get_mut`,
/// `borrow`/`borrow_mut`) so a caller says which one it needs. `Deref` would
/// read more naturally still, the way `MutexGuard` itself does, but
/// `deref(&self) -> &Connection` cannot return a `Result`, so a closed database
/// would have to panic — `MutexGuard` can only do it because it has no empty
/// state.
pub struct DbGuard<'a> {
    guard: MutexGuard<'a, Option<Connection>>,
    /// Debug-only: what [`reentrancy::release`] un-registers on drop. Absent
    /// from release builds, where the whole mechanism does not exist.
    #[cfg(debug_assertions)]
    key: usize,
}

impl<'a> DbGuard<'a> {
    // `db` is read only by the debug-only re-entrancy registration below, so in
    // a release build it is genuinely unused — that is the mechanism working,
    // not an oversight.
    #[cfg_attr(not(debug_assertions), allow(unused_variables))]
    fn new(db: &'a Database, guard: MutexGuard<'a, Option<Connection>>) -> Self {
        Self {
            guard,
            #[cfg(debug_assertions)]
            key: reentrancy::register(db),
        }
    }
}

/// Un-registers this thread's hold. Debug-only, like everything else about the
/// re-entrancy check — release builds give `DbGuard` no `Drop` at all, so the
/// mutex is released by `MutexGuard`'s own drop exactly as before.
#[cfg(debug_assertions)]
impl Drop for DbGuard<'_> {
    fn drop(&mut self) {
        reentrancy::release(self.key);
    }
}

impl DbGuard<'_> {
    /// The connection this guard holds.
    pub fn conn(&self) -> std::result::Result<&Connection, Unavailable> {
        self.guard.as_ref().ok_or(Unavailable::Closed)
    }

    /// The same connection, mutably — what `Connection::transaction` needs.
    pub fn conn_mut(&mut self) -> std::result::Result<&mut Connection, Unavailable> {
        self.guard.as_mut().ok_or(Unavailable::Closed)
    }

    /// Close the connection *without* releasing the lock. Idempotent.
    ///
    /// A backup import needs exactly this shape: the file closed so it can be
    /// overwritten, and the lock still held so nothing reaches the database
    /// between the close and the [`replace`](Self::replace) that reopens it.
    pub fn close(&mut self) -> rusqlite::Result<()> {
        match self.guard.take() {
            // The returned connection is dropped here, which retries the close.
            Some(conn) => conn.close().map_err(|(_, err)| err),
            None => Ok(()),
        }
    }

    /// Put a freshly opened connection in the slot.
    pub fn replace(&mut self, conn: Connection) {
        *self.guard = Some(conn);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use terrazgo_testkit::files::TempFile;

    fn sidecar(path: &Path, suffix: &str) -> PathBuf {
        let mut file = path.as_os_str().to_owned();
        file.push(suffix);
        PathBuf::from(file)
    }

    /// A file-backed database in WAL mode with one row written, so the WAL is
    /// not empty. In-memory databases have no sidecars to lose.
    fn open_wal(path: &Path) -> Connection {
        let conn = Connection::open(path).unwrap();
        conn.pragma_update(None, "journal_mode", "WAL").unwrap();
        conn.execute_batch("CREATE TABLE t (v TEXT); INSERT INTO t VALUES ('kept');")
            .unwrap();
        conn
    }

    fn one_row(conn: &Connection) -> String {
        conn.query_row("SELECT v FROM t", [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn closing_deletes_the_wal_sidecars_and_keeps_the_data() {
        let file = TempFile::reserve("database-close.db");
        let db = Database::new(open_wal(file.path())).unwrap();

        assert!(sidecar(file.path(), "-wal").exists());
        assert!(sidecar(file.path(), "-shm").exists());

        db.close().unwrap();

        // SQLite deletes both sidecars when the last connection closes
        // cleanly, and folds the write-ahead log back into the database.
        assert!(!sidecar(file.path(), "-wal").exists());
        assert!(!sidecar(file.path(), "-shm").exists());
        assert_eq!(one_row(&Connection::open(file.path()).unwrap()), "kept");
    }

    #[test]
    fn closing_twice_is_not_an_error() {
        let file = TempFile::reserve("database-close-twice.db");
        let db = Database::new(open_wal(file.path())).unwrap();

        db.close().unwrap();
        db.close().unwrap();
    }

    #[test]
    fn a_closed_database_reports_itself_rather_than_panicking() {
        let file = TempFile::reserve("database-closed-access.db");
        let db = Database::new(open_wal(file.path())).unwrap();
        db.close().unwrap();

        let mut guard = db.lock().unwrap();
        assert!(matches!(guard.conn(), Err(Unavailable::Closed)));
        assert!(matches!(guard.conn_mut(), Err(Unavailable::Closed)));
    }

    #[test]
    fn replace_puts_the_database_back_in_service() {
        let file = TempFile::reserve("database-replace.db");
        let db = Database::new(open_wal(file.path())).unwrap();

        let mut guard = db.lock().unwrap();
        guard.close().unwrap();
        assert!(matches!(guard.conn(), Err(Unavailable::Closed)));

        // The backup import's shape: closed and reopened without ever
        // releasing the lock.
        guard.replace(Connection::open(file.path()).unwrap());
        assert_eq!(one_row(guard.conn().unwrap()), "kept");
    }

    /// The shape that hung a test run for 53 minutes: seed through a guard,
    /// then call something that takes the lock itself. In a debug build that is
    /// now a panic in the first millisecond instead of a hang.
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "not reentrant")]
    fn locking_twice_on_one_thread_panics_instead_of_hanging() {
        let file = TempFile::reserve("database-reentrant.db");
        let db = Database::new(open_wal(file.path())).unwrap();

        let _held = db.lock().unwrap();
        // Stands in for any function that takes a `&Database` and locks it
        // itself — which is every `terrazgo_geo::fetch` entry point.
        let _second = db.lock();
    }

    #[cfg(debug_assertions)]
    #[test]
    fn releasing_the_guard_makes_the_next_lock_fine() {
        let file = TempFile::reserve("database-reentrant-ok.db");
        let db = Database::new(open_wal(file.path())).unwrap();

        // Scoped, which is what the fix to the geo tests does.
        {
            let guard = db.lock().unwrap();
            assert_eq!(one_row(guard.conn().unwrap()), "kept");
        }
        let guard = db.lock().unwrap();
        assert_eq!(one_row(guard.conn().unwrap()), "kept");

        // And it stays correct over many acquire/release cycles — the
        // registration is popped, not leaked.
        drop(guard);
        for _ in 0..100 {
            let _ = db.lock().unwrap();
        }
    }

    /// Two databases are independent: holding one must not look like holding
    /// the other. The shell holds exactly this pair — the record book and the
    /// geo cache — and SIGPAC commands hold the first while the second is
    /// locked underneath them.
    #[cfg(debug_assertions)]
    #[test]
    fn holding_one_database_does_not_block_another() {
        let app = TempFile::reserve("database-two-app.db");
        let cache = TempFile::reserve("database-two-cache.db");
        let app_db = Database::new(open_wal(app.path())).unwrap();
        let cache_db = Database::new(open_wal(cache.path())).unwrap();

        let _app_guard = app_db.lock().unwrap();
        let cache_guard = cache_db.lock().unwrap();
        assert_eq!(one_row(cache_guard.conn().unwrap()), "kept");
    }

    /// The hardening is only worth applying if it actually stops these, and a
    /// PRAGMA returning `Ok` proves nothing — every assertion here reads the
    /// EFFECT back.
    #[test]
    fn hardening_blocks_what_a_hostile_database_file_would_use() {
        let conn = Connection::open_in_memory().unwrap();
        harden(&conn).unwrap();
        // Creation still succeeds under these flags — only USE is refused,
        // which is exactly why a contract test guards our own schema.
        conn.execute_batch(
            "CREATE TABLE t (v INTEGER);
             CREATE VIEW v AS SELECT 1 AS x;
             CREATE TRIGGER tr AFTER INSERT ON t BEGIN INSERT INTO t VALUES (99); END;",
        )
        .unwrap();

        conn.execute("INSERT INTO t VALUES (1)", []).unwrap();
        let planted: i64 = conn
            .query_row("SELECT COUNT(*) FROM t WHERE v = 99", [], |r| r.get(0))
            .unwrap();
        assert_eq!(planted, 0, "a trigger in an imported file must not fire");

        assert!(
            conn.query_row("SELECT x FROM v", [], |r| r.get::<_, i64>(0))
                .is_err(),
            "reading a view must be refused"
        );

        // DEFENSIVE: writable_schema is refused, and the pragma reports Ok
        // while doing nothing — so read it back rather than trust the call.
        let _ = conn.execute_batch("PRAGMA writable_schema=ON");
        let writable: i64 = conn
            .query_row("PRAGMA writable_schema", [], |r| r.get(0))
            .unwrap();
        assert_eq!(writable, 0, "writable_schema must stay off");
        assert!(
            conn.execute("UPDATE sqlite_schema SET sql = 'x'", [])
                .is_err(),
            "sqlite_schema must not be writable"
        );
    }

    /// The schema has 33 `ON DELETE CASCADE` clauses. Foreign-key actions look
    /// trigger-shaped, so this pins that turning triggers off does not turn
    /// them off — the single fact the hardening would be unsafe without.
    #[test]
    fn hardening_leaves_foreign_key_cascades_working() {
        let conn = Connection::open_in_memory().unwrap();
        harden(&conn).unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        conn.execute_batch(
            "CREATE TABLE parent (id INTEGER PRIMARY KEY);
             CREATE TABLE child (id INTEGER PRIMARY KEY,
                 parent_id INTEGER REFERENCES parent(id) ON DELETE CASCADE);
             INSERT INTO parent VALUES (1);
             INSERT INTO child VALUES (1, 1);",
        )
        .unwrap();

        conn.execute("DELETE FROM parent WHERE id = 1", []).unwrap();
        let orphans: i64 = conn
            .query_row("SELECT COUNT(*) FROM child", [], |r| r.get(0))
            .unwrap();
        assert_eq!(orphans, 0, "ON DELETE CASCADE must still fire");
    }

    /// The hardening must not undo the shutdown fix. `DEFENSIVE` disables
    /// `PRAGMA journal_mode=OFF`, so this also checks it does not disable
    /// journal-mode changes in general.
    #[test]
    fn hardening_keeps_wal_and_still_deletes_the_sidecars() {
        let file = TempFile::reserve("database-hardened-wal.db");
        let conn = Connection::open(file.path()).unwrap();
        harden(&conn).unwrap();
        // Hardened BEFORE asking for WAL: the ordering that could have failed.
        conn.pragma_update(None, "journal_mode", "WAL").unwrap();
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "wal");

        // ...and the downgrade DEFENSIVE exists to refuse, read back rather
        // than taken from the pragma's return value.
        let _ = conn.pragma_update(None, "journal_mode", "OFF");
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "wal", "nothing in SQL may drop us out of WAL");

        conn.execute_batch("CREATE TABLE t (v TEXT); INSERT INTO t VALUES ('kept');")
            .unwrap();
        assert!(sidecar(file.path(), "-wal").exists());

        Database::new(conn).unwrap().close().unwrap();
        assert!(!sidecar(file.path(), "-wal").exists());
        assert!(!sidecar(file.path(), "-shm").exists());
        assert_eq!(one_row(&Connection::open(file.path()).unwrap()), "kept");
    }

    #[test]
    fn a_poisoned_lock_is_reported_but_never_blocks_the_close() {
        let file = TempFile::reserve("database-poisoned.db");
        let db = Arc::new(Database::new(open_wal(file.path())).unwrap());

        let panicking = Arc::clone(&db);
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let _ = std::thread::spawn(move || {
            let _held = panicking.lock();
            panic!("a command panicked while holding the connection");
        })
        .join();
        std::panic::set_hook(previous);

        // Ordinary access refuses, because the data behind the lock may be
        // half-written...
        assert!(matches!(db.lock(), Err(Unavailable::Poisoned)));

        // ...but the close goes through anyway. One panicked command must not
        // be the reason a write-ahead log is left on disk.
        db.close().unwrap();
        assert!(!sidecar(file.path(), "-wal").exists());
        assert_eq!(one_row(&Connection::open(file.path()).unwrap()), "kept");
    }

    /// The hardening lives on the CONNECTION, not in the file — which is why
    /// [`harden`] is called at every open site rather than once when the
    /// database is created.
    ///
    /// Two consequences, and both matter. A hostile file cannot arrive with the
    /// hardening pre-disabled, because these settings are not stored in files
    /// at all. And the protection does not travel: any other tool opening the
    /// database — or a future open site here that forgets to call `harden` —
    /// gets none of it. Only `journal_mode` is in the file header.
    #[test]
    fn the_hardening_lives_on_the_connection_not_in_the_file() {
        let file = TempFile::reserve("database-persistence.db");
        {
            let conn = Connection::open(file.path()).unwrap();
            conn.pragma_update(None, "journal_mode", "WAL").unwrap();
            harden(&conn).unwrap();
            conn.execute_batch(
                "CREATE TABLE t (v INTEGER);
                 CREATE VIEW v AS SELECT 1 AS x;
                 CREATE TRIGGER tr AFTER INSERT ON t BEGIN INSERT INTO t VALUES (99); END;",
            )
            .unwrap();
            conn.close().map_err(|(_, e)| e).unwrap();
        }

        // A plain connection, hardening none of it.
        let conn = Connection::open(file.path()).unwrap();

        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "wal", "journal_mode IS in the file header");

        let trusted: i64 = conn
            .query_row("PRAGMA trusted_schema", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            trusted, 1,
            "trusted_schema is per-connection, back to its default"
        );

        conn.execute("INSERT INTO t VALUES (1)", []).unwrap();
        let planted: i64 = conn
            .query_row("SELECT COUNT(*) FROM t WHERE v = 99", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            planted, 1,
            "the trigger fires again once nobody disables it"
        );
        assert!(
            conn.query_row("SELECT x FROM v", [], |r| r.get::<_, i64>(0))
                .is_ok(),
            "the view is readable again once nobody disables it"
        );
    }
}
