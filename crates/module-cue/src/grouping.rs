// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! How one treatment record splits into several presented rows.
//!
//! This lives in the treatment domain rather than in either consumer because
//! **both** of them need the same split and must produce the same one: the
//! printed record book prints one section-3.1 row per crop group, and the SIEX
//! export emits one `TratamFito` per crop group (the 3.11.4 descriptor
//! constrains all DGCs in one entry to share the crop). A rule that two
//! documents must agree on belongs to the domain they both read, not to
//! whichever of them was written first — it lived in `module_cue::export` until
//! that export moved out to `terrazgo-siex` (2026-08-20), which made the shared
//! ownership visible.

use crate::models::TreatmentPlot;

/// Group treated plots by their frozen crop snapshot, sorted by key so the
/// output order is deterministic across runs and across the two documents.
///
/// Snapshots are frozen at insert, so a grouping can never drift between one
/// export and the next — which is what lets the SIEX side key a stable alias on
/// the group.
pub fn crop_groups(plots: &[TreatmentPlot]) -> Vec<(String, Vec<&TreatmentPlot>)> {
    let mut groups: Vec<(String, Vec<&TreatmentPlot>)> = Vec::new();
    for plot in plots {
        // \u{1F} (unit separator) never appears in species/variety text, so
        // the concatenated key cannot collide across groups.
        let key = format!(
            "{}\u{1F}{}",
            plot.crop_name_snapshot.as_deref().unwrap_or(""),
            plot.variety_snapshot.as_deref().unwrap_or("")
        );
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, members)) => members.push(plot),
            None => groups.push((key, vec![plot])),
        }
    }
    groups.sort_by(|a, b| a.0.cmp(&b.0));
    groups
}
