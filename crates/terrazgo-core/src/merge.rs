// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Turning a merged log into the tables a farmer reads.
//!
//! Log replication itself needs nothing from this module: `record_change` rows
//! are immutable and uniquely identified, so replicating them is a set union
//! and the history survives every merge by construction. **What lives here
//! is the other half — given a register's merged log, what the tables hold**
//! (docs/sync.md → "Two layers, and only one of them has policy").
//!
//! The two layers are one file each, and the split is the point:
//!
//!   * [`applier`] writes rows and **decides nothing**. Handed a table and an
//!     image, it makes the table hold it.
//!   * [`head`] decides. Which versions of a register are current, which of
//!     them is live, and what has to be undone and replayed to make it so.
//!
//! A third file sits above both, and it is the only one a person reaches:
//! [`review`] shows the versions a machine may not choose between, and writes
//! down the choice. It decides nothing either — the person does.
//!
//! Nothing here merges the log or transports it. A register's log is assumed
//! already complete on this device — every set it holds arrived with the sets
//! it was built on. How rows get here is the bundle's problem, and the import
//! makes sure of it: it refuses a file unless the device ends up holding
//! everything the sender held (`bundle::refuse_if_incomplete`).

mod applier;
mod head;
mod review;

pub use applier::Applier;
pub(crate) use applier::quote_ident;
pub use head::{
    BranchState, Head, REGISTER_HEADS_SQL, branch_states, heads, live_head, make_live, settle,
    waiting,
};
pub(crate) use head::{CLEAR_CONFLICT_SQL, REGISTER_ROWS_NAMED_SQL, state_is_removed};
pub(crate) use review::compare_records;
pub use review::{
    CORE_ROW_CAPTIONS, ConflictReview, ReviewLine, ReviewValue, ReviewVersion, RowCaption, resolve,
    review,
};
