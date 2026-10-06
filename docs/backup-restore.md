# Backup & restore

Terrazgo keeps everything in one SQLite file. The record book has to survive a
lost or broken device, so back up regularly and keep the copies somewhere else
(a USB stick, another computer).

## Where the live database lives

| OS | Path |
|---|---|
| Linux | `~/.local/share/org.terrazgo.app/terrazgo.db` |
| macOS | `~/Library/Application Support/org.terrazgo.app/terrazgo.db` |
| Windows | `%APPDATA%\org.terrazgo.app\terrazgo.db` |

While the app runs there are two more files next to it, `terrazgo.db-wal` and
`terrazgo.db-shm`. Closing the app normally folds them back into the database
and removes them, so a stopped Terrazgo leaves a single file. After a crash or
a forced kill they stay behind, and the next start recovers from them.

Don't copy the live file by hand while the app is running: the copy can be
damaged or miss recent changes. Use the in-app export instead. It takes a
consistent snapshot (`VACUUM INTO`) and checks it.

## Exporting a backup (in-app)

Status view → **Backup → Export backup** → choose where to save it. The file is
a complete database on its own, checked after writing, with no extra files. You
can copy it anywhere.

## Restoring

### In-app (preferred)

Status view → **Backup → Import backup** → pick the backup file → confirm.
The app then:

1. checks the file. It runs an integrity check, refuses a backup made by a
   *newer* version of the app (update the app first), and upgrades a backup
   made by an *older* one;
2. saves a copy of the current database to
   `<data dir>/backups/pre-import-<timestamp>.db`, so an import made by mistake
   can be undone by importing that file back. It keeps the three most recent
   copies and removes older ones after each import, so the folder doesn't keep
   growing;
3. replaces the database and reloads.

### By hand (the app won't start, a new device, …)

1. Close Terrazgo completely.
2. Go to the data directory for your OS (table above). On a fresh install,
   open the app once so the directory exists.
3. Delete `terrazgo.db`, `terrazgo.db-wal` and `terrazgo.db-shm` if they are
   there.
4. Copy your backup file there and rename it to `terrazgo.db`.
5. Start Terrazgo. A backup from an older version is upgraded on startup.

A backup from a newer version of the app can't be restored into an older one.
Update the app first.
