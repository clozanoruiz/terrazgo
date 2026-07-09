# Backup & restore

Terrazgo stores everything in one SQLite file. Regulatory records must survive
a lost or broken device, so back up regularly and keep copies off the device
(USB stick, another computer).

## Where the live database lives

| OS | Path |
|---|---|
| Linux | `~/.local/share/org.terrazgo.desktop/terrazgo.db` |
| macOS | `~/Library/Application Support/org.terrazgo.desktop/terrazgo.db` |
| Windows | `%APPDATA%\org.terrazgo.desktop\terrazgo.db` |

While the app runs, `terrazgo.db-wal` and `terrazgo.db-shm` sidecar files may
exist next to it. **Never copy the live file by hand while the app is running**
— the copy can be torn or missing recent writes. Use the in-app export, which
takes a consistent snapshot (`VACUUM INTO`) and verifies it.

## Exporting a backup (in-app)

Status view → **Backup → Export backup** → choose a destination. The exported
file is a complete, self-contained database: verified after writing, no sidecar
files, safe to copy anywhere.

## Restoring

### In-app (preferred)

Status view → **Backup → Import backup** → pick the backup file → confirm.
The app:

1. validates the file (integrity check; backups from a *newer* app version are
   refused — update the app first; *older* backups are migrated forward);
2. saves a safety copy of the current database to
   `<data dir>/backups/pre-import-<timestamp>.db` — an accidental import is
   always reversible by importing that file back;
3. swaps the database and reloads.

### Manual (app not working, new device, …)

1. Close Terrazgo completely.
2. Go to the data directory for your OS (table above). Create it (and open the
   app once) if this is a fresh install.
3. Delete `terrazgo.db`, `terrazgo.db-wal` and `terrazgo.db-shm` if present.
4. Copy your backup file there and rename it to `terrazgo.db`.
5. Start Terrazgo. If the backup came from an older app version, the migration
   runner upgrades it automatically on startup.

A backup from a **newer** app version cannot be restored into an older app —
update the app first.
