// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The alert crates, their kinds and the dictionaries, held against each other
//! (docs/data-model.md → "Alerts: the settled design").
//!
//! There is no kinds table: a kind is a constant its crate declares, and the
//! shell's `alerts::ALERT_CRATES` is the one list of crates that raise alerts.
//! Nothing in the database would catch two crates sharing a code, or a kind
//! shipped without its words — so this file does, in both directions:
//!
//!   * every declared kind has its text in every locale, and every alert text
//!     in a dictionary belongs to a declared kind. The second direction is what
//!     catches a crate whose alerts were written, worded, and never added to
//!     the list: its text would be here with no kind behind it;
//!   * the same for each crate's own name, which the Status view shows when
//!     that crate's alerts could not be worked out;
//!   * no code is declared twice, and no crate is listed twice;
//!   * every kind is about a table that syncs. The kind is the only thing that
//!     names its subject's table — an act takes it from there, never from the
//!     screen — so a misspelt or device-local table here is the one way left
//!     to file acts under rows no other device can find.

use std::collections::{BTreeMap, BTreeSet};

use crate::i18n_contract::dictionaries;
use terrazgo_core::sync::SyncRole;
use terrazgo_lib::alerts::ALERT_CRATES;
use terrazgo_lib::registry::composed_sync_shape;

/// Every declared code, with the crate that declares it.
fn declared_codes() -> Vec<(&'static str, &'static str)> {
    ALERT_CRATES
        .iter()
        .flat_map(|alert_crate| {
            alert_crate
                .kinds
                .iter()
                .map(|kind| (kind.code(), alert_crate.name))
        })
        .collect()
}

/// The keys under `prefix` in one dictionary, with the prefix taken off.
fn suffixes<'a>(dictionary: &'a BTreeMap<String, String>, prefix: &str) -> BTreeSet<&'a str> {
    dictionary
        .keys()
        .filter_map(|key| key.strip_prefix(prefix))
        .collect()
}

#[test]
fn no_alert_code_is_declared_twice() {
    let mut first: BTreeMap<&str, &str> = BTreeMap::new();
    for (code, crate_name) in declared_codes() {
        if let Some(other) = first.insert(code, crate_name) {
            panic!(
                "`{code}` is declared by both {other} and {crate_name}: an act on one \
                 would be read as an act on the other"
            );
        }
    }
    assert!(
        !first.is_empty(),
        "no kinds declared at all — the rest of this file would check nothing"
    );
}

#[test]
fn every_kind_is_about_a_table_that_syncs() {
    // An act on an alert syncs, and names its subject by table and id. A table
    // the aggregate map does not declare is a typo or a lookup; a `Local` one
    // is derived or device-local, so its ids mean nothing on another device.
    // Either way the act would roam and match nothing there.
    let shape: BTreeMap<&str, SyncRole> = composed_sync_shape()
        .into_iter()
        .map(|entry| (entry.table, entry.role))
        .collect();
    for alert_crate in ALERT_CRATES {
        for kind in alert_crate.kinds {
            let table = kind.subject_table();
            match shape.get(table) {
                None => panic!(
                    "{}'s `{}` is about `{table}`, which the aggregate map does not declare",
                    alert_crate.name,
                    kind.code()
                ),
                Some(SyncRole::Local) => panic!(
                    "{}'s `{}` is about `{table}`, which never syncs",
                    alert_crate.name,
                    kind.code()
                ),
                Some(_) => {}
            }
        }
    }
}

#[test]
fn no_alert_crate_is_listed_twice() {
    let mut names = BTreeSet::new();
    for alert_crate in ALERT_CRATES {
        assert!(
            names.insert(alert_crate.name),
            "{} is on the list twice, so its alerts would be listed twice",
            alert_crate.name
        );
    }
}

#[test]
fn every_declared_kind_has_its_text_in_every_locale() {
    for (locale, dictionary) in dictionaries() {
        for (code, crate_name) in declared_codes() {
            let key = format!("alert.type.{code}");
            assert!(
                dictionary.contains_key(&key),
                "{locale} has no `{key}` for {crate_name}'s kind — the card would print \
                 the bare code"
            );
        }
    }
}

#[test]
fn every_alert_text_belongs_to_a_declared_kind() {
    let declared: BTreeSet<&str> = declared_codes().into_iter().map(|(code, _)| code).collect();
    for (locale, dictionary) in dictionaries() {
        for code in suffixes(&dictionary, "alert.type.") {
            assert!(
                declared.contains(code),
                "{locale} words an alert `{code}` that no crate on the shell's \
                 ALERT_CRATES declares — either a crate is missing from the list, \
                 or the text outlived its kind"
            );
        }
    }
}

#[test]
fn every_alert_crate_has_its_name_in_every_locale() {
    for (locale, dictionary) in dictionaries() {
        for alert_crate in ALERT_CRATES {
            let key = format!("alert.source.{}", alert_crate.name);
            assert!(
                dictionary.contains_key(&key),
                "{locale} has no `{key}` — the notice for this crate's missing alerts \
                 would print its internal name"
            );
        }
    }
}

#[test]
fn every_alert_source_text_belongs_to_a_listed_crate() {
    let listed: BTreeSet<&str> = ALERT_CRATES.iter().map(|c| c.name).collect();
    for (locale, dictionary) in dictionaries() {
        for name in suffixes(&dictionary, "alert.source.") {
            assert!(
                listed.contains(name),
                "{locale} words an alert source `{name}` that is not on ALERT_CRATES"
            );
        }
    }
}
