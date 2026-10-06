// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Every shell-level contract test, in ONE binary.
//!
//! Cargo compiles each file directly under `tests/` as its own crate and links
//! it against the whole dependency graph — here that is tauri plus typst, and a
//! debug binary of it weighs about 670 MB. Eighteen of those is ~12 GB of
//! build output and eighteen full links, which is what exhausted the CI
//! runner's disk in 2026-09: `ld` mmaps its output, the filesystem fills, and
//! touching the mapped pages raises SIGBUS rather than a legible ENOSPC.
//!
//! Cargo auto-discovers `tests/<name>/main.rs` as a SINGLE target called
//! `<name>`, and files in a subdirectory of `tests/` are not targets of their
//! own — so each contract is a module of this one binary: one link, one
//! binary, the same tests. (This is a crate root, not a module file, so the
//! project's no-`mod.rs` rule does not reach it; `mod foo;` here resolves to
//! `tests/contracts/foo.rs` because that is the directory this root lives in.)
//!
//! **A new contract test is a file in `tests/contracts/` and one `mod` line
//! here.** Putting it directly in `tests/` still works and still passes, which
//! is exactly why it is worth knowing that it also silently adds another
//! 670 MB link to every CI run.
//!
//! Running one of them is `cargo test -p terrazgo --test contracts <filter>`,
//! where the filter is the module name (`neutral_voice`, `index_contract`, …)
//! or any test name inside it.
//!
//! Each module keeps its own `#![allow(clippy::unwrap_used, clippy::expect_used)]`
//! and its own doc comment saying what it guards; nothing about the tests
//! themselves changed when they moved here.

mod alert_kinds_contract;
mod applier_contract;
mod book_merge_contract;
mod command_registration;
mod db_shutdown;
mod duplicate_rules_contract;
mod form_feedback;
mod hardened_connections;
mod i18n_contract;
mod index_contract;
mod lookup_scope;
mod migration_composition;
mod neutral_voice;
mod number_formatting;
mod overscroll_contract;
mod quick_check_cost;
mod random_histories;
mod registry_hints;
mod schema_features;
mod season_link_contract;
mod spdx_headers;
mod startup_gate;
mod statement_cache;
mod sync_fields_contract;
mod sync_shape_contract;
mod third_party;
