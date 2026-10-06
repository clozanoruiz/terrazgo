// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! When two of this module's records look like one operation recorded twice
//! (docs/sync.md → Duplicate suspects). One rule per register, declared here
//! beside the tables it reads and listed by the shell; core runs them.
//!
//! **What is compared is what a record froze, never another user row's id.** A
//! product is the farmer's own row, and two phones that each add one from the
//! catalogue offline hold two — so two records of one spray name two product
//! ids and one authorisation number.

use terrazgo_core::duplicates::{
    Detection, DuplicatePolicy, DuplicateRule, Overlap, Period, Same, When,
};

/// One spray, two operators, one of whom typed the wrong day or listed only
/// the plots they covered: the same product (or the same non-chemical measure)
/// on overlapping plots within a day.
pub const TREATMENT_DUPLICATES: DuplicatePolicy = DuplicatePolicy {
    table: "treatment_record",
    detection: Detection::Rules(&[DuplicateRule {
        when: When::Within {
            period: Period::Days {
                from: "application_date",
                to: "application_end_date",
            },
            days: 1,
        },
        same: &[
            Same::Value("authorisation_number_snapshot"),
            Same::Value("measure_code"),
        ],
        overlaps: &[Overlap {
            table: "treatment_plot",
            parent: "treatment_record_id",
            column: "plot_id",
        }],
    }]),
};

/// A store, a transport or a harvest treated once and recorded by the
/// applicator and by the manager: the same subject and product within a day.
pub const NON_FIELD_DUPLICATES: DuplicatePolicy = DuplicatePolicy {
    table: "non_field_treatment",
    detection: Detection::Rules(&[DuplicateRule {
        when: When::Within {
            period: Period::Day { on: "treated_on" },
            days: 1,
        },
        same: &[
            Same::Value("subject_kind_code"),
            Same::Value("premises_id"),
            Same::Value("subject_product_code"),
            Same::Value("authorisation_number_snapshot"),
        ],
        overlaps: &[],
    }]),
};

/// One seed lot's treatment recorded at the store and at the sowing: the same
/// kind and product on overlapping plots within a day.
pub const SEED_TREATMENT_DUPLICATES: DuplicatePolicy = DuplicatePolicy {
    table: "seed_treatment",
    detection: Detection::Rules(&[DuplicateRule {
        when: When::Within {
            period: Period::Day { on: "sown_on" },
            days: 1,
        },
        same: &[
            Same::Value("treatment_kind_code"),
            Same::Value("product_registration_number"),
        ],
        overlaps: &[Overlap {
            table: "seed_treatment_plot",
            parent: "seed_treatment_id",
            column: "plot_id",
        }],
    }]),
};

/// A laboratory numbers each bulletin once, so one bulletin number twice in a
/// book is one analysis entered twice.
pub const ANALYSIS_DUPLICATES: DuplicatePolicy = DuplicatePolicy {
    table: "analysis_record",
    detection: Detection::Rules(&[DuplicateRule {
        when: When::SameBook,
        same: &[Same::Stated("bulletin_number")],
        overlaps: &[],
    }]),
};
