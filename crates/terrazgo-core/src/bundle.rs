// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The file two devices hand each other, and what it is made of
//! (docs/sync.md → Transport: a bundle file, framed so a link can reuse it).
//!
//! **The messages and the framing are separate on purpose.** [`SyncManifest`],
//! [`ChangeSetFrame`] and [`RowChange`] are plain serde types that say nothing
//! about files; the framing below writes them as gzipped NDJSON with a trailer.
//! A live transport later swaps the framing and keeps the messages, which is
//! the whole reason the merge was defined without reference to how deltas
//! travel.
//!
//! # Why NDJSON
//!
//! **The payload is already JSON text on disk.** `record_change.payload` is
//! TEXT holding `{"before":…,"after":…}`, and a bundle carries those bytes
//! verbatim. That is not convenience: the change-set hash is recomputable on
//! the far side only "provided payloads are kept byte for byte as received", so
//! any format that re-encodes — a typed column layout, a quoted delimiter
//! format — would let two devices hold one change set and compute two
//! different hashes for it.
//!
//! It also streams. One line is one change set, so neither side holds a
//! cooperative's year in memory, and a truncated file is valid up to the line
//! it was truncated at — which is what "file existence is not file readiness"
//! (docs/sync.md → Part 1) asks a receiver to be able to tell.
//!
//! # What is checked before anything is kept
//!
//! Reading the file ([`read_bundle`]): the format version, that every change
//! set lies between what the file starts from and what its sender holds, and —
//! once the last line has been read — the row count and a CRC32 over everything
//! before the trailer. Then against this database ([`apply_bundle`]): the
//! schema (below), the group, every stamp's distance from this device's clock,
//! and that the file starts no later than what this device holds
//! ([`refuse_if_incomplete`]). The rows go into the log, what the purge erased
//! is dropped or erased here too, and the registers settle after all of that,
//! inside ONE transaction, so a bundle that fails anywhere leaves nothing
//! behind.

use std::collections::{BTreeMap, BTreeSet};
// `Hasher` for twox-hash's `write`/`finish`, which it implements through the
// std trait rather than inherently.
use std::hash::Hasher as _;
use std::io::{BufRead, BufReader, Read, Write};

use flate2::Crc;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::date::now_utc_iso;
use crate::error::{CoreError, Result};
use crate::sync::{Hlc, VersionVector};

/// The bundle format's own version, which has nothing to do with the schema's.
/// A reader refuses what it does not know rather than guessing at it.
///
/// 2 since the purge (2026-10-02): a manifest says what the file starts from
/// and what its sender knows of every device.
pub const FORMAT_VERSION: u32 = 2;

/// The extension a bundle is saved under. Stamped to the second and naming the
/// device by the caller, per docs/sync.md → "The duplicate-name trap".
pub const EXTENSION: &str = "tzsync";

/// The first line: who is sending, what they are running, and what they hold.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncManifest {
    /// [`FORMAT_VERSION`] as the sender wrote it.
    pub format: u32,
    /// The sender's `user_version`. Delta exchange requires an equal one:
    /// a whole database can be migrated forward on import, a stream of row
    /// images cannot (docs/sync.md → Schema skew: equal versions only).
    pub schema_version: i64,
    /// A fingerprint of the sender's schema, which `schema_version` alone
    /// cannot stand in for while the project is pre-release: migration files
    /// are edited in place, so a column can be added WITHOUT the version
    /// moving. `import_backup` meets the same problem and answers it by
    /// probing the file's shape — a bundle has no file to probe, so the sender
    /// states it. See [`schema_digest`].
    pub schema_digest: String,
    /// The device that wrote this bundle.
    pub device: String,
    /// The holding it belongs to. A receiver that has joined another one
    /// refuses it — this is what stops a neighbour's file, handed over on the
    /// same memory stick, merging into the wrong book
    /// (docs/sync.md → Device identity).
    pub group: String,
    /// When, in UTC. For the farmer reading a folder of them, never for merge:
    /// the clock that orders change sets is each one's own [`Hlc`].
    pub created_at: String,
    /// What the sender holds, per device — **simultaneously "here is what I
    /// have" and "here is what I need"**, which is what makes two file copies
    /// a full bidirectional sync. Counted by what its database has held, so a
    /// change set the purge took out still counts.
    pub seen: VersionVector,
    /// What the file was trimmed to: it carries every change set past this
    /// that its sender holds, and nothing at or below it. Empty for a file
    /// carrying the whole log.
    ///
    /// A receiver holding less than this would be left without change sets
    /// the file's own were built on, so it refuses the file; one holding at
    /// least this much holds everything the sender held once the file applies
    /// — a number past `since` that the file does not carry is one its sender
    /// erased (docs/sync.md → What a database has held, and what a file starts
    /// from). Defaulted so that a file of an older format reads far enough to
    /// be refused for its version rather than as unreadable.
    #[serde(default)]
    pub since: VersionVector,
    /// What the sender knows each OTHER device holds: the highest `seen` it has
    /// heard from each, directly or passed on. What lets a phone know, through
    /// the laptop, that another phone holds a removal (docs/sync.md → What
    /// each device knows of the others).
    #[serde(default)]
    pub known: BTreeMap<String, VersionVector>,
}

/// One change set, as it travels: the fields every row of it shares, then its
/// rows.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangeSetFrame {
    /// The device that wrote it and its number there — together the change
    /// set's name, everywhere and forever.
    pub device: String,
    pub seq: i64,
    pub hlc: Hlc,
    pub changed_at: String,
    pub actor: Option<String>,
    pub rows: Vec<RowChange>,
}

/// One logged row. The per-row half of `record_change`; the rest is on the
/// frame.
///
/// **`payload` and `version_vector` are the stored TEXT, carried verbatim**
/// rather than parsed and re-serialised. Re-encoding would change nothing a
/// reader can see and everything the change-set hash depends on.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RowChange {
    pub id: String,
    pub entity_table: String,
    pub entity_id: String,
    pub season_id: Option<String>,
    pub operation: String,
    pub root_table: String,
    pub root_id: String,
    pub version_vector: String,
    pub payload: String,
}

/// The last line: how much came before it, and a checksum of it.
///
/// The count is what gzip's own trailer cannot give — gzip checks that the
/// bytes it produced are the bytes that went in, not that all of them were
/// written before the writer stopped.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleTrailer {
    pub change_sets: usize,
    pub rows: usize,
    /// CRC32 over every byte of the uncompressed stream before this line.
    pub crc32: u32,
}

/// One line of the container. Tagged, so a reader can tell the kinds apart
/// without counting lines, and so a later kind is a new variant rather than a
/// new format version.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Line {
    Manifest(SyncManifest),
    ChangeSet(ChangeSetFrame),
    Trailer(BundleTrailer),
}

/// What an export wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ExportSummary {
    pub change_sets: usize,
    pub rows: usize,
}

/// A device's schema as a single value: XXH3-64 over the DDL of every table
/// and index, in name order.
///
/// Two devices running the same build ran the same migration files, so their
/// stored DDL is byte-identical — comments included, since SQLite keeps the
/// text it was given. Two devices whose migration files differ produce
/// different digests even at one `user_version`, which is exactly the
/// pre-release hazard `import_backup` guards against by probing shape.
pub fn schema_digest(conn: &Connection) -> Result<String> {
    let mut stmt = conn.prepare(
        "SELECT sql FROM sqlite_schema
         WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%'
         ORDER BY name",
    )?;
    let mut hasher = twox_hash::XxHash3_64::new();
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let sql: String = row.get(0)?;
        hasher.write(sql.as_bytes());
        // A separator no DDL text contains, so two statements cannot be
        // rearranged into the same byte stream.
        hasher.write(&[0]);
    }
    Ok(format!("{:016x}", hasher.finish()))
}

/// The `user_version` this database is at.
pub(crate) fn schema_version(conn: &Connection) -> Result<i64> {
    Ok(conn.pragma_query_value(None, "user_version", |r| r.get(0))?)
}

/// A change set's content as one value: XXH3-64 over its rows, by `id`.
///
/// **This is an identity check, not tamper-evidence.** It answers "do your
/// `(D, 51)` and mine say the same thing", which is how a peer tells a change
/// set it already holds from two devices writing under one identity — the
/// rewind path docs/sync.md records as unpreventable. A forged bundle
/// recomputes it; there is no key here and nothing claims otherwise.
///
/// Computed rather than stored, on both sides, from the same fields: a stored
/// digest would need a closing call at every write site, and any replica can
/// recompute this one from the rows it holds.
///
/// Ordered by `id` here rather than left in whatever order the caller read
/// them. Two replicas do hold a set's rows in one order — the ids are UUIDv7s
/// minted by one process and carried verbatim — but making the digest depend
/// on a caller's `ORDER BY` would be a way for two correct implementations to
/// disagree.
pub fn change_set_hash(rows: &[RowChange]) -> String {
    let mut ordered: Vec<&RowChange> = rows.iter().collect();
    ordered.sort_by(|left, right| left.id.cmp(&right.id));
    let mut hasher = twox_hash::XxHash3_64::new();
    for row in ordered {
        for field in [
            row.id.as_str(),
            row.entity_table.as_str(),
            row.entity_id.as_str(),
            row.season_id.as_deref().unwrap_or(""),
            row.operation.as_str(),
            row.root_table.as_str(),
            row.root_id.as_str(),
            row.version_vector.as_str(),
            row.payload.as_str(),
        ] {
            hasher.write(field.as_bytes());
            hasher.write(&[0]);
        }
    }
    format!("{:016x}", hasher.finish())
}

/// Every change set this device holds that `seen` does not, oldest first.
///
/// Scoped in SQL per device against the leading pair of `record_change`'s
/// UNIQUE `(origin_device, origin_seq, …)` — the key slice 1 chose for exactly
/// this question, in place of an index of its own. A device absent from `seen`
/// has been seen up to 0, so everything of its goes.
fn undelivered(conn: &Connection, seen: &VersionVector) -> Result<Vec<ChangeSetFrame>> {
    let mut devices = conn.prepare("SELECT DISTINCT origin_device FROM record_change")?;
    let devices: Vec<String> = devices
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;

    let mut stmt = conn.prepare(
        "SELECT origin_device, origin_seq, hlc, changed_at, actor,
                id, entity_table, entity_id, season_id, operation,
                root_table, root_id, version_vector, payload
         FROM record_change
         WHERE origin_device = ?1 AND origin_seq > ?2
         ORDER BY origin_seq, id",
    )?;
    let mut frames: BTreeMap<(String, i64), ChangeSetFrame> = BTreeMap::new();
    for device in devices {
        let from = seen.get(&device);
        let mut rows = stmt.query(rusqlite::params![device, from])?;
        while let Some(row) = rows.next()? {
            let seq: i64 = row.get("origin_seq")?;
            let change = RowChange {
                id: row.get("id")?,
                entity_table: row.get("entity_table")?,
                entity_id: row.get("entity_id")?,
                season_id: row.get("season_id")?,
                operation: row.get("operation")?,
                root_table: row.get("root_table")?,
                root_id: row.get("root_id")?,
                version_vector: row.get("version_vector")?,
                payload: row.get("payload")?,
            };
            // The set's shared fields come off its first row rather than being
            // re-read from every one of them: they are the same on all of a
            // set's rows by construction (`audit::begin` computes them once).
            match frames.get_mut(&(device.clone(), seq)) {
                Some(frame) => frame.rows.push(change),
                None => {
                    frames.insert(
                        (device.clone(), seq),
                        ChangeSetFrame {
                            device: device.clone(),
                            seq,
                            hlc: row.get("hlc")?,
                            changed_at: row.get("changed_at")?,
                            actor: row.get("actor")?,
                            rows: vec![change],
                        },
                    );
                }
            }
        }
    }
    // Clock order across devices, so a reader meets causes before effects —
    // an HLC is causality-respecting, which is the one thing it guarantees.
    let mut frames: Vec<ChangeSetFrame> = frames.into_values().collect();
    frames.sort_by(|left, right| {
        left.hlc
            .cmp(&right.hlc)
            .then_with(|| left.device.cmp(&right.device))
    });
    Ok(frames)
}

/// What this device has seen of every device, itself included — the vector a
/// manifest carries. What the log holds, raised to what the database has held:
/// a change set the purge took out of the log was held all the same, and a
/// manifest that stopped claiming it would have it sent back.
pub fn seen_by(conn: &Connection) -> Result<VersionVector> {
    let mut stmt =
        conn.prepare("SELECT origin_device, MAX(origin_seq) FROM record_change GROUP BY 1")?;
    let mut vector = VersionVector::default();
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let device: String = row.get(0)?;
        let seq: i64 = row.get(1)?;
        vector.observe(&device, seq);
    }
    let mut held = conn.prepare("SELECT device, through FROM sync_held")?;
    let mut rows = held.query([])?;
    while let Some(row) = rows.next()? {
        let device: String = row.get(0)?;
        let through: i64 = row.get(1)?;
        vector.observe(&device, through);
    }
    Ok(vector)
}

/// Write a bundle of everything `seen` has not got.
///
/// `device` is this device's identity, which the manifest names so the far side
/// knows who is asking. The writer streams: no part of the bundle is held in
/// memory beyond the change set being written.
///
/// **It writes to the database**, once and only ever once: a device with no
/// sync group mints one here (`sync::ensure_sync_group`). Exporting is the
/// first moment a device claims to be a holding's device, and therefore the
/// first moment the question has an answer.
///
/// **The frames and the manifest's `seen` are read in one snapshot.** A
/// receiver decides whether the file is complete by comparing what it holds
/// against `seen` ([`refuse_if_incomplete`]), so `seen` has to describe exactly
/// the log the frames were drawn from. A write landing between the two reads
/// would make the file claim a set it does not carry, and every device would
/// refuse it. The shell's one connection behind a mutex means none can land
/// today; the read transaction says so rather than relying on it.
pub fn write_bundle(
    conn: &Connection,
    device: &str,
    seen: &VersionVector,
    out: impl Write,
) -> Result<ExportSummary> {
    // Minted first, because it is the one write here and the snapshot below is
    // for reading.
    let group = crate::sync::ensure_sync_group(conn)?;
    // `unchecked_transaction` opens a transaction on a shared `&Connection`.
    // rusqlite's ordinary `transaction()` wants `&mut` so the compiler can rule
    // out a second one opening inside it; this one checks that at run time
    // instead, which is all an export that only reads needs.
    let snapshot = conn.unchecked_transaction()?;
    let frames = undelivered(&snapshot, seen)?;
    let manifest = SyncManifest {
        format: FORMAT_VERSION,
        schema_version: schema_version(&snapshot)?,
        schema_digest: schema_digest(&snapshot)?,
        device: device.to_owned(),
        group,
        created_at: now_utc_iso(),
        seen: seen_by(&snapshot)?,
        since: seen.clone(),
        known: crate::sync::known(&snapshot)?,
    };
    // Nothing was written inside it, so ending it by rolling back is the same
    // as committing — and is what dropping it does.
    drop(snapshot);

    let mut encoder = GzEncoder::new(out, flate2::Compression::default());
    let mut crc = Crc::new();
    let written = |line: &Line, crc: &mut Crc, out: &mut GzEncoder<_>| -> Result<()> {
        let mut bytes = serde_json::to_vec(line)?;
        bytes.push(b'\n');
        crc.update(&bytes);
        out.write_all(&bytes)?;
        Ok(())
    };

    written(&Line::Manifest(manifest), &mut crc, &mut encoder)?;
    let mut rows = 0usize;
    for frame in &frames {
        rows += frame.rows.len();
        written(&Line::ChangeSet(frame.clone()), &mut crc, &mut encoder)?;
    }
    // The trailer's own bytes are not in the checksum they carry.
    let trailer = BundleTrailer {
        change_sets: frames.len(),
        rows,
        crc32: crc.sum(),
    };
    let mut bytes = serde_json::to_vec(&Line::Trailer(trailer))?;
    bytes.push(b'\n');
    encoder.write_all(&bytes)?;
    encoder.finish()?;

    Ok(ExportSummary {
        change_sets: frames.len(),
        rows,
    })
}

/// A bundle read back: the manifest, and the change sets in the order they
/// were written.
///
/// Produced only by [`read_bundle`], which returns it **after** the trailer has
/// verified — so holding one is holding a complete bundle.
#[derive(Debug, Clone)]
pub struct Bundle {
    pub manifest: SyncManifest,
    pub change_sets: Vec<ChangeSetFrame>,
}

/// What people call the device a bundle came from — so a screen can say "Móvil
/// de Juan" where the manifest says `0192f3a4-…`.
///
/// The name this device has for it first: a device named here, or named
/// elsewhere and synced. Otherwise the name the bundle itself carries: a
/// device registers itself as a `sync_peer` register when it opens its
/// database, so the sender's own row travels in any bundle holding its whole
/// log, with the name somebody gave it — as the latest change set in the
/// bundle left it. `None` for a device nobody has named yet.
///
/// Read without applying anything, so an import can name the file's sender
/// before asking whether to apply it.
pub fn sender_label(conn: &Connection, bundle: &Bundle) -> Result<Option<String>> {
    let device = &bundle.manifest.device;
    let known: Option<String> = conn
        .query_row(
            "SELECT label FROM sync_peer WHERE id = ?1",
            [device],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    if known.is_some() {
        return Ok(known);
    }
    let mut carried: Option<(&Hlc, Option<String>)> = None;
    for frame in &bundle.change_sets {
        for row in &frame.rows {
            if row.entity_table != "sync_peer" || &row.entity_id != device {
                continue;
            }
            if carried.as_ref().is_some_and(|(hlc, _)| *hlc > &frame.hlc) {
                continue;
            }
            let payload: serde_json::Value = serde_json::from_str(&row.payload)?;
            let label = payload["after"]["label"].as_str().map(str::to_owned);
            carried = Some((&frame.hlc, label));
        }
    }
    Ok(carried.and_then(|(_, label)| label))
}

/// Read and verify a bundle.
///
/// Refuses, in the order it can: a format version it does not know, a manifest
/// that is not the first line, a line it cannot parse, a change set its own
/// manifest says the sender does not hold, a missing trailer, and a count or
/// checksum that does not match what came before it.
///
/// It does NOT check the schema, the clock, the change-set hashes or whether
/// the file is complete for this device — those compare the bundle against a
/// particular database, which is [`apply_bundle`]'s business, not the
/// container's.
pub fn read_bundle(input: impl Read) -> Result<Bundle> {
    let mut reader = BufReader::new(GzDecoder::new(input));
    let mut crc = Crc::new();
    let mut manifest: Option<SyncManifest> = None;
    let mut change_sets: Vec<ChangeSetFrame> = Vec::new();
    let mut rows = 0usize;

    loop {
        let mut bytes = Vec::new();
        // Read the raw bytes INCLUDING the newline: the checksum was taken
        // over what the writer wrote, and a reader that re-adds a newline of
        // its own choosing would disagree about a `\r`.
        if reader.read_until(b'\n', &mut bytes)? == 0 {
            return Err(CoreError::Invalid("bundle_incomplete"));
        }
        let line: Line =
            serde_json::from_slice(&bytes).map_err(|_| CoreError::Invalid("bundle_unreadable"))?;
        match line {
            Line::Manifest(found) => {
                if manifest.is_some() || !change_sets.is_empty() {
                    return Err(CoreError::Invalid("bundle_unreadable"));
                }
                if found.format != FORMAT_VERSION {
                    return Err(CoreError::Invalid("bundle_format_unsupported"));
                }
                crc.update(&bytes);
                manifest = Some(found);
            }
            Line::ChangeSet(frame) => {
                let Some(stated) = &manifest else {
                    return Err(CoreError::Invalid("bundle_unreadable"));
                };
                // The frames are drawn from the log the manifest describes, so
                // one past what it says the sender holds, or at or below what
                // it says the file starts from, was not written by this app —
                // and would have the completeness check reasoning from a claim
                // the file itself contradicts.
                if frame.seq > stated.seen.get(&frame.device)
                    || frame.seq <= stated.since.get(&frame.device)
                {
                    return Err(CoreError::Invalid("bundle_unreadable"));
                }
                crc.update(&bytes);
                rows += frame.rows.len();
                change_sets.push(frame);
            }
            Line::Trailer(trailer) => {
                let Some(manifest) = manifest else {
                    return Err(CoreError::Invalid("bundle_unreadable"));
                };
                if trailer.change_sets != change_sets.len()
                    || trailer.rows != rows
                    || trailer.crc32 != crc.sum()
                {
                    return Err(CoreError::Invalid("bundle_incomplete"));
                }
                // Read past the trailer before believing it. Two things live
                // out here: gzip's OWN trailer, a CRC and a length over the
                // whole uncompressed stream, which the decoder only checks
                // when something reads to the end — return at the trailer line
                // and a file whose last bytes were lost passes every check
                // above, because no DATA was missing. And anything after the
                // trailer, which a bundle this app wrote does not have.
                let mut after = Vec::new();
                match reader.read_until(b'\n', &mut after) {
                    Ok(0) => {}
                    Ok(_) => return Err(CoreError::Invalid("bundle_unreadable")),
                    Err(_) => return Err(CoreError::Invalid("bundle_incomplete")),
                }
                return Ok(Bundle {
                    manifest,
                    change_sets,
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Applying one
// ---------------------------------------------------------------------------

/// Every column of `record_change`, as a carried row is rebuilt on the far
/// side. Fixed text, so it reaches the statement cache — an import runs it
/// once per row.
const INSERT_ROW_SQL: &str = "INSERT INTO record_change
       (id, entity_table, entity_id, season_id, operation, changed_at, actor, payload,
        root_table, root_id, origin_device, origin_seq, version_vector, hlc)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)";

/// One change set this device already holds, read back for the hash comparison.
const HELD_SET_SQL: &str = "SELECT id, entity_table, entity_id, season_id, operation,
            root_table, root_id, version_vector, payload
     FROM record_change
     WHERE origin_device = ?1 AND origin_seq = ?2";

/// What an import did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportSummary {
    /// The device that sent the bundle.
    pub peer: String,
    /// What the peer says it holds — which is also what it is asking for, so
    /// a caller can answer with a bundle of its own without another round trip.
    pub peer_seen: VersionVector,
    pub change_sets_applied: usize,
    /// The books (`season` ids) the applied change sets wrote into, sorted —
    /// not those of change sets already held, so a whole-log file names only
    /// what it changed. What a caller reads to ask what the file brought, such
    /// as a code this device's catalogues cannot name (docs/sync.md → What
    /// stays device-local).
    pub books: Vec<String>,
    /// Change sets that were already here, byte for byte. An ordinary result:
    /// two devices exchanging bundles both carry what the other already has.
    pub change_sets_already_held: usize,
    pub rows: usize,
    /// Registers brought in line with the log.
    pub registers_settled: usize,
    /// Of those, how many are waiting for a person — in the review queue,
    /// which leaves out two branches that say the same thing (docs/sync.md →
    /// Versions that say the same thing are not listed).
    pub conflicts: usize,
    /// What this import erased here, as a purge made elsewhere asked.
    pub purged: crate::repository::PurgeSummary,
    /// Changes made to a record the purge had erased, by a device that never
    /// had its removal — a retired phone come back, or one never heard from.
    /// Not applied, here or anywhere, and named so the screen can say so
    /// (docs/sync.md → An import meets the purge).
    pub discarded: Vec<DiscardedChanges>,
}

/// The changes one device made to records erased for good, which an import
/// left out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiscardedChanges {
    pub device: String,
    /// What people call it, as this device knows it once the file has applied
    /// — so a name the file carried counts. `None` for a device nobody named,
    /// or never heard from.
    pub device_label: Option<String>,
    /// How many of its change sets — each one save, by one person.
    pub changes: usize,
}

/// Apply a verified bundle to this database.
///
/// `now_ms` is this device's wall clock, taken by the caller the way
/// [`Hlc::next`] takes it — so the clock check can be tested without a device
/// whose date is wrong.
///
/// **Everything happens in one transaction and a refusal leaves nothing
/// behind.** The rows go into the log as they are read and the registers
/// settle afterwards; a failure anywhere rolls back both, which is what
/// "verified before anything is applied" amounts to without holding the whole
/// delivery in memory. Foreign keys are deferred for the reason
/// docs/sync.md gives: a register may reference a row another device created,
/// so no order of rows satisfies every constraint along the way.
///
/// What it refuses, and why each is not a constraint failure:
///
/// * a different schema — `bundle_schema_mismatch`, because a stream of row
///   images cannot be migrated forward the way a whole database can;
/// * another holding's book — `sync_not_paired` when this device has joined
///   no group yet, `sync_group_mismatch` when it has joined a different one;
/// * a stamp beyond ε — [`CoreError::PeerClockAhead`], naming the device and
///   the offset;
/// * a file that starts later than what this device holds —
///   `bundle_skips_changes`, from [`refuse_if_incomplete`], because applying it
///   would deliver changes without what they were built on;
/// * a change set it already holds whose content differs — two devices writing
///   under one identity (`device_identity_shared`);
/// * whatever the merge refuses: a season label collision, a row filed under
///   the wrong register, a register with no head.
///
/// **What the purge erased stays erased** (docs/sync.md → An import meets the
/// purge). A register named by a `purged_register` row — held here, or arriving
/// in this file — takes from the file only what was written on top of its
/// removal, by devices holding nothing older of it; what the removal had seen is dropped, and
/// what never saw it is discarded and named in the summary. A marker arriving
/// does the same to what is already here.
pub fn apply_bundle(conn: &mut Connection, bundle: &Bundle, now_ms: i64) -> Result<ImportSummary> {
    if bundle.manifest.schema_version != schema_version(conn)?
        || bundle.manifest.schema_digest != schema_digest(conn)?
    {
        return Err(CoreError::Invalid("bundle_schema_mismatch"));
    }
    // Whose book this is, before whose clock is wrong: a file from the
    // neighbour's holding is not a clock problem however far out its clock is.
    // Both refusals are answered by the same act — `sync::join_sync_group` —
    // and they are told apart because how loudly to ask before doing it is not
    // the same question. Joining an unpaired device is the ordinary first
    // sync; leaving one group for another is not.
    match crate::sync::sync_group(conn)? {
        None => return Err(CoreError::Invalid("sync_not_paired")),
        Some(ours) if ours != bundle.manifest.group => {
            return Err(CoreError::Invalid("sync_group_mismatch"));
        }
        Some(_) => {}
    }
    // Before the transaction: nothing here reads the database, and a refusal
    // should not have opened one.
    for frame in &bundle.change_sets {
        if !frame.hlc.within_skew(now_ms) {
            return Err(CoreError::PeerClockAhead {
                peer: bundle.manifest.device.clone(),
                ahead_ms: frame.hlc.ahead_of(now_ms),
            });
        }
    }
    // After the clock: a new file from a device whose clock is wrong is still
    // wrong, so that is the refusal to fix first.
    refuse_if_incomplete(conn, bundle)?;

    let tx = conn.transaction()?;
    tx.execute_batch("PRAGMA defer_foreign_keys = ON")?;
    let this_device = crate::sync::installed_device(&tx)?;
    let mut markers = Markers::gather(&tx, bundle)?;

    let mut applied = 0usize;
    let mut already_held = 0usize;
    let mut rows = 0usize;
    let mut discarded: BTreeMap<String, BTreeSet<i64>> = BTreeMap::new();
    let mut arriving: Vec<(&ChangeSetFrame, Vec<&RowChange>)> = Vec::new();
    for frame in &bundle.change_sets {
        match held_set(&tx, &frame.device, frame.seq)? {
            Some(held) => {
                // Compared on the registers no purge names: one side may have
                // erased a register the other still holds rows of.
                let mut mine = Vec::new();
                for row in held {
                    if !markers.names(&tx, &row.root_table, &row.root_id)? {
                        mine.push(row);
                    }
                }
                let mut theirs = Vec::new();
                for row in &frame.rows {
                    if !markers.names(&tx, &row.root_table, &row.root_id)? {
                        theirs.push(row.clone());
                    }
                }
                if change_set_hash(&mine) != change_set_hash(&theirs) {
                    return Err(CoreError::Invalid("device_identity_shared"));
                }
                already_held += 1;
            }
            None => {
                let mut kept = Vec::new();
                for row in &frame.rows {
                    match markers.verdict(&tx, row, &frame.device, frame.seq)? {
                        Verdict::Unmarked | Verdict::OnTop => kept.push(row),
                        Verdict::Erased => {}
                        Verdict::Discarded => {
                            discarded
                                .entry(frame.device.clone())
                                .or_default()
                                .insert(frame.seq);
                        }
                    }
                }
                // A change set nothing of which is kept is held all the same —
                // what this database has held of its device is raised below.
                if !kept.is_empty() {
                    arriving.push((frame, kept));
                }
            }
        }
    }

    // The head each touched register was showing BEFORE the delivery, which is
    // what `settle` rewinds out of and which nothing records — so it has to be
    // read while it is still true.
    let mut registers: BTreeMap<(String, String), Option<crate::merge::Head>> = BTreeMap::new();
    for (_, kept) in &arriving {
        for row in kept {
            let key = (row.root_table.clone(), row.root_id.clone());
            // `entry` rather than a lookup and an insert: reading the live head
            // is a query, so it must happen only for a register not already
            // recorded — and it can fail, which `or_insert_with` cannot carry.
            if let std::collections::btree_map::Entry::Vacant(slot) = registers.entry(key) {
                let (table, id) = slot.key();
                let live = crate::merge::live_head(&crate::merge::heads(&tx, table, id)?).cloned();
                slot.insert(live);
            }
        }
    }

    let mut books: BTreeSet<String> = BTreeSet::new();
    {
        let mut insert = crate::sql::cached_statement(&tx, INSERT_ROW_SQL)?;
        for (frame, kept) in &arriving {
            applied += 1;
            rows += kept.len();
            for row in kept {
                if let Some(book) = &row.season_id {
                    books.insert(book.clone());
                }
                insert.execute(rusqlite::params![
                    row.id,
                    row.entity_table,
                    row.entity_id,
                    row.season_id,
                    row.operation,
                    frame.changed_at,
                    frame.actor,
                    row.payload,
                    row.root_table,
                    row.root_id,
                    frame.device,
                    frame.seq,
                    row.version_vector,
                    frame.hlc,
                ])?;
            }
        }
    }

    // What the purge erased, erased here too: the registers a marker names that
    // still hold something the removal had seen, or that never saw it.
    let purged = markers.erase_covered(&tx, &mut registers, &mut discarded)?;
    let (registers_settled, conflicts) = settle_all(&tx, registers)?;
    let discarded = discarded
        .into_iter()
        .map(|(device, sets)| {
            let device_label: Option<String> = tx
                .query_row(
                    "SELECT label FROM sync_peer WHERE id = ?1",
                    [&device],
                    |row| row.get(0),
                )
                .optional()?
                .flatten();
            Ok(DiscardedChanges {
                device,
                device_label,
                changes: sets.len(),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    // The file applied, so this device now holds everything its sender held —
    // change sets the sender erased included — and the knowledge it carried
    // is true here.
    crate::sync::raise_held(&tx, &bundle.manifest.seen)?;
    crate::sync::raise_known(&tx, &bundle.manifest.device, &bundle.manifest.seen)?;
    for (device, seen) in &bundle.manifest.known {
        if device != &this_device {
            crate::sync::raise_known(&tx, device, seen)?;
        }
    }
    tx.commit()?;

    Ok(ImportSummary {
        peer: bundle.manifest.device.clone(),
        peer_seen: bundle.manifest.seen.clone(),
        change_sets_applied: applied,
        books: books.into_iter().collect(),
        change_sets_already_held: already_held,
        rows,
        registers_settled,
        conflicts,
        purged,
        discarded,
    })
}

/// Refuse a file unless, once it is applied, this device holds every change set
/// its sender held (docs/sync.md → A trimmed reply is safe only for the device
/// it answers).
///
/// **Why the whole sender, and not each arriving set's causes.** A reply is
/// trimmed to what the device it answers already holds, so imported anywhere
/// else it can arrive without what it was built on. A register's version vector
/// cannot catch that on its own: it counts only the change sets that touched
/// its register, so a plot built on a farm another device opened carries a
/// vector naming the plot's writer alone. What CAN be checked is the file
/// against its sender. Every log here holds every set's causes — local writes
/// keep that, and so does this check — so a device that ends up holding
/// everything its sender held holds every cause of everything it received,
/// within a register or across them.
///
/// **Checked against what the file starts from** (since slice 11): the file
/// carries every change set its sender holds past `since`, so a device holding
/// at least `since` of every device holds, afterwards, everything the sender
/// held. A number past `since` the file does not carry is one its sender erased
/// — which is why this no longer walks the frames for a hole: since the purge,
/// a hole is ordinary (docs/sync.md → What a database has held, and what a file
/// starts from). What this device holds is counted by
/// [`crate::sync::held_through`], which no erasure lowers.
///
/// A reply imported by the device it answered passes, as does every whole-log
/// bundle and a reply given to a device already holding everything the one it
/// answered held.
///
/// Public, and reading only the file and one key per device, so the import
/// screen runs it before asking anything: a file that cannot apply must not
/// lead a farmer into leaving one group for another.
pub fn refuse_if_incomplete(conn: &Connection, bundle: &Bundle) -> Result<()> {
    for (device, from) in bundle.manifest.since.iter() {
        if crate::sync::held_through(conn, device)? < from {
            return Err(CoreError::Invalid("bundle_skips_changes"));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// What the purge erased
// ---------------------------------------------------------------------------

/// The `purged_register` rows that bear on this import: those this database
/// holds for the registers the file touches, and those the file carries.
/// Looked up once per register, on `idx_purged_register_root`.
struct Markers {
    /// Every register looked up so far, and the purges naming it — empty for a
    /// register no purge erased.
    by_register: BTreeMap<(String, String), Vec<Marker>>,
}

/// One purge of a register: the version that removed it, and the change set
/// the purge was written in.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Marker {
    removal: VersionVector,
    device: String,
    seq: i64,
}

/// What one arriving row is to a register the purge may have erased.
enum Verdict {
    /// No purge names its register.
    Unmarked,
    /// Its change set was written by a device that held a purge of the
    /// register: something new in it — a slot filled again — written knowing
    /// the old was gone. Applied.
    OnTop,
    /// The removal had seen it: erased history. Dropped.
    Erased,
    /// Written where no purge of the register had arrived: a correction, a
    /// restore, a choice — a book deleted more than thirty days ago is gone,
    /// whatever a device that had not heard so did to it. Discarded, and named.
    Discarded,
}

/// The purges naming one register, each with the change set it was written
/// in. A seek on `idx_purged_register_root`, and one on `idx_record_change_root`
/// for each purge — a register of its own.
const REGISTER_MARKERS_SQL: &str = "SELECT p.removal, c.origin_device, c.origin_seq
     FROM purged_register p
     JOIN record_change c ON c.root_table = 'purged_register' AND c.root_id = p.id
     WHERE p.root_table = ?1 AND p.root_id = ?2";

/// The distinct change sets one register's log holds, with their versions. A
/// seek on `idx_record_change_root`. This and the two below run only for a
/// register a marker names, so they are prepared for the import that needs
/// them rather than kept in the shared cache.
const REGISTER_SETS_SQL: &str = "SELECT DISTINCT origin_device, origin_seq, version_vector
     FROM record_change WHERE root_table = ?1 AND root_id = ?2";

/// One change set's rows in one register. A seek on the UNIQUE
/// `(origin_device, origin_seq, …)`.
const ERASE_SET_SQL: &str = "DELETE FROM record_change
     WHERE origin_device = ?1 AND origin_seq = ?2 AND root_table = ?3 AND root_id = ?4";

impl Markers {
    /// The markers the file carries, read off its frames, beside nothing else
    /// yet: the ones held here are looked up as each register comes up.
    fn gather(conn: &Connection, bundle: &Bundle) -> Result<Markers> {
        let mut markers = Markers {
            by_register: BTreeMap::new(),
        };
        let mut carried: Vec<((String, String), Marker)> = Vec::new();
        for frame in &bundle.change_sets {
            for row in &frame.rows {
                if row.entity_table != "purged_register" {
                    continue;
                }
                let payload: serde_json::Value = serde_json::from_str(&row.payload)?;
                let after = &payload["after"];
                let (Some(table), Some(id), Some(removal)) = (
                    after["root_table"].as_str(),
                    after["root_id"].as_str(),
                    after["removal"].as_str(),
                ) else {
                    return Err(CoreError::Invalid("bundle_unreadable"));
                };
                carried.push((
                    (table.to_owned(), id.to_owned()),
                    Marker {
                        removal: VersionVector::from_json(removal)?,
                        device: frame.device.clone(),
                        seq: frame.seq,
                    },
                ));
            }
        }
        for ((table, id), marker) in carried {
            markers.lookup(conn, &table, &id)?;
            if let Some(held) = markers.by_register.get_mut(&(table, id))
                && !held.contains(&marker)
            {
                held.push(marker);
            }
        }
        Ok(markers)
    }

    /// The purges naming a register, held here or carried — looked up the
    /// first time the register comes up.
    fn lookup(&mut self, conn: &Connection, table: &str, id: &str) -> Result<&[Marker]> {
        let key = (table.to_owned(), id.to_owned());
        if !self.by_register.contains_key(&key) {
            let mut stmt = crate::sql::cached_statement(conn, REGISTER_MARKERS_SQL)?;
            let mut rows = stmt.query([table, id])?;
            let mut held = Vec::new();
            while let Some(row) = rows.next()? {
                let text: String = row.get(0)?;
                held.push(Marker {
                    removal: VersionVector::from_json(&text)?,
                    device: row.get(1)?,
                    seq: row.get(2)?,
                });
            }
            self.by_register.insert(key.clone(), held);
        }
        Ok(self.by_register.get(&key).map(Vec::as_slice).unwrap_or(&[]))
    }

    /// Whether a purge names the register.
    fn names(&mut self, conn: &Connection, table: &str, id: &str) -> Result<bool> {
        Ok(!self.lookup(conn, table, id)?.is_empty())
    }

    /// What a change set `(device, seq)` written with `vector` is to a register.
    ///
    /// **The purge wins**: of what a purged register holds, only what was
    /// written knowing of a purge of it is kept — every stamp carries the
    /// purges it has seen ([`crate::audit`]). Everything else either is the
    /// history the removal had seen, or was written where no purge had
    /// arrived, and goes wherever a purge is; so a device holds a purged
    /// register's whole history (no purge has reached it) or none of it,
    /// never part.
    fn judge(markers: &[Marker], device: &str, seq: i64, vector: &VersionVector) -> Verdict {
        if markers.is_empty() {
            return Verdict::Unmarked;
        }
        if markers
            .iter()
            .any(|marker| marker.removal.has_seen(device, seq))
        {
            return Verdict::Erased;
        }
        if markers
            .iter()
            .any(|marker| vector.has_seen(&marker.device, marker.seq))
        {
            return Verdict::OnTop;
        }
        Verdict::Discarded
    }

    fn verdict(
        &mut self,
        conn: &Connection,
        row: &RowChange,
        device: &str,
        seq: i64,
    ) -> Result<Verdict> {
        let markers = self.lookup(conn, &row.root_table, &row.root_id)?;
        if markers.is_empty() {
            return Ok(Verdict::Unmarked);
        }
        let vector = VersionVector::from_json(&row.version_vector)?;
        Ok(Self::judge(markers, device, seq, &vector))
    }

    /// Erase, from what this database already holds, every change set of a
    /// marked register that is not on top of its removal — and bring the
    /// register's tables in line with what is left, or empty them when nothing
    /// is. Returns what was erased whole.
    ///
    /// A register handled here leaves `registers`: its tables are rebuilt from
    /// what remains of its log, with nothing materialised to rewind out of.
    fn erase_covered(
        &self,
        tx: &rusqlite::Transaction,
        registers: &mut BTreeMap<(String, String), Option<crate::merge::Head>>,
        discarded: &mut BTreeMap<String, BTreeSet<i64>>,
    ) -> Result<crate::repository::PurgeSummary> {
        let mut erased_whole: Vec<(String, String)> = Vec::new();
        // The books what goes whole was filed in, read before its log goes.
        let mut books: BTreeSet<String> = BTreeSet::new();
        if self.by_register.values().all(Vec::is_empty) {
            return Ok(crate::repository::PurgeSummary::default());
        }
        let mut register_sets = tx.prepare(REGISTER_SETS_SQL)?;
        let mut rows_named = tx.prepare(crate::merge::REGISTER_ROWS_NAMED_SQL)?;
        let mut erase = tx.prepare(ERASE_SET_SQL)?;
        for ((table, id), markers) in &self.by_register {
            if markers.is_empty() {
                continue;
            }
            let mut sets: Vec<(String, i64, VersionVector)> = Vec::new();
            {
                let mut rows = register_sets.query([table, id])?;
                while let Some(row) = rows.next()? {
                    let text: String = row.get(2)?;
                    sets.push((row.get(0)?, row.get(1)?, VersionVector::from_json(&text)?));
                }
            }
            let mut covered = Vec::new();
            let mut remaining = 0usize;
            for (device, seq, vector) in &sets {
                match Self::judge(markers, device, *seq, vector) {
                    Verdict::Erased => covered.push((device.clone(), *seq)),
                    Verdict::Discarded => {
                        covered.push((device.clone(), *seq));
                        discarded.entry(device.clone()).or_default().insert(*seq);
                    }
                    Verdict::OnTop | Verdict::Unmarked => remaining += 1,
                }
            }
            if covered.is_empty() {
                continue;
            }
            if remaining == 0 {
                books.extend(crate::repository::books_of(
                    tx,
                    [&(table.clone(), id.clone())],
                )?);
            }
            let named: Vec<(String, String)> = rows_named
                .query_map([table, id], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?;
            // Raised before they go, as the purge raises before it erases: a
            // change set erased here — this device's own among them — is never
            // numbered again.
            let mut going = VersionVector::default();
            for (device, seq) in &covered {
                going.observe(device, *seq);
            }
            crate::sync::raise_held(tx, &going)?;
            for (device, seq) in &covered {
                erase.execute(rusqlite::params![device, seq, table, id])?;
            }
            {
                // Every row the register ever held leaves the tables; what is
                // left of its log, if anything, puts back what it states.
                let mut applier = crate::merge::Applier::new(tx);
                for (entity_table, entity_id) in &named {
                    applier.delete(entity_table, entity_id)?;
                }
            }
            crate::sql::cached_statement(tx, crate::merge::CLEAR_CONFLICT_SQL)?
                .execute([table, id])?;
            registers.remove(&(table.clone(), id.clone()));
            if remaining == 0 {
                erased_whole.push((table.clone(), id.clone()));
            } else {
                // Rebuilt from what was written on top of the removal: written
                // where nothing older of the register was held, it is all
                // there is of it.
                crate::merge::settle(tx, table, id, None)?;
            }
        }
        crate::repository::PurgeSummary::of(tx, &erased_whole, &books)
    }
}

/// A change set this device already holds, as [`RowChange`]s, or `None`.
fn held_set(conn: &Connection, device: &str, seq: i64) -> Result<Option<Vec<RowChange>>> {
    let mut stmt = crate::sql::cached_statement(conn, HELD_SET_SQL)?;
    let mut rows = stmt.query(rusqlite::params![device, seq])?;
    let mut held = Vec::new();
    while let Some(row) = rows.next()? {
        held.push(RowChange {
            id: row.get("id")?,
            entity_table: row.get("entity_table")?,
            entity_id: row.get("entity_id")?,
            season_id: row.get("season_id")?,
            operation: row.get("operation")?,
            root_table: row.get("root_table")?,
            root_id: row.get("root_id")?,
            version_vector: row.get("version_vector")?,
            payload: row.get("payload")?,
        });
    }
    Ok((!held.is_empty()).then_some(held))
}

/// Settle every register the delivery touched, returning how many were settled
/// and how many are left conflicted.
///
/// **A season whose label collides is set aside and tried again**, because the
/// check reads the tables and therefore answers for one register rather than
/// for the bundle: a delivery where one book releases a name — renamed, or
/// deleted — and another takes it is applicable as a whole, but only in one
/// order, and the order registers settle in is their UUIDs'. Retried while any
/// pass makes progress, so a chain of releases resolves too; refused only when
/// a whole pass frees nothing (docs/sync.md → The collision check sees one
/// register, not the bundle).
///
/// Deferring is safe precisely because that check runs BEFORE anything is
/// written for its register — a refusal leaves the tables untouched, which no
/// other refusal here can promise.
fn settle_all(
    tx: &rusqlite::Transaction,
    registers: BTreeMap<(String, String), Option<crate::merge::Head>>,
) -> Result<(usize, usize)> {
    let mut pending: Vec<((String, String), Option<crate::merge::Head>)> =
        registers.into_iter().collect();
    let settled = pending.len();
    let mut conflicts = 0usize;
    while !pending.is_empty() {
        let before = pending.len();
        let mut deferred = Vec::new();
        let mut refused = None;
        for ((table, id), was_live) in pending {
            match crate::merge::settle(tx, &table, &id, was_live.as_ref()) {
                Ok(heads) => {
                    // Counted as the queue lists them: two branches that say
                    // the same thing are two heads and nothing to review.
                    if heads.len() > 1 && crate::merge::waiting(tx, &table, &id)? {
                        conflicts += 1;
                    }
                }
                Err(collision @ CoreError::SeasonLabelCollision { .. }) => {
                    deferred.push(((table, id), was_live));
                    refused = Some(collision);
                }
                Err(other) => return Err(other),
            }
        }
        // A pass that freed nothing will free nothing next time either, so the
        // collision is real and a person has to rename the book it names.
        if deferred.len() == before
            && let Some(collision) = refused
        {
            return Err(collision);
        }
        pending = deferred;
    }
    Ok((settled, conflicts))
}
