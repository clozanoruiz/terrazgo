// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! When two of this module's records look like one operation recorded twice
//! (docs/sync.md → Duplicate suspects). One rule per register, declared here
//! beside the tables it reads and listed by the shell; core runs them.
//!
//! **What is compared is what a record froze, never another user row's id.** A
//! fertiliser material is the farmer's own row, and two phones that each add
//! one offline hold two — so the material is compared by the code the record
//! snapshotted.

use terrazgo_core::duplicates::{
    Detection, DuplicatePolicy, DuplicateRule, Overlap, Period, Same, When,
};

/// One application recorded twice: the same type and material on overlapping
/// plots within a day — RD 1051/2022's register, shaped like the treatment
/// rule.
pub const FERTILISATION_DUPLICATES: DuplicatePolicy = DuplicatePolicy {
    table: "fertilisation_record",
    detection: Detection::Rules(&[DuplicateRule {
        when: When::Within {
            period: Period::Days {
                from: "applied_on",
                to: "application_end_date",
            },
            days: 1,
        },
        same: &[
            Same::Value("fertilisation_type_code"),
            Same::Value("material_code_snapshot"),
        ],
        overlaps: &[Overlap {
            table: "fertilisation_plot",
            parent: "fertilisation_record_id",
            column: "plot_id",
        }],
    }]),
};

/// Watering on consecutive days is ordinary, so no slack at all: two records
/// whose periods share a day on overlapping plots — which is also what a daily
/// record inside an accumulated fortnight looks like.
pub const IRRIGATION_DUPLICATES: DuplicatePolicy = DuplicatePolicy {
    table: "irrigation_record",
    detection: Detection::Rules(&[DuplicateRule {
        when: When::Within {
            period: Period::Days {
                from: "irrigated_on",
                to: "irrigation_end_date",
            },
            days: 0,
        },
        same: &[],
        overlaps: &[Overlap {
            table: "irrigation_plot",
            parent: "irrigation_record_id",
            column: "plot_id",
        }],
    }]),
};

/// A crop is in one plan — the form refuses a second on one device
/// (`crop_already_planned`) — so two plans in one book covering one crop are
/// two devices that each drew it up offline. The local guard cannot see the
/// other device; this is how the book reports it.
pub const PLAN_DUPLICATES: DuplicatePolicy = DuplicatePolicy {
    table: "fertilisation_plan",
    detection: Detection::Rules(&[DuplicateRule {
        when: When::SameBook,
        same: &[],
        overlaps: &[Overlap {
            table: "fertilisation_plan_crop",
            parent: "fertilisation_plan_id",
            column: "crop_id",
        }],
    }]),
};
