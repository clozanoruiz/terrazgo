// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! How big the demo record book is, in both languages.
//!
//! **A count, asserted because a section can only go missing quietly.** Every
//! other test in this directory checks that a cell says the right thing; none
//! of them notices a whole page that stopped being emitted, because a page
//! nobody asks about is a page nobody misses. A section of the book is a legal
//! duty, so losing one silently is the failure worth a test of its own.
//!
//! The numbers lived in a status banner until 2026-08-24, where nothing
//! verified them and they were not in the doc that banner pointed at. They are
//! here instead: if a section legitimately arrives or leaves, this test is the
//! thing that says so out loud, and updating it is the deliberate act.
//!
//! **Both languages, because the layout is per COUNTRY and must not fork.** A
//! Castilian book with more pages than the Catalan one would mean the
//! translation had changed the document rather than its words — which is
//! exactly what the `Labels` struct exists to make impossible, checked here
//! from the outside.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use terrazgo_recordbook::ReportLanguage;

/// The demo holding's book. Update deliberately when a section lands, and say
/// which section in the commit — never to make a red test green.
const PAGES: usize = 15;
const SHEETS: usize = 23;

/// A fixed date, so the book does not change shape on the day a PHI window or
/// an advisory deadline happens to lapse.
const GENERATED_ON: &str = "2026-08-09";

#[test]
fn the_demo_book_is_the_same_size_in_every_language() {
    let mut conn = terrazgo_recordbook::open_in_memory().unwrap();
    terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
    assert!(module_cue::demo::seed_demo(&mut conn).unwrap().seeded);

    let (season_id, farm_id): (String, String) = conn
        .query_row(
            "SELECT season_id, farm_id FROM treatment_record LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();

    for language in ReportLanguage::ALL {
        let code = language.code();

        let pdf = terrazgo_recordbook::render_cuaderno(
            &conn,
            &season_id,
            &farm_id,
            GENERATED_ON,
            language,
        )
        .unwrap();
        assert_eq!(
            pdf.warnings,
            Vec::<String>::new(),
            "the {code} template rendered with warnings"
        );
        assert_eq!(
            pdf.page_count, PAGES,
            "the {code} book is {} pages, not {PAGES} — a section arrived or went \
             missing. If it arrived, update the constant and name it in the commit.",
            pdf.page_count
        );

        let xlsx = terrazgo_recordbook::render_cuaderno_xlsx(
            &conn,
            &season_id,
            &farm_id,
            GENERATED_ON,
            language,
        )
        .unwrap();
        assert_eq!(
            xlsx.sheet_count, SHEETS,
            "the {code} workbook has {} sheets, not {SHEETS}",
            xlsx.sheet_count
        );
    }
}
