// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Render the demo record book to disk, in every language, as both outputs —
//! the eyeball check that follows a section landing in the book.
#![allow(clippy::unwrap_used, clippy::expect_used)]

fn main() {
    let mut conn = terrazgo_recordbook::open_in_memory().unwrap();
    terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
    let summary = module_cue::demo::seed_demo(&mut conn).unwrap();
    assert!(summary.seeded);
    let (season_id, farm_id): (String, String) = conn
        .query_row(
            "SELECT season_id, farm_id FROM treatment_record LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let out = std::env::args()
        .nth(1)
        .expect("usage: cargo run -p terrazgo-recordbook --example render_demo -- <output-dir>");
    for language in terrazgo_recordbook::ReportLanguage::ALL {
        let code = language.code();
        let pdf = terrazgo_recordbook::render_cuaderno(
            &conn,
            &season_id,
            &farm_id,
            "2026-08-09",
            language,
        )
        .unwrap();
        assert_eq!(
            pdf.warnings,
            Vec::<String>::new(),
            "template warnings in {code}"
        );
        std::fs::write(format!("{out}/cuaderno_{code}.pdf"), &pdf.bytes).unwrap();
        let xlsx = terrazgo_recordbook::render_cuaderno_xlsx(
            &conn,
            &season_id,
            &farm_id,
            "2026-08-09",
            language,
        )
        .unwrap();
        std::fs::write(format!("{out}/cuaderno_{code}.xlsx"), &xlsx.bytes).unwrap();
        println!("{code}: {} pages", pdf.page_count);
    }
}
