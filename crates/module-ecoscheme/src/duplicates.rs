// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! When two of this module's records look like one operation recorded twice
//! (docs/sync.md → Duplicate suspects). One rule per register, declared here
//! beside the tables it reads and listed by the shell; core runs them.

use terrazgo_core::duplicates::{
    Detection, DuplicatePolicy, DuplicateRule, Overlap, Period, Same, When,
};

/// Two flocks on one plot at once are legitimate; one flock recorded twice is
/// not — so the periods must meet, the plots overlap AND a herd (its REGA code)
/// be on both. An open end is a grazing still under way, which meets every day
/// after it began.
pub const GRAZING_DUPLICATES: DuplicatePolicy = DuplicatePolicy {
    table: "grazing_record",
    detection: Detection::Rules(&[DuplicateRule {
        when: When::Within {
            period: Period::Ongoing {
                from: "started_on",
                to: "ended_on",
            },
            days: 0,
        },
        same: &[],
        overlaps: &[
            Overlap {
                table: "grazing_plot",
                parent: "grazing_record_id",
                column: "plot_id",
            },
            Overlap {
                table: "grazing_animal",
                parent: "grazing_record_id",
                column: "rega_code",
            },
        ],
    }]),
};

/// One operation recorded twice: the same practice and kind of work on
/// overlapping plots within a day.
pub const CULTURAL_OPERATION_DUPLICATES: DuplicatePolicy = DuplicatePolicy {
    table: "cultural_operation",
    detection: Detection::Rules(&[DuplicateRule {
        when: When::Within {
            period: Period::Days {
                from: "performed_on",
                to: "performed_end_date",
            },
            days: 1,
        },
        same: &[
            Same::Value("practice_code"),
            Same::Value("operation_kind_code"),
        ],
        overlaps: &[Overlap {
            table: "cultural_operation_plot",
            parent: "cultural_operation_id",
            column: "plot_id",
        }],
    }]),
};

/// A cover is established once per plot and campaign, whatever date each
/// device gave it: the same practice and cover type on overlapping plots in one
/// book.
pub const SOIL_COVER_DUPLICATES: DuplicatePolicy = DuplicatePolicy {
    table: "soil_cover",
    detection: Detection::Rules(&[DuplicateRule {
        when: When::SameBook,
        same: &[Same::Value("practice_code"), Same::Value("cover_type_code")],
        overlaps: &[Overlap {
            table: "soil_cover_plot",
            parent: "soil_cover_id",
            column: "plot_id",
        }],
    }]),
};
