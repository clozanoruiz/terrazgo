# Sync

What changed and how to merge it is kept separate from how it travels. **Part 1**
covers stage 1 and what was learnt in the field that any transport has to deal
with. **Part 2** is the stage-2 merge layer: what each mechanism does, why (the
reasons that keep it from going backwards), the tests that check it, and what it
costs.

| Stage | What it is | Merge logic |
| --- | --- | --- |
| **1** | One-way mirror: the phone writes, the desktop imports a snapshot. | none |
| **2** | Two-way local sync: every device writes; changes come from the append-only `record_change` log and travel by copying a file (later, over a link). | **Part 2** |
| **3** | Cloud: the same change format and merge rules; only the transport is new. | reused |

---

# Part 1 — Stage 1, and what it left behind

## What already ships

The backup export/import ([backup-restore.md](backup-restore.md)) covers all the
mechanics of moving a file from one device to another: a consistent snapshot
(`VACUUM INTO`, reopened and integrity-checked), the Android document picker,
careful checks on the receiving side, a safety copy before importing, and
device-local state (`settings.json`, `geo-cache.db`) left out on purpose.

## Stage 1 is not being built

- **Not built**: a guard that detects and warns, a read-only mirror role, and a
  "sync" entry separate from Settings → Backup. They would protect a
  replace-everything model that stage 2 does away with.
- **Built**: the pre-import safety copies are pruned to the three most recent
  (`backup::prune_pre_import_copies`), ordered by **file name**. The name holds
  an ISO instant, which survives a copy that resets the modification time. A
  failed prune never fails an import.
- **Where a change came from travels in the sync file, not in a snapshot**: the
  device id is in `settings.json`, outside backups, because a snapshot restored
  somewhere else is a different replica (Device identity).
- **Different app versions are not a defect**: a backup newer than the app is
  refused with a clear message; sync files are stricter still (Schema skew).

## The cable, and what any transport has to deal with

### A file written by the app does not appear over MTP in the same USB session

The file is complete and indexed, and the phone lists it, but the desktop's MTP
view shows the old contents until the **cable is unplugged and plugged in
again**. It looks just like a failed save. A cable flow must say
"unplug and plug in again after exporting", or use another route (the share
sheet).

### How long the file sits incomplete

The exported file was visible for **13.6 s before it was complete**, and empty
for the first 0.4 s. `io::copy` returns once the bytes are queued, and `flush`
does nothing for a file, so every export ends with `sync_all` before reporting
success (`user_files::fill_user_file`), and the backup's copy also checks the
byte count. What `sync_all` costs depends on **FUSE passthrough**: 12 ms for
6 MB on a Pixel 8a, about 12 s on a Galaxy A22 whose old 4.14 kernel can't do
it. The `/mnt/pass_through` mount is the evidence, not the
`persist.sys.fuse.passthrough.enable` property, which both devices set. The slow
case is accepted: the bytes take that long whether the app waits or not.

### The duplicate-name trap

Exporting twice a day gave two files the same name, and the picker renamed a
`.db` file to `….db (1)`, which the import dialog's filter then no longer
offered. `exportName.js` puts the time to the second in the name, and every
export format uses it.

### The three consequences that bind stage 2

1. **A file existing doesn't mean it is ready**: the receiver checks it is
   complete.
2. **The flow must not invite copying a file while it is still being written.**
3. **A write issued is not a write landed**, and a change sent is not a change
   applied.

---

# Part 2 — Stage 2: the merge layer

## The concurrency profile it must survive

Two or more people record on their own phones during the day, a third person
writes on the laptop, and everything is brought together on that laptop, which
is itself a writing device. Entries are made in batches and late, so the time
of entry and the time of the event are far apart. So: **last-writer-wins by wall
clock isn't safe**; **every device needs an identity**; two people editing the
same record is plausible; and **the history must survive the merge**, since the
reviews, a rewind and a resolution all read it.

## Two layers, and only one of them has policy

- **Copying the log can't conflict**: `record_change` rows never change and each
  has a unique identity, so copying them is a set union, and the history
  survives every merge.
- **Building the tables from the log is where every decision is made**: given a
  register's merged log, what do the tables hold?

## The unit of merge is the whole register

**A register and its children merge as one statement.** A treatment with its
plots, problems and justifications is one thing a person stated, and causality
is tracked per register. Merging row by row, a dose corrected on one device and
a plot added on another would both apply with nothing noticed, leaving a record
neither person wrote; tracking by register puts that case in front of a person.
Applying a winning register replaces its whole set of children, which also
solves two devices filling the same child `UNIQUE` slot. The cost is that more
conflicts are shown than a row-level scheme would show, and that is right for a
record read at an inspection. It is the CouchDB shape (a fixed winner, the loser
kept for review), the aggregate boundary of domain-driven design, and git's
merge commit.

## Device identity

`device_id` (UUIDv7) lives in `settings.json` **because** that file isn't in
backups: a snapshot restored somewhere else is a different replica and creates
its own id. It reaches every write **through the connection**: `open_app_db`
requires it as an argument and puts it in a TEMP table, which never travels in a
`VACUUM INTO` snapshot, and `audit::begin` refuses a connection without one.
**No form can write it**: `update_settings` keeps the current id and only takes
preferences (`AppSettings::apply_form`), because a Settings view opened before a
restore would still hold the old id.

`sync_peer` (synced and logged, its primary key the device id) has a name people
can edit, so a conflict says "Móvil de María". Each device makes sure its own row
exists whenever it opens the database. A device can be retired (a soft change
that can be undone, and never this device), and the purge doesn't wait for a
retired device. After a backup import, `succeed_sync_peer` retires the replaced
id and moves its name to the new one.

**`sync_group`**: one row or none (`CHECK (row_id = 1)`), in the **database**,
because a restored backup is a new replica but the same farm. It is created on
the first **export**, never at launch, so two devices set up separately don't
refuse each other forever. It is an identity, not a list of members, so a device
can't carry one farm's records into another. Joining is one act
(`sync::join_sync_group`). The import says which situation it is in:
`sync_not_paired` (the normal first sync) or `sync_group_mismatch` (leaving a
group). The refused file can then be applied as it is.

#### What a contractor would need, and why it is not this

One person syncing several owners' farms independently isn't supported, and the
group isn't what stops it: sync works on the whole log, `record_change` has no
farm column, and `operator`, `advisor`, `user_profile` and `export_alias` don't
belong to a farm. Scoping by farm would be multi-tenant separation, which isn't
planned.

**Worth considering if a contractor, advisor or cooperative asks: one database
per farm** (not decided). Contractors and service companies, advisors checking
several farms, a cooperative's technician, a gestoría printing books for
clients, a family whose two holders sync with different people: each has
several independent farms, each with its own sync group. As separate database
files on one installation:

- **It matches the law.** The book is the holder's duty, and a contractor's
  treatments on farm A belong in farm A's book; the contractor keeps a synced
  copy of the farmer's database, not the other way round. The copies of the
  contractor's operators, machinery and products in each database are what each
  book has to print anyway.
- **Separation comes for free**: separate files, groups and backups, and
  nothing of one client can be reached from another. A file names its group, so
  an import can find its database, and a file from a group nobody holds can
  start a new one: a new client. When a contract ends, the database is deleted
  and the farmer retires the device.
- **Optional, never forced**: a large farm under one holder keeps its farms in
  one database, as today.

What it would take: a list of databases and switching between them (backup
import already does the swap); a device id per database instead of per
installation, because two databases sharing one id would clash
(`device_identity_shared`) if a copy of one ever joined the other's group; the
active profile and the integrity result per database; catalogues in one file per
installation (they are already device-local) or refreshed in every database; the
alerts bell across databases; Android storage. That is a large piece of work.
What it costs a contractor: a change to their own registries (a renewed licence)
has to be made once per client (an "apply to all my databases" action, one
change set in each, would help), and there is no view across clients (read-only
views over attached files would help).

**Rejected: scoping by farm inside one database.** A file only applies if the
receiver then holds everything its sender held, and a device numbers its change
sets once across its whole database. A contractor sending farmer A only farm A's
changes would leave A forever missing farm B's sets, so every file would be
refused. Making it work needs numbering per farm, a farm on every log row, a
farm on the shared registries, and per-farm purges and conflict queues:
separate databases rebuilt row by row inside one file, with a leak between
clients one query away.

**The device is not the actor**: `record_change.actor` is still the
`user_profile` who made the write.

### Sequence rewind: three paths closed, one detected

| Path | Outcome |
| --- | --- |
| A backup imported on the same device | It creates a new id before the swap, so it **can't happen** |
| A backup imported on another device | That device's own id: can't happen |
| `settings.json` lost | A new id and a new device to name: can't happen |
| The whole app-data folder copied to a second machine | **Can't be prevented** |

The last one fails loudly: a device holding `(D, 51)` with different content
refuses the file as `device_identity_shared`, comparing a **content hash of the
change set** worked out from the rows when needed and never stored. And two sets
with the same `(device, seq)` cancel each other out, leaving a register with rows
and no head: `heads` refuses with `register_has_no_head` instead of applying
nothing and reporting success.

## The change stamp — one value per write transaction

`audit::begin(conn, actor)` opens a transaction **and** its change set together
as one `WriteTx` (it derefs to the transaction): one device sequence number and
one clock value. `tx.register(root_table, root_id, season_id)` stamps each
register the transaction touches with its own version vector: its current heads
merged with this change set. The log helpers take that `&ChangeStamp`.

**A change set is the unit of writing; the register is still the unit of
merge.** Seven writes touch two registers in one transaction because the domain
needs it: a seed treatment and the declaration it withdraws, a water point and
its plot's declaration, a soil cover and its maintenance lines' own registers,
an advisor and its links, a zone check per zone type. Splitting them would lose
atomicity. The merge applies or conflicts each register separately; an
inconsistency across registers is an offline invariant, and is reported.

### A crate written afterwards pays none of this, and cannot opt out

1. **At compile time**: a second change set in one transaction would need a
   second `WriteTx` while the first holds the connection, and a stamp borrows
   its `WriteTx`, so it ends at `commit()`. Two `compile_fail` doctests check
   both.
2. **At write time**: `write_change` works out each row's register from the row
   itself (its id, its parent column, or its slot's values) and refuses a stamp
   that disagrees, an image naming another row, or a `season_id` that isn't the
   stamp's.
3. **The aggregate map is a `Module` trait method with no default**, so a module
   with tables doesn't compile until it classifies them.
4. **`sync_shape_contract.rs`** refuses an unclassified user-data table, and a
   `UNIQUE` key two devices could fill without the merge seeing it.
5. **`applier_contract.rs`** rebuilds **every** synced table from its log alone
   and compares it row by row; a table nothing in it writes fails by name. It
   seeds data in the shell, the one place that sees every module, so no crate
   has to remember to call it.

There is no merge test harness per crate, on purpose: what is particular to a
register (where its rows are filed, its images, its slots, its duplicate rule and
book tables) is checked by the five above and the tests next to them, and the
merge itself is one engine, tested by core's scenarios. The testkit has
`last_stamp` and a whole-log `sync::send` for scenarios that aren't about the
transport.

## What `record_change` gains

These columns are repeated on every log row, so a log row can be read on its
own. A register with ten child rows repeats about sixty bytes eleven times.

| Column | Purpose |
| --- | --- |
| `root_table`, `root_id` | the register the change belongs to: a row's own id, its parent's for a child, a slot's values as a JSON array |
| `origin_device`, `origin_seq` | the replica and its change-set counter; together, the change-set key |
| `version_vector` | JSON `{device: seq}`, the register's vector after this write |
| `hlc` | a hybrid logical clock stamp, used **only** to order changes already known to be concurrent |

**The clock** is one INTEGER: bit 63 clear, 47 bits of Unix milliseconds (good
until about the year 6400) and a 16-bit counter. Integer order is clock order,
and the value is never negative. The next stamp follows the paper's send rule;
the receive rule comes for free, since the log holds other devices' rows. **A
full counter carries into the milliseconds instead of refusing the write**; the
one way to fill it is a phone whose date was set back from a year ahead. In JSON
it is a **decimal string** (it is larger than 2^53); `sync::Hlc` serialises that
way itself, and the frontend compares with `BigInt`.

**Indexes**: the `UNIQUE (origin_device, origin_seq, entity_table, entity_id)`,
whose first two columns serve "changes since" (`origin_seq > ?`, a range per
device) and the next sequence number; `(root_table, root_id, origin_seq)` for a
register's history; and `hlc`. A sequence index, not a time index. Every
question a write asks the log is one seek, checked by a full-scan test with a
control.

## Merge

### Version vectors decide whether there is a conflict

| Incoming `E` against local head `L` | Meaning | Action |
| --- | --- | --- |
| `E ≥ L` in every component | newer, causally | apply |
| `L ≥ E` | already held | store the log row, change nothing |
| neither | **concurrent** | conflict |

### Hybrid logical clocks decide which side goes live — and nothing else

An HLC ([Kulkarni et al., 2014](https://cse.buffalo.edu/tech-reports/2014-04.pdf))
gives a total order that respects causality in one sortable value, but like
every clock of the Lamport family it **can't detect concurrency**; vectors do
that. So the vector decides whether there is a conflict, and the HLC decides
which side is live while a person looks at it: it always increases on a device
even when the wall clock jumps back, never ties, and stays close to real time.
The last tie-break is the device id. **It never replaces `changed_at`**, the time
the screens show, since an HLC can run ahead of real time.

#### How far ahead a stamp may be

A device adopts a stamp that is ahead of its own clock, so one phone set to 2030
would drag every device there for good. An incoming stamp more than **ε = 2
hours** ahead makes the file be refused (`CoreError::PeerClockAhead`, naming the
device and how far ahead; `Hlc::within_skew`, checked before the transaction
opens). Skew that was accepted heals within ε, so the exact value doesn't matter
much. Two hours covers a clock set to Spanish local time as if it were UTC (+1
in winter, +2 in summer). Beyond a few hours the device's application dates are
wrong too, so refusing reports a problem in the book instead of hiding it.
Daylight saving doesn't matter: the physical part is Unix milliseconds.

### Applying a winner needs rewind and replay

A change set only logs the rows it changed, so a winner is applied by undoing
the local concurrent sets (their `before` images, newest first) and replaying the
incoming branch (`after` images, in order). Within a set the order is
`record_change.id`, a UUIDv7, which is in creation order within one process and
is kept when received. **The log is never rewritten; only the tables change.**

#### A rewind is not written down, so a replay has to infer it

The log is only ever added to, so a rewind leaves no trace. Once a resolution
merges both heads, the losing set is ordinary ancestry, and replaying the whole
lineage would put rewound rows back on every device except the one that rewound
them. That only shows when the two branches touched **different rows** of one
register. So **a version's state is a chain**: its sets, newest `(hlc, device)`
first, each one kept only if it is causally comparable with every set already
kept, which is the order `live_head` picks by at every point in the history.
**A set is beaten only by one that is itself kept**; the looser rule passes every
ordinary scenario and loses history in that one. This is what makes **a
register's tables a pure function of its log**: a device that receives
everything at the end holds the same as one that synced at every step.
`merge_scenarios.rs` checks it against a device that was away the whole time.

**A branch is what its own chain holds and the other's doesn't**, never what both
versions have seen. A set both have seen can be in one chain only (beaten on one
side and kept on the other), and if branches were split by what both have seen,
neither would rewind and replay it. For example, a laptop that keeps its own
version of a conflict and sends its file would leave the phone holding, for
good, a plot the kept version never had. Below the newest set both chains keep,
the two chains are the same, so the first time the live branch's own part
touches a row, it was written on top of that shared part, and its before-image
is what the rewind needs.

#### A branch is applied a row at a time, not a step at a time

Only the last thing a branch did to each row is applied (the first, for a
rewind). Going through the states in between trips partial `UNIQUE` indexes for
states no device was ever in (a book created then deleted would hold its name
for one statement), and costs a register's whole history instead of its rows.

**The rows a branch takes out of the book go first.** Touching each row once
moves a row's removal to where it was last touched, which can come after another
row's arrival. For example: a plot's declaration withdrawn on one phone and
deleted by a resolution that brings another back first; on the device where the
first one still existed, both existed for one statement, and the index refused
every file from then on. The writer can't put these in order, because which row
leaves depends on what each receiver holds, so resolutions and restores don't
try, and the apply orders it on the tables it writes. Within one register no
existing row changes a `UNIQUE` key (a child row keeps its plot, a slot row its
slot), so removing first is all the ordering a register needs.

### The generic row applier

One function builds `INSERT … ON CONFLICT(id) DO UPDATE` from a payload's keys,
checked against `PRAGMA table_info`. **A payload's keys are its table's
columns**, by design (`#[serde(rename)]` where a field and a column differ).
`applier_contract.rs` seeds a real campaign and one of every other register
through the repositories, replays its log into an empty database through the
applier alone, and compares every synced table row by row.

#### Where the applier's statements live

The applier's SQL is different for each table and set of keys, so it must **not**
use the prepared-statement cache. That cache is one LRU keyed by SQL text on the
single connection every command shares, and it holds the write path's
statements (`audit::begin` goes from 24 µs to 4–5 µs with it). One import would
push them out, and no size fixes an unbounded set of keys. The applier groups
rows by (table, set of keys) and holds one plain `prepare` per group for the
import. `sql::cached_statement` is the only way into the cache and takes
`&'static str`; `statement_cache.rs` counts its statements against
`db::STATEMENT_CACHE_CAPACITY` (32).

### Foreign keys are deferred, not ordered

A register made on one device may point at a farm made on another, so no apply
order is safe. The apply sets `PRAGMA defer_foreign_keys = ON`, and an
incomplete file is refused whole at commit. `foreign_keys` stays on.

### The aggregate map

Applying a winner needs a register's children, which the incoming set doesn't
carry, so each table is classified (`sync::SyncRole`): core's in
`CORE_SYNC_SHAPE` and each module's through `Module::sync_shape()`.

| Role | Meaning |
| --- | --- |
| `Root` | a register of its own, identified by its row id |
| `Child { root, fk }` | part of another register, merged with it; `fk` names the register's row and cascades from it |
| `Slot { columns }` | every row with these values, over time, is one register; `root_id` is the values as a JSON array |
| `Local` | never logged, never merged (`sync_conflict`, `catalogue`, `catalogue_code`, `sync_known`, `sync_held`) |

`PRAGMA foreign_key_list` alone can't classify: `plot_zone_flag` and
`geo_feature` cascade from `plot` but are registers of their own.

**Registers keyed by slot.** Six tables keep one live row per `UNIQUE` key and
replace it: `farm_advisor`, `geo_feature`, `plot_zone_flag`,
`plot_water_declaration`, `register_declaration`, `export_alias`. Keyed by row
id, two devices filling one key would be two unrelated registers; the second
would fail on the index and its file would be refused forever. Keyed by the slot,
they are two versions of one register, the normal conflict path. The key is read
from the row with the same constant its declaration uses (`sync::slot_id_of`).
The contract test holds every `UNIQUE` key to the map; the exceptions are
`export_alias`'s `(target, alias)` (one submitting device per farm) and
`season`'s `(farm, label)` (Seasons created on two devices).

#### The map answers on the way in as well as on the way out

`write_change` checks every row a device writes, but a row that **arrives** is
bytes from a file whose table name the applier puts into SQL. So the merge asks
`sync::register_of` about every arriving row and refuses one filed under another
register, or in a table that never syncs. It is a primary-key seek per row, on
import only.

### Seasons created on two devices

Two phones opening "2026/27" at the start of a campaign is the likely case.
**A season's id is a UUIDv5 over `farm_id|starts_on|ends_on`**, so two devices
opening one campaign with the same dates write one register, which vectors and
the clock already handle. The id **keeps the dates it was created with**: it is
made from them and never worked out again to look a season up. The same farm and
the same dates is one book (`season_dates_taken`), and creating a deleted book
again brings it back.

**What the id can't solve, a person decides.** The name comes from the *years*,
the id from the exact dates, which nothing prefills, so two devices often
collide on the name and not on the id. **`UNIQUE (farm_id, label)` stays, and
the import refuses with `season_label_collision`**, naming the farm and the
book, because every automatic way out destroys something:

| Way out | What it costs |
| --- | --- |
| An id from the span of years | a farm's spring and autumn cycles merge |
| An id from a name that can't change | two different campaigns both called "2026" merge |
| Renaming the losing book when applying | the app invents an identifier on a printed, submitted document |

**Merging on something that isn't an identity destroys records; two books with
the same name only look untidy.** The farmer renames this device's book and
imports again (nothing is sent again), then merges the two if they were one
campaign (Merging two books).

#### The collision check sees one register, not the bundle

It asks what the partial index would ask: the tables, ignoring a book that
arrives deleted. Within one file, a book giving up a name and another taking it
are settled in UUID order, so the importer puts a colliding register aside, tries
it again once every other register is settled, and only refuses if it still
collides.

## Counters, and why the merge layer must never be shown one

> **Never store a running total two devices can both change. Store the events
> and work out the total.**

Juan takes 50 l offline, María 100 l. As events they are two registers, nothing
conflicts, and the tank reads 350 l by `SUM()`. As `tank.remaining` they are one
row written twice, 450 against 400, and every possible answer is wrong. So
product stock, water allowances and fuel are event tables plus a view; nothing
in the schema is such a counter. Two things follow: a derived total inherits the
duplicate problem, and **an offline invariant can't be enforced, only
reported**. An overdraw is something the book advisory points out, never a
silent clamp.

## Conflicts as the person sees them

A `sync_conflict` row per concurrent pair holds both branches' change-set keys.
Three properties matter:

- **The book never breaks or blocks**: a fixed winner is live straight away,
  the same on every device.
- **Nothing is lost**: the loser stays in the log.
- **A person may merge; an algorithm may not.** Resolving is an ordinary logged
  write whose stamp merges both heads, so it closes the conflict everywhere.
  Editing the record from its own screen does the same.

The queue is **for the whole database, on the Status view**: a conflict is work
waiting, and a register isn't always in a book. Naming devices is in Settings.
**The comparison is generic**: `merge::review` rebuilds both versions from the
log (a losing branch is never written to the tables) and returns
table/row/column lines; `src/lib/syncFields.js` gives the words; a reference
shows as the row it names through a combined `RowCaption` map, which also gives
a register its title in the queue. A register nobody has described still gets
reviewed, with its column names. `sync_fields_contract.rs` fails when a register
the words map covers gets a column it doesn't name. `season_id` is compared and
shown as the book's name.

**Keeping a version is one write** (`merge::resolve`): every row the two
branches disagree about, as the kept branch left it, then `settle`. Comparing
with the live branch is enough in a three-way conflict, because `contributing`
keeps exactly the version `live_head` picks. **A removal on either side is no
different**: the choice writes what differs, like any other (Bringing a register
back writes only what changes). Two versions can hold the same child slot with
rows of their own (two devices adding the same plot), and in key order the
arriving row met the leaving one in the slot, so **the apply empties a slot
before filling it** (A branch is applied a row at a time).

This path is tested on the hard cases: a set of scenarios across four simulated
devices (concurrent edits, an edit against a soft delete, two devices filling
one child slot, a three-way edit, a resolution while a fourth device is away,
the same file applied twice; `merge_scenarios.rs`), and random histories
(`random_histories.rs`, How it is tested), which found what the scenarios
hadn't.

### Versions that say the same thing are not listed

`settle` compares the live version with each other one using the review's own
comparison, so there is one definition of "differs" for both. A removal is
compared as removed or not, never by its time. A version that agrees isn't
listed: two people keeping the same copy of a duplicate, or two devices merging
one pair of books the same way. Both stay in the log, and the next write merges
them. Two rows held under different ids never agree.

**Two removals agree, whatever the record held**: removed is removed, which is
all any screen shows. Otherwise a record that a phone corrected and then deleted
with its book, while the laptop also deleted the book, would offer a choice that
changes nothing visible; it would block bringing the book back while it waited,
and once chosen it would turn the record into a removal of its own, which
bringing the book back would then leave behind. When the book is brought back,
the record returns as the later removal left it; the other stays in the log.

### A backup import says what it would cost, and hands the name on

A backup import is a mirror, not a merge, and what it loses is limited to change
sets that never left this device. `backup::discarded_by_import` counts them
before the swap, and the confirmation says how many exist nowhere else. A
damaged or foreign file is refused before the question is asked. The import
creates a new device id, and `succeed_sync_peer` retires the old row and carries
its name over.

## Transport: a bundle file, framed so a link can reuse it

Plain serde messages (`SyncManifest`, `ChangeSetFrame`, `RowChange`) in
**gzipped NDJSON**: a manifest line, a line per change set, and a final line
with the count and a CRC32. The extension is `.tzsync`, and the name has the
time to the second and the device. NDJSON because the payloads are JSON text
copied byte for byte, so the change-set hash comes out the same anywhere. Hashes
are XXH3-64 (`twox-hash`, past 1.0, so its output won't change). They are
compared in pairs and never stored: they check identity, they are not
tamper-evidence.

**The manifest's `seen` is both what the sender holds and what it asks for**, so
**two file copies are a full two-way sync**. It also has `since` (what the file
was trimmed to) and `known` (what the sender knows each device holds, for the
purge). A live transport would change the framing and keep the messages. A file
is checked (count, CRC, schema, completeness) before anything is applied, and it
is applied in one transaction.

### Schema skew: equal versions only

Both devices need the same `user_version` **and the same schema digest** (XXH3
over every table's and index's DDL, in name order). A whole database can be
migrated forward; a stream of row images can't. And pre-release migrations are
edited in place without the version changing.

### How big is a sync

`cargo test -p module-phytosanitary --release --test bundle_size -- --ignored --nocapture --test-threads=1`:

| Exchange | Change sets | Gzipped |
| --- | --- | --- |
| A day, one operator (8 records) | 8 | 2.1 KB |
| A day, three operators (30 records) | 30 | 5.3 KB |
| Whole log, a season on a medium farm (400 records) | 447 | 78 KB |
| Whole log, a cooperative's year (4 000 records) | 4 127 | 871 KB |

About **180–270 bytes per change set** at any scale. A delta costs what the delta
is, and a whole-log file is under a megabyte even at the top end, so no state
about what another device holds is kept between sessions. Catalogues and
`geo-cache.db` never travel.

### A trimmed reply is safe only for the device it answers

A reply is trimmed to the manifest it answers, and if another device imports it,
it can deliver a change set without the sets it was built on. Four failures were
reproduced on three devices: a gap in one device's numbers; a change without its
cause, **leaving two devices with the same log and different records forever**;
a cause in another register held by a foreign key (a raw `FOREIGN KEY constraint
failed`); and one held by a name (a false `season_label_collision`). Version
vectors can't see the last two: a register's vector only counts the sets that
touched it.

**The rule: a file applies only if, afterwards, this device holds everything its
sender held.** It needs no knowledge of what depends on what, within a register
or across them: every log has no gaps and holds every set's causes, and the rule
keeps the receiver's log the union of two such logs. It is git's bundle
prerequisite and a delta-CRDT's interval condition, with the same fallback, a
whole copy. **It is checked against what the file starts from**: a file carries
every set its sender holds past `since`, and only applies on a device holding at
least `since` of every device, counted by `sync_held`, which no erasure lowers
(`bundle::refuse_if_incomplete`, which `inspect_sync_bundle` also runs, before
asking anything). The refusal, `bundle_skips_changes`, says to import a file from
this device on the sender and export again.

On the sending side, **the reply answers every file imported in this session**,
trimmed to the common ancestor of their manifests, so the laptop's one file is
safe for every phone it heard from. After a restart it falls back to the whole
log. The manifest and the frames are read in one snapshot.

Considered and not used: checking each arriving set's vector (blind across
registers); per-set dependencies with a waiting area, Automerge's way (a format
change, and it still can't apply the laptop's later writes); naming the receiver
(refuses harmless files, and protects nothing that doesn't write it); never
trimming (reads the whole history again at every import for as long as the farm
uses the app). Tests: `crates/terrazgo-core/tests/bundle.rs`.

## Retention and real deletion

Art. 16.3's three years are for the entries and their documents; no text asks
for a history of changes, or for deleted entries to be kept
(docs/cuaderno-print.md → What the law asks of the entries over time). What the
log keeps, and for how long, depends on what reads it.

### What the log holds

On module-phytosanitary's scaled farms the log is **60–77% of the database**
(2.2 of 3.6 MB for a season of 400 treatments, 25.6 of 33.1 MB for a
cooperative's year). Removing isn't deleting: a soft delete logs the row before
and after, so deleting a book's records makes the file grow by about half of what
the book weighed. Only a purge gives space back.

### Live registers keep their whole history

Nothing prunes a live register: a device joining replays every register from its
first change set, a conflict's review and resolution rebuild each version from
the register's log, and a record's history view, when it is built, will read it.
Compacting to each row's last image saves little and loses the history; keeping
stubs would make a copy of the whole database the only way to join. Neither is
done.

### Deleting a book with its records

**One act, one change set, generic** (`repository::delete_book`): every live row
of every table the installed aggregate map attaches to a season
(`sync::book_tables`) is removed as its register's own version, read from the
log, and the book with them; a slot keyed by the book is withdrawn. A register a
future module adds is deleted without anything written for it:
`book_merge_contract.rs` holds every table with `season_id` to the map.
**Refused while something in the book is waiting for a person**: a conflict
(`book_delete_conflicts_waiting`) or two opposite removals
(`book_delete_removals_waiting`), since deleting would settle it without anyone
seeing. On other devices, a record written into the book before the deletion
arrived becomes a record in a removed book, and is listed; a correction made in
the meantime is a conflict against the removal.

**Brought back by record, not by book** (`repository::restore_book`): a record
comes back when **every current version of it is a change set that also deleted
this book**. Undoing the book's own change set would bring a book renamed
elsewhere back live and empty, with its records listed nowhere; testing each
record reaches them whether the book is removed or live again, and a live book
holding such records says so on its page, with *Recuperarlos*. A record removed
on its own stays removed. Each record comes back by undoing what the deletion
wrote in it: its own row, with the removal cleared (Bringing a register back
writes only what changes). Refused while a conflict is waiting in the book
(`book_restore_conflicts_waiting`), or while another live book has its name
(`season_restore_name_taken`).

**Offered back for thirty days** (`REMOVED_BOOK_DAYS`, counted from the day of
the deletion by the clock of the device that made it): a closed section under
the record-book list, *Cuadernos eliminados*, shows each book's farm and dates,
when, by whom and on which device it was deleted, what went with it per kind,
until when it can come back, and *Recuperar*. **After that day the book isn't
offered back, and goes for good** (The purge). A book not yet erased stays in the
section without the button, saying until when it could have come back and what
its erasure is waiting for (`repository::book_erasure`, which asks the purge's
own conditions about that book):

| It is waiting for | The line says |
| --- | --- |
| Devices not known here to hold its deletion, or the deletion of something in it | their names; syncing them, or retiring one in Settings → *Dispositivos de esta explotación*, ends the wait |
| A person on the Status view: a record still live in it, a conflict, two opposite duplicate removals | it goes once that is settled *en la página «Estado»* |
| Only the calendar | the day it goes, or, once due, the next time Terrazgo opens |

This is only asked about what can go of the book (The purge → What can go); once
that has gone, the book leaves the section (`book_erasure` returns `None`). A
record still live in it keeps it listed, as waiting on the Status view.

**The confirmation names the book, counts what goes, and says where it can come
back from and until when**: *"¿Eliminar el cuaderno 2025/2026 de Los Llanos y
sus 412 registros? Podrá recuperarlos hasta el 31/10/2026, al pie de la lista de
cuadernos. Después se borrarán definitivamente."* The date comes from the
backend, so the screen doesn't keep its own copy of the thirty days.

### What a purge has to survive

A simple purge (rows and log rows deleted on one device) was tried on simulated
devices exchanging real files, and failed in six ways. Code and tests refer to
these rules by number:

| | The simple purge | The rule |
| --- | --- | --- |
| 1 | A device joining a purged laptop is refused: its numbers have gaps | a file says what it starts from; a gap above that is an erasure |
| 1b | A device's own last change set is purged, its number reused, then refused everywhere as `device_identity_shared` | what a database has held, never lowered, sets its next number |
| 2 | A device that hasn't purged sends the history back | every device purges, and an erased change that arrives is dropped |
| 3 | Purged before a device that corrected the record offline holds the removal: live with none of its plots, or the removal lost | **only once every device holds the removal**; a correction made before then is already here, as a conflict |
| 4 | A restore arriving after the purge rebuilds half a record | **what was written where the purge hadn't arrived is discarded**: gone, full stop |
| 5 | The same campaign opened again makes the book's derived id again, with no history | a marker per erased register, read by every stamp |

Two more come from the code: a removed row can be pointed at by a record that
stays, or by one that arrives later, so only what nothing else can point at is
erased (What can go); and one change set can write a purged register and one
that stays, so the identity check only compares registers no marker names.

### The purge, as settled

#### Nobody asks: what was deleted goes, thirty days on

**Everything deleted from a book that nothing else can point at (a record removed
on its own, or what went with a book) goes for good once thirty days have passed
and every device holds the deletion**, on whichever device gets there first.
There is no button: the thirty-day wait gives what a button would, and the
removed-books section says what a book past its date is waiting for. What is
given up: a record's history can only find removed records thirty days back, and
a removed record can no longer be dug out of the live database by hand.

#### What can go: what nothing else can point at

The registers `sync::book_tables` finds **whose table no other register's table
points at** (its own children aside), read from the schema's foreign keys, so a
module's tables sort themselves (`BookTable::erasable`; `sync::install_shape`
marks what is pointed at). `book_merge_contract.rs` holds the list below to the
composed schema, so a module adding a reference changes it, and this text.
Treatments, fertilisations and their plans, harvests, analyses, cultural
operations, grazing, non-field and seed treatments, and a book's declarations go.
**The books themselves, crops, sowings, irrigation records and soil covers never
do**, because other records point at each of them, and neither does a plot,
operator, machine, product, advisor, premises or farm. What stays is removed and
shown nowhere: no list, print, export or section offers it.

**Why not erase what records point at.** The purge is checked on one device, but
a version naming what it erased can arrive later: from a device not heard from
yet (a phone that records into a book the laptop deleted and purged a month
before), or from one that, after saying it holds a removal, edits a record it
still holds in that removed book. If the book or crop that version names were
erased, each device would refuse the other's files for good, with a raw `FOREIGN
KEY constraint failed`; two devices are enough. Kept, the late record arrives as
a record in a removed book (Records in a removed book), and a late edit as a
conflict. The alternative was to discard such versions and say so, but that
needs versions built on a discarded one to be discarded too, and an identity
check that accepts a change set partly discarded: repair machinery, where this
prevents the case.

**What it costs** (a deleted book, then the purge, release): 2–3% of what a purge
frees. 140 KB of 4 072 KB for a book of 30 plots, 30 crops, 30 sowings and 400
treatments; 620 KB of 39 820 KB at 120 plots and 4 000 treatments.

**To the farmer the book is deleted for good**, and every screen says so: the
confirmation's *"Después se borrarán definitivamente"* stands, the section stops
listing a book once what can go of it has gone, and an import counts such a book
as erased. What stays is never mentioned. The one place a deleted book's name
shows again is a record that arrives for it late, on the Status view, as a
record in a removed book, which is what the rule is for.

#### When a register goes

All of these must hold (`repository::purge_due`):

- **every version it has is a removal**, so no conflict is waiting on it, and it
  isn't one of two opposite duplicate removals. Two devices removing one
  register separately leave two versions that agree; the marker names them
  together, so a device holding either one drops it as history;
- **each removal was made more than thirty days ago**, by its own `changed_at`.
  A wrong clock only moves the wait; the next condition reads no clock;
- **every active device holds each removal, as far as this device knows**. Each,
  not one: a device that brought the book back on top of one removal would see
  its restore discarded. A retired device isn't waited for;
- **its table is one nothing else points at** (What can go), so nothing that
  stays, and nothing that arrives later, can name what goes;
- **no `export_alias` names it**, until the exporter's submission log can say
  the authority no longer holds it.

It is found through a partial index over each register's removed rows, so a live
row costs nothing. `index_contract.rs` holds every register of the book to that
index, and every column pointing at one to an index that includes removed rows.
**Which tables can go is worked out once per connection**, when the map is
installed: reading it from the schema on every purge added 1.7 ms to every start
and import, for an answer only the schema changes. Read once, a purge with
nothing due costs 0.40 ms with the composed schema.

#### When it runs

A few seconds after start (`db::PURGE_AFTER_START`), off the readiness path, and
after every import: the two moments something becomes due. A purge that can't
run says so and is tried again; an import never fails because of it. **It
erases rather than compacts**: `secure_delete` overwrites what it frees, and a
checkpoint empties the write-ahead log. Copies made earlier are out of its
reach.

#### What each device knows of the others

A device learns what another holds from that device's `seen`, recorded only once
a file applies, which is when the completeness rule makes it true. Every manifest
also has what its sender knows of every device (`known`), kept device-local in
`sync_known`, so phones learn about each other through the laptop and any device
can purge.

#### It travels as a row per register — and the row is a marker

One insert-only `purged_register` row per erased register (the register, the
removal's version vector, when), in one change set: two devices erasing one
register write two rows, never a conflict. **The row stays as a marker**, and
**every write stamps a register on top of every marker naming it**
(`WriteTx::register`), so a register whose id is its key rather than a new one (a
book's declaration made again) is newer than its deletion everywhere, not a
rival to it. Books are never erased, and opening the same campaign again brings
the deleted book back under the name and dates just given.

#### What a database has held, and what a file starts from

`sync_held` (device-local) is the highest number this database has held from each
device, raised when a file applies and when a purge runs, and never lowered. It
is what a manifest says the device holds, and where its own next number starts.
A file states `since`, what it was trimmed to; a number past that which the file
doesn't carry is one its sender erased.

#### An import meets the purge

Each arriving row's register is looked up among the markers, held or arriving:

- **seen by the removal** → dropped: it is erased history;
- **written by a device that already held a purge of the register** → applied:
  something new in it, written knowing the old was gone, such as a declaration
  made again for its book (every stamp carries the purges it has seen,
  `audit.rs`);
- **anything else**, written where no purge had arrived (a late correction, a
  book brought back on a phone whose calendar was behind, a choice made in the
  review, on a device that was away, retired or never heard from) →
  **discarded, and the import names the device**: *"Un cambio hecho en Tablet
  vieja a un registro borrado definitivamente no se ha aplicado."* Refusing
  instead would let such a change reach the laptop first and stop its files
  everywhere.

**Deleted more than thirty days ago means gone, full stop.** A device holds a
purged register's whole history, or none of it, never part. (Bringing a late
restore back whole, built on changes other devices had discarded, left two
devices building one record from two logs.) A book brought back late comes back
itself, with its crops, which are never erased (What can go), and without what
went.

A marker arriving does the same to what is held, so every device ends up with
the same log as well as the same tables. The import also says what it erased,
books and records counted separately, both sources together, with no number of
days: *"Se ha borrado definitivamente lo que ya no se podía recuperar: un
cuaderno y 412 registros."* A book counts once the last of what can go of it has
gone, since that is when it went for the farmer.

**A version naming a row that was never erased needs none of this.** A record
written later into a deleted book, or a late edit of a record still filed in
one, names the book, which is kept (What can go), so it applies like any
version: as a record in a removed book, or as a conflict.

#### Bringing a register back writes only what changes

**A write that brings a removed register back only writes what changes**
(`repository::undo`). Bringing back a duplicate removed twice over, or a book
with its records, puts back what the removal wrote (the record's own row, with
the removal cleared); keeping a version in the review, with a removal on either
side, writes what the two versions disagree about. Every device such a write
reaches holds the register's history and can rebuild the rest from it: a file
only applies after everything it was built on (`bundle::refuse_if_incomplete`),
and where the purge has arrived, a write that never saw it is discarded (An
import meets the purge). What the register points at (its book, a crop removed
on its own) is never erased (What can go), so every device holds that too.

**A merge carries what its moved records name** in the same way: removed records
stay in the book that goes, removed with it, except the removed rows a moved
record names, which go with it through the rows that name a book; their plots
and other rows follow them (Merging two books → The merge).

### Knowledge that runs behind cannot drift

What a phone knows about another arrives through the laptop, one exchange late,
which makes it later, never wrong:

| Situation | What happens |
| --- | --- |
| A phone's knowledge of another is old | knowledge only understates; a device only gains sets under one id, and a restored copy gets a new id |
| A phone learns through the laptop that another holds a removal | the same file made it hold everything that phone had written, so an offline correction is already here as a conflict |
| Two devices purge at once | two markers, each true; a later write is stamped on top of both |
| One device restores while another purges | the restore never saw the purge: it is discarded wherever the purge is, and named; the book comes back, without what went |
| A device nobody knew about holds the record live | the marker erases it there, being older than the removal; its own change made in the meantime is discarded and named |
| A device nobody knew about records into a book deleted and purged elsewhere | the book wasn't erased (What can go): the record arrives everywhere as a record in a removed book |
| A device, after saying it holds a removal, edits a record it still holds in that removed book | the same: a conflict, or a record in a removed book; never a version naming something erased |
| A device retired elsewhere, not yet heard about here | this device still waits for it |

### What the confirmations say

A record removed on its own: *"Dejará de aparecer en el cuaderno"*, with no
promise about the database and no date, since no screen brings one back. A book:
the date until which it can come back, and that it then goes for good. The map's
*Quitar el contorno* still says the outline isn't deleted from the database,
which is true: an outline is a slot of a plot, and a plot is never erased.

### What it costs a farmer in time

**Everyday work doesn't change.** A save does two more seeks (a marker,
`sync_held`); the `write_cost.rs` A/B comparison (release, interleaved) is within
noise on a fresh book and on one with 20 000 records, in memory and on a WAL
file. Finding nothing due is one statement per erasable register of a book, on
removed rows (`query_scope.rs`).

**Each new act has a time budget, measured when it is built**
(`module-phytosanitary/tests/book_merge_cost.rs`, release, median of three):

| Act | Budget | Measured, 500 / 4 000 treatments |
| --- | --- | --- |
| Merging two books | — (the reference) | 220 ms / 1.73 s, with or without anything carried |
| Deleting a book with its records | at most a merge | 224 ms / 2 s |
| Bringing a book back | twice a merge | not measured in its current form (it writes only what changes) |
| The purge | at most deleting the same book | 91 ms / 0.77 s; applying its file elsewhere 121 ms / 1 s |
| The section's line for a book past its date | at most what the section reads within the thirty days (5 ms / 44 ms) | waiting for a device 6 ms / 46 ms; due and not yet erased 6 ms / 45 ms |

### How it is tested — a laptop and three phones

A laptop L and phones P, Q, R (R away the whole time) exchange real files
through `write_bundle`/`apply_bundle`. Each situation is run with whole-log files
**and** trimmed replies, and with L writing something unrelated each round: a
whole-log file sends erased history again and a purged last change set lowers a
number, so each can hide a divergence the other shows. **Every situation ends
with the same tables on every device, the same review queue, no conflict nobody
caused, and every refusal named by its code.** Each mechanism was taken out in
turn to see its scenario fail.

**Deleting a book**: `terrazgo-core/tests/book_delete_scenarios.rs` (four
devices) and `book_delete.rs`:

| | Situation | Ends in |
| --- | --- | --- |
| 1 | L deletes a book; P had recorded into it offline | P's record live in a removed book, listed everywhere |
| 2 | L deletes a book; P had corrected one of its records | a conflict, correction against removal |
| 3 | L and P each delete the same book | the removals agree, nothing listed; brought back, everything returns |
| 3b | L deletes a book while P removes one record on its own | brought back, that record stays removed |
| 3c | P corrects a record and deletes the book; L deletes it too | the removals agree, nothing listed; brought back, the record as the later deletion left it |
| 4 | L deletes and brings back a book while Q records into it | everything back, nothing in conflict |
| 5 | P brings back a book that Q, never told, records into and corrects | the new record in the book; the correction against the restatement is a conflict |
| 6 | A record removed on its own before the book went | stays removed when the book comes back |
| 7 | L deletes a book while P merges it | a conflict per record; nothing lost either way |
| 8 | L deletes a book while a conflict or opposite removals wait | refused by name |
| 9 | L's one reply to P and Q | as with whole-log files |
| 10 | L deletes a book while P renames it | the book live and empty where the rename is live, its records offered back on its page |

**The purge**: `terrazgo-core/tests/purge_scenarios.rs` (four devices and one
joining) and `purge.rs`. Every situation also ends with the same **log** on every
device:

| | Situation |
| --- | --- |
| 1–5, 1b | the simple purge's failures above, each converged |
| 7 | P purges on knowledge of Q that reached it only through L |
| 8 | L and P purge at once, knowing different things |
| 9 | Q brings a record back while P purges it: the record goes, the book comes back |
| 10 | Q, restored from an old copy, holds the record live when the marker arrives |
| 11 | a new phone joins after a purge, by file and by a backup copy |
| 12 | a retired phone comes back with a change to an erased record (discarded, named) and unrelated changes (applied) |
| 13 | a phone retired on L while P, not yet told, purges: P waits |
| 14, 14b | a removal younger than thirty days is never erased; a wrong clock only moves the wait |
| 15 | a book deleted with its records, purged across all four devices |
| 16 | a correction kept over a removal where the purge hadn't arrived: discarded, named, gone everywhere |
| 17 | a retired phone's change reaches L before the purge does; L discards it, and every device goes on taking L's files |
| 19 | a book brought back late: its sowings gone, the crop removed on its own still there, removed |
| 21 | a record removed on its own in a live book goes thirty days later |
| 22 | L and P delete one book separately while Q removes a sowing: two and three agreeing removals, erased everywhere |
| 22b | Q, hearing of one deletion only, brings the book back: L waits for Q to hold both; Q's restore is a conflict, not a discard |
| 23 | a crop removed while a sowing named it, its book merged (on one device, and on two the same way): the merged book erased everywhere |
| 24 | a phone not yet heard from records into a book L deleted and purged: a record in a removed book on every device, files taken both ways |
| 25 | a phone that holds a removal later edits a record another device moved out of that book: a conflict, files taken both ways |
| 26 | a phone not heard from corrects a record as its book is deleted, then keeps the deletion in the review; a phone brings the book back and edits on top: every device converges, the record gone |

Wherever a situation erases a book or a crop, it is the records that are erased;
the book and the crop stay, removed.

**Random histories**: `terrazgo-core/tests/random_histories.rs`. Three devices
make random edits, removals, book deletions, restores, merges, stray moves,
resolutions and purges, exchanging whole and trimmed files, and each history must
end with every device holding what a fresh device joining would hold (tables,
review queue, records in a removed book) and one log. Thirty-two histories run
with the suite (four seconds); a thousand longer ones with `cargo test -p
terrazgo-core --release --test random_histories -- --ignored`, after any change
to the merge layer. It has found what the scenario tables hadn't: one log and two
books, a resolution failing on a `UNIQUE` index, and a purge whose erased book a
later version still named. Taking out any of those fixes makes it fail. The seeds
are fixed and the clock isn't, so a failure names its seed and every act. Some
failures are rare enough that thirty-two histories miss them; scenarios 24 and
25 cover the purge's in the suite.

**Over the whole schema**: `src-tauri/tests/contracts/random_histories.rs`,
where every crate is in view. The three devices start from the demo campaign and
also record, correct (plots, notes, what a record points at in another register)
and remove every module's records, from a small pool so that two devices
recording one operation write a pair. They save change sets that write several
registers (a cover with its mowing and grazing, a seed treatment withdrawing its
book's "no treated seed" declaration, a duplicate kept with its copy's removal);
fill slot-keyed registers (that declaration, carried between books by a merge,
and a zone check); judge duplicates (distinct, keep one, bring back one of a pair
removed twice over); acknowledge an alert; and give the book acts the duplicate
policies the app gives them. An act failing on a raw database error instead of a
named refusal also fails it. Sixteen histories run with the suite; a thousand
longer ones with `cargo test -p terrazgo --release --test contracts
random_histories -- --ignored`, after any change to the merge layer or to a
register. `merge_scenarios.rs` covers what it found on a plot's water
declaration (A branch is applied a row at a time).

### What stays out

- Acts about passed deadlines, and duplicate verdicts: an act is a live register,
  so the purge never takes one; an act about an erased record matches nothing.
- Erasing what records point at (books, crops, sowings, irrigation records, soil
  covers; plots, operators, farms…).
- A button to erase now.
- The exporter's submission log, which will decide when a record that may have
  reached the authority can go.
- Compacting live history.

## Export numbers and alert acknowledgements

### `export_alias` collisions — one submitting device per farm, built with the exporter

Each exported record's SIEX number (`IdAjena*`, `MAX(alias) + 1` per target) is
its identity at the authority: sent again to edit it, with `Borrar` to delete
it. So two devices exporting before they sync could give one number to two
records, or two numbers to one. **This is prevented rather than repaired: each
farm has at most one submitting device, and only that device creates the farm's
numbers.**

- **Per farm, because a farm is what SIEX receives** (one `CodigoRea`, one CUE),
  so a device may submit none, one or several of its farms. By default it is the
  device that created the farm; a device that receives the farm by sync never
  claims it; a person moves it in Settings, farm by farm. It is a device, not a
  profile: one person with two devices would bring the collisions back.
- **Two devices claiming one farm offline is an alarm for that farm**, not a
  queue entry: both may have submitted, so the farm's export closes everywhere,
  every screen of it says so, and the message says to check the authority for
  activities sent twice.
- **Numbers become per farm** (today there is one sequence per database). Still
  to find out first: whether `IdAjena` must be unique per explotación (Anexo
  VI's wording) or per sender. If per sender, the role falls back to one device
  per database.
- It also records submissions (`submitted_at`, together with the delivery path).

**Built with the exporter, not before**: nothing can create a number while the
export has no button, and bringing the export back requires this first. Until
then `UNIQUE (target, alias)` stays, and two devices that did give one number to
two records would refuse each other's files loudly. A collision rule for a
handover claimed on two devices was built and then reverted (`2e0a418`). What it
showed: the number can't be `UNIQUE` once two devices can create numbers, the
outcome is a pure function of the claims, and only the device that created a
losing number renumbers it.

### Alert acknowledgements roam

Alerts are worked out when read and never travel (docs/data-model.md → "Alerts:
the settled design"). **What a person does about them travels**, as one
insert-only `alert_acknowledgement` row per act: two devices acting on one alert
write two registers and can't conflict, and the status (worked out when the list
is read, stored nowhere) is the strongest act (`AlertStatus` is declared `Active
< Acknowledged < Dismissed`, so "dismiss wins" is `max`). One row per alert with
dismiss winning by rule would be a register two devices write as two heads, and
resolving it by rule would need a per-table order in `live_head` and
`contributing`.

**An act names the deadline it saw** (`due_date`; empty for standing zone
alerts), because a licence or an ITV comes back under the same key, and a 2026
dismissal must not silence 2031. So a date corrected while the alert is live comes
back unseen, and a plot that re-enters a zone stays dismissed. An act that
changes nothing isn't written.

**Measured, not pruned** (`bundle_size.rs`): an act is 1 249 B in the database,
three quarters of it log, and 49 B in a file; pruning one costs +604 B, since it
is a logged delete. Old acts cost nothing to read: the list looks up today's
subjects (`idx_alert_acknowledgement_subject`), checked by a counting test.

## Duplicate suspects

Two workers recording one spray produce two valid rows, and **no sync algorithm
can resolve that**, since nothing conflicts. **A natural key raises a suspicion
and never identifies a record**: a person decides, and a machine drops nothing.
Strategy: [architecture.md](architecture.md).

### One rule per register, declared beside it

A register's rule (a Rust constant in its own crate, collected by the shell) says
when two of its live records on one farm look like one operation recorded twice.
It has up to three tests, all of which must hold: **when** (dates within N days,
periods overlapping, or the same book), **what** (columns that must hold the same
value; some count two empties as the same and some only a stated value), and
**where** (child sets sharing a value: plots, herds). A pair matching any rule is
a suspect. **What is compared is what a record stored, never another user row's
id**: two phones adding one product hold two product rows with one authorisation
number. "Within 1 day" is the realistic slip when entries are made late.

| Register | When | What | Where |
| --- | --- | --- | --- |
| `treatment_record` | within 1 day | authorisation number, non-chemical measure | treated plots |
| `non_field_treatment` | within 1 day | subject kind, premises or produce, authorisation number | — |
| `seed_treatment` | within 1 day | treatment kind, product registration number | sown plots |
| `fertilisation_record` | within 1 day | fertilisation type, material code | plots |
| `irrigation_record` | periods overlap | — | plots |
| `sowing_record` | within 1 day | kind | sown plots |
| `harvest_record` | same day / same book | product code and quantity / the delivery note, stated | harvested plots / — |
| `analysis_record` | same book | bulletin number, stated | — |
| `cultural_operation` | within 1 day | practice, operation kind | plots |
| `soil_cover` | same book | practice, cover type | plots |
| `grazing_record` | periods overlap (open end: still grazing) | — | plots, herds |
| `crop` | same book | its plot and the species code / its plot and the species name | — |
| `fertilisation_plan` | same book | — | planned crops |

**A new register can't be forgotten**: every register of the book (with a
`season_id`) declares rules, or `Detection::Never` with the reason.
`duplicate_rules_contract.rs` refuses one that did neither, runs every rule under
`EXPLAIN QUERY PLAN`, and refuses a rule no form names in `checkSaved`.

### Worked out when read, stored nowhere

A stored list would go stale with every edit, deletion, verdict or resolution
made elsewhere, and would never catch two copies typed on one laptop. So each
register's rules run when the list is read, on `(farm_id, <first day>)` or the
book index. **Limited to what is asked**: the Status view lists pairs with a
record in a **current book** (its campaign ended less than a year ago, or hasn't
ended; a date, since nothing archives a book), and a book's page lists its own
pairs whatever its age. The other record can be in any book of the farm. An
import says how many pairs are waiting, and names its sender
(`bundle::sender_label`).

### SQL fetches, Rust pairs

SQL decides which records are read: per register, one statement for its records
and one per compared child set, limited in scope and on an index, which the
contract test checks. Rust decides which pairs (`find_pairs`): group by what must
be equal, sort by first day, and go through each group once. Asking SQLite for
the pairs directly cost 1.1 s for the Status view on a farm with 4 000
treatments a campaign, about a microsecond per candidate in the window, in the
plot-overlap `EXISTS`, which no index improved. Nothing is read only to be thrown
away, so this isn't the filtering in Rust that the project's rules forbid.

#### Which records are compared

A register compared by book: the live records of the books in scope. A dated
register: every live record of each farm whose first day falls in that farm's
**span** (the scope's earliest first day to its latest last day, widened by the
slack), which also reaches a twin book of the same campaign. A pair is listed
when at least one of its records is in scope. Not reached from the Status view:
a record of an older book that began before the span and runs into it (a grazing
never closed). That one is found from its own book's page.

#### What it costs

`cargo test -p module-phytosanitary --release --test duplicate_cost -- --ignored --nocapture`,
with ten campaigns behind and one treatment in a hundred doubled:

| Farm | Treatments in current books | Pairs | Status view | Book page | After a save |
| --- | --- | --- | --- | --- | --- |
| Medium | 804 | 4 | 4.3 ms | 2.6 ms | 0.57 ms |
| A cooperative's year (400 plots) | 8 040 | 40 | 43 ms | 20 ms | 2.1 ms |
| The same on 120 plots, one product everywhere | 8 040 | 1 652 | 98 ms | 44 ms | 2.1 ms |

A save pays nothing measurable for the extra indexes (A/B). A counting test
(`duplicates.rs`) checks the work is the same with two campaigns behind or ten.

### The same rule, right after the form saves

**One rule serves the form and the list**, and **the form asks right after the
save, never before it**. The save is unchanged; then
`repository::list_saved_duplicates` asks which pairs the save put its record in
(for every register the change set wrote: a soil cover's save writes a mowing
and a grazing), and the pair review opens straight away. The rule reads the
record as stored, so nothing is worked out twice; the person sees both records
and can keep either; and **a bug here can't stop a treatment being recorded**,
since the check runs after the commit. The cost is that a caught duplicate is
written and then removed: two change sets, about 0.5 KB. Checking before the
save was considered and not used: it needs a second calculation of every
compared value per register, or a save that refuses, which nothing else in the
app does.

The fetch reads near the record (`duplicates::record_candidates_sql`) and finds
exactly the pairs its book page lists for it; each crate checks this with
`terrazgo_testkit::duplicates::assert_saved_pairs_match_the_book`. A form calls
`checkSaved(register, id)` (`src/lib/savedCheck.js`) after saving; the book
page's duplicates panel reviews each pair in turn. **The form is a convenience;
the list is the guarantee**: a record that arrives any other way is listed just
the same.

### A person's verdict roams, as an act

Like alert acknowledgements, in core's `duplicate_verdict`: one insert-only row
per act, with `subject_table` (from the rule, never from the screen),
`first_id`/`second_id` (the smaller first), `verdict` (`distinct` or
`duplicate`) and `kept_id`. It names the pair and nothing else; a pair doesn't
come back. **The reason is the verdict**: *keep this one* writes the other
record's soft delete and the `duplicate` act in one change set. A pair judged
`distinct` isn't listed again.

### Two people removing opposite copies — reported, and restorable

*Keep A* on one device and *keep B* on another are two soft deletes of two
registers: nothing conflicts, and the operation is now in the book zero times.
No offline design can prevent it, so it is reported: a pair removed by opposite
acts is listed, in the danger colour, with *restore this one*, which undoes its
removal. Deleting and merging books are refused while such a pair is waiting;
the purge never erases one; a merge never carries one.

### How the list is ordered

Pairs first written on two devices or by two people come first, then the most
recent. The order ranks; it never decides.

### The screens

On the Status view, between the conflicts and the alerts, in the alerts' colour
(a suspicion, not two answers the book holds); a pair removed twice over leads
with the consequence, *"Ya no está en el cuaderno"*. On a book's page, one line
above the tabs opens the same cards. **The review of a pair is generic**
(`repository::review_pair`, using `merge::review`'s comparison): child rows are
lined up by their `UNIQUE` key minus the parent column, every stated field is
shown with the differences marked as printed, and when two copies are identical
it says the choice doesn't matter. The columns are *Registro 1* and *2*, each
headed by device, person, day, campaign and farm.

## Merging two books that turned out to be one campaign

Two devices that open one campaign with different dates write two books (Seasons
created on two devices). Just pointing every record at the other book and
deleting this one loses records in these cases, which the design below handles:

| | Case | What happened |
| --- | --- | --- |
| P0 | An empty book deleted on one device while another records into it (no merge at all) | the record live in a deleted book, shown nowhere |
| P1 | A record written elsewhere before the merge arrives | the same |
| P2 | An edit elsewhere at the time of the move | the edit live in the deleted book |
| P3 | A merge with a conflict waiting | the conflict settled without anyone seeing |
| P4 | Two devices merging one pair the same way | a conflict per record, saying nothing |
| P5 | Two devices merging opposite ways | both books deleted |
| P6 | A book whose only records are removed | could not be deleted |

### The merge

**The person chooses which book stays**, pre-selected the same way on every
device: the one named by its dates if exactly one is, otherwise the older one
(`repository::kept_by_default`). Keeping a fixed book and letting the person
choose the *name* was tried and dropped: reusing the absorbed book's name while it
could still come back elsewhere made both devices refuse each other's files.
What choosing costs is P5, which needs two people merging one pair in opposite
ways offline; it loses nothing, and bringing a book back undoes it.

**One change set, generic** (`repository::merge_books`): every live record of
the absorbed book is moved (tables from the aggregate map, each image from the
log) and the absorbed book is deleted. A slot keyed by the book (a declaration)
is withdrawn and stated in the kept book only where that book states nothing.
Children belong to their register and aren't touched. **Removed records stay
where they are**, removed with the book, since they are records no screen shows,
**except the removed rows a moved record names**, which go with it as they are
and still removed, together with what they name in turn; only their rows naming
a book are written (`carry::carry_what_they_name`). If they were left behind, a
sowing naming a crop removed before the merge would name a row of a removed
book, and its form, which lists its book's crops, couldn't show the crop it
names. One of two opposite duplicate removals is never carried, since writing on
top of it would take away the option of bringing one of the pair back. Moving
records out of a removed book carries what they name in the same way. These rows
are found through the absorbed book's removed rows, on their partial indexes, so
a merge that carries nothing costs no more.

**Refused while a conflict or two opposite removals wait in the absorbed book**
(P3): each has its answer on the Status view.

Measured (`book_merge_cost.rs`): 4 000 treatments merge in 1.7 s, add 17 MB to
the log (recording them logged 21 MB) and travel in a 197 KB file; a move is an
ordinary logged edit of every record.

### Records in a removed book

P0–P2 are one state, and the merge isn't the only way into it (a restore, a
resolution to a version naming an old book), so it is **worked out when read**:
the Status view lists every live record in a removed book, grouped by book, and
an import says how many there are. There are two acts: *move them* to a live book
of the farm (the suggestion is the one whose campaign overlaps most), carrying
what they name as a merge does; or *bring the book back*. A record with a
conflict waiting is resolved first. A treatment in a removed book still raises
its plazo. The list starts from `idx_season_removed`. There is no `merged_into`
column: it would be a schema change, a chain once books are merged twice, and no
answer for P0.

### The screens

**The merge is only offered on a book's page with a book whose campaign overlaps
it** (`repository::merge_candidates`): twins always share days and consecutive
campaigns never do, so the offer appearing is itself the sign of a twin. The
panel picks the other book and which one stays, pre-selected, with a line on why
leaving the default is safe. On the Status view, records in a removed book sit
between the conflicts and the duplicates, in the conflicts' colour, one card per
book with *Pasar los registros* and *Recuperar el cuaderno*. The collision
message mentions the merge.

Not built: offering the merge at the collision itself (a rename, the import and
the merge as one flow), and prefilling a new book's dates from the farm's last
one. The second is the cheaper prevention, since two devices accepting the
prefill create one id.

## What stays device-local — and what that costs

`settings.json` (the device id), `geo-cache.db`, the catalogues,
`sync_conflict`, `sync_known` and `sync_held`, plus the alerts and duplicate
suspects, which are stored nowhere.

**Catalogue skew** (a record naming a code this device doesn't have) can happen:
FEGA adds codes between releases (the crop catalogue gains over a hundred a
year), a phone that refreshed its catalogues records one, and the laptop that
didn't receives it. It can't break the apply (no foreign key runs from user data
to a catalogue code); the tier-1 lookups that do have foreign keys are seeded by
migrations and so are equal under the equal-schema rule; and an unresolved code
shows as its raw value, which is the record's legal value anyway.

**The book says so**: the advisory's `unnamed_codes` counts the codes the book
prints without a name. The book's assembly resolves every printed code through
one cache, which notes each code no catalogue row names, and the advisory runs
that same assembly, so the count is what the page shows, whatever the cause (a
newer device's record, catalogues never imported), and it needs no list of which
column holds which catalogue's codes. It costs the advisory one assembly: 14 ms
on a 400-treatment book, 184 ms at 4 000 (release, desktop).

**Such a record can be corrected here.** A save checks against this device's
catalogue only the codes it adds or changes (problem codes, the growth stage,
the non-chemical measure, `MAT_FERTI`), never one the record already has: that
one was checked where it was written, and reference data never stands between a
farmer and a lawful record. A code a save adds can only come from this device's
own pickers, so an unknown one there is still refused.

**And its form shows the code.** Every picker list has all the codes the device
holds, marked `offered` (`catalogue::held_picks`). A picker offers the live ones,
names a stored one it no longer offers, and shows a code this device doesn't have
as the code itself, with one line pointing to the catalogue refresh. Otherwise
the field would look empty, and a farmer filling it in would replace a real
value. The lists show a code the authority has since retired in the same way.

**And the import says the catalogues may need updating.** `ImportSummary.books`
lists the books the file's newly applied change sets wrote into (never those
already held, so a whole-log file only costs what it changed), and the shell
reads each as the book prints it (`terrazgo_recordbook::prints_unnamed_codes`, an
assembly per book, stopping at the first yes). "May", because a code the file
brought outside its books (a material's kind, a premises class) isn't read
there; the book's advisory still counts what it prints. It is said after the
import, not in the dialog before: the records aren't in the tables until it
applies, and nothing in the import depends on the catalogues, so updating
afterwards names every record already received. Not used for this: code counts
per catalogue in the manifest (silent when two devices hold the same number of
different codes, and a format change), a hash of the set (no direction, and
devices that never delete a code never converge), and the manifest naming the
codes its change sets use (it needs the column-to-catalogue list across four
modules, which `maintenance.md` §1 rejects for the same reason).

## Rejected alternatives

| Rejected | Why |
| --- | --- |
| Row-level merge | notices nothing in the dose-against-plot case |
| Field-level last-writer-wins | mixes two people's edits into a record neither wrote |
| Prefer the local branch | cheaper, but devices disagree until somebody acts |
| A full CRDT | too much for a few writers rarely touching one row; per-register vectors are the right weight |
| Ids derived from content for records | fragile (1,5 against 1,49) and wrong for near matches; `season` is the one exception, on an exact key |
| One acknowledgement row per alert | a register two devices write as two heads; one row per act can't conflict |
| Splitting SIEX numbers by device | needs agreement between devices that the app can't reach |
| Syncing catalogues, or copying their labels onto records | catalogues are a curated, versioned set; labels are never copied |
| A `change_set` side table | a join for sixty bytes, and a log row that can no longer be read alone |

Kept in reserve: carrying the `(catalogue, code, label)` triples a file uses,
with a provenance of their own. That would need its own design.

## Peer discovery and a live transport (not built)

**Bluetooth LE is for keeping in step, not for a device joining**: at 10–65 kB/s
in the field, a day's changes take seconds and catching up a season a minute or
two, but a first sync of 6–60 MB takes fifteen minutes to over an hour, so
joining needs Wi-Fi or the file. **A listener would be the first inbound network
surface** in an app whose only network path, `terrazgo-net`, is outbound only and
enforced by the build. So a live transport belongs in a new crate next to it,
with the shell as its only user, or that rule has to be rewritten on purpose. The
messages don't change; only the framing does. Also not covered: iOS (no build
yet), and farm↔user membership and roles.
