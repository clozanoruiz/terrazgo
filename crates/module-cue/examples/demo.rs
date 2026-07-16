// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Runnable demo + 30-second smoke test: seeds a demo database via
//! [`module_cue::demo::seed_demo`], then reads the treatment records back from
//! it and prints them cuaderno-style. The database file is left behind for
//! ad-hoc SQL inspection (`sqlite3 demo.db`).
//!
//! Run from the workspace root (re-running recreates the file — dev databases
//! are recreated, not migrated):
//!
//! ```sh
//! cargo run -p module-cue --example demo --features demo            # ./demo.db
//! cargo run -p module-cue --example demo --features demo -- path.db # custom path
//! ```
//!
//! Seeding goes through the public repository API (see `src/demo.rs`); the
//! printing below re-reads everything from the database, so the example also
//! smoke-tests the read path and row mappers.

use module_cue::models::TreatmentRecordWithPlots;
use module_cue::{demo, open, repository};

// `Box<dyn Error>` accepts any error type via `?` (`CueError` from the repository,
// `io::Error` from the file removal) — fine at a binary's top level. Library code
// keeps the precise `CueError` enum instead.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).unwrap_or_else(|| "demo.db".into());

    // Recreate from scratch on every run; remove the WAL sidecar files too so a
    // stale -wal can't resurrect rows from a previous run.
    for suffix in ["", "-wal", "-shm"] {
        let file = format!("{path}{suffix}");
        if std::path::Path::new(&file).exists() {
            std::fs::remove_file(&file)?;
        }
    }

    let mut conn = open(&path)?;
    let summary = demo::seed_demo(&mut conn)?;

    println!(
        "Seeded {path} — campaign {}",
        summary.season_label.as_deref().unwrap_or("—"),
    );
    println!();

    // --- farm, plots, operator, machinery — read back from the database -------
    let (farm_name, owner, country): (String, Option<String>, String) = conn.query_row(
        "SELECT name, owner_name, country_code FROM farm LIMIT 1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    println!(
        "FARM  {farm_name} · owner {} · country {country}",
        owner.as_deref().unwrap_or("—")
    );

    let mut stmt = conn.prepare("SELECT name, area_ha FROM plot ORDER BY name")?;
    let plots: Vec<(String, Option<f64>)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    drop(stmt);
    for (name, area_ha) in plots {
        println!("  plot {:<10} {:>4} ha", name, area_ha.unwrap_or(0.0));
    }
    println!();

    let (op_name, licence, level, expires): (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = conn.query_row(
        "SELECT full_name, licence_number, licence_level_code, licence_expiry_date
             FROM operator LIMIT 1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )?;
    println!(
        "OPERATOR  {op_name} · licence {} ({}) · expires {}",
        licence.as_deref().unwrap_or("—"),
        level.as_deref().unwrap_or("—"),
        expires.as_deref().unwrap_or("—"),
    );

    let (mach_name, next_itv, roma): (String, Option<String>, Option<String>) = conn.query_row(
        "SELECT m.name, m.next_inspection_due_date, x.roma_number
         FROM machinery m LEFT JOIN machinery_es_extension x ON x.machinery_id = m.id
         LIMIT 1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    println!(
        "MACHINERY {mach_name} · ROMA {} · next ITV {}",
        roma.as_deref().unwrap_or("—"),
        next_itv.as_deref().unwrap_or("—"),
    );

    for (n, id) in summary.treatment_ids.iter().enumerate() {
        let fetched = repository::get_treatment_record(&conn, id)?;
        print_treatment(&conn, n + 1, &fetched)?;
    }

    let changes: i64 = conn.query_row("SELECT COUNT(*) FROM record_change", [], |r| r.get(0))?;
    println!();
    println!("AUDIT  {changes} record_change rows · poke around with: sqlite3 {path}");
    Ok(())
}

/// Print one treatment the way the cuaderno will show it: only the frozen
/// snapshot values — what was legally true at application time. The plot name
/// is the one display field not snapshotted, so it is looked up by id.
fn print_treatment(
    conn: &rusqlite::Connection,
    n: usize,
    t: &TreatmentRecordWithPlots,
) -> Result<(), Box<dyn std::error::Error>> {
    let r = &t.record;
    println!();
    println!("TREATMENT {n} — {}", r.application_date);
    println!(
        "  product    {} (reg. {})",
        r.product_name_snapshot,
        r.authorisation_number_snapshot.as_deref().unwrap_or("—"),
    );
    println!(
        "  substances {}",
        r.active_substances_snapshot.as_deref().unwrap_or("—")
    );
    println!("  dose       {} {}", r.dose_value, r.dose_unit_code);
    let problems = t
        .problems
        .iter()
        .map(|p| format!("{}:{}", p.reason_category_code, p.problem_code))
        .collect::<Vec<_>>()
        .join(", ");
    println!(
        "  problems   {} — {}",
        problems,
        r.target_organism.as_deref().unwrap_or("—"),
    );
    let justifications = t
        .justifications
        .iter()
        .map(|j| j.justification_code.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    println!("  justified  {justifications}");
    println!(
        "  efficacy   {}",
        r.efficacy_code.as_deref().unwrap_or("not yet assessed")
    );
    println!(
        "  operator   {} (licence {})",
        r.operator_name_snapshot,
        r.operator_licence_snapshot.as_deref().unwrap_or("—"),
    );
    println!(
        "  machinery  ROMA {} · REGANIP {}",
        r.machinery_roma_snapshot.as_deref().unwrap_or("—"),
        r.machinery_reganip_snapshot.as_deref().unwrap_or("—")
    );
    println!(
        "  PHI        {} days → harvest allowed from {}",
        r.phi_days_used, r.phi_end_date
    );
    for p in &t.plots {
        let plot_name: String =
            conn.query_row("SELECT name FROM plot WHERE id = ?1", [&p.plot_id], |r| {
                r.get(0)
            })?;
        println!(
            "  plot       {:<10} {:>4} ha · {} {}",
            plot_name,
            p.surface_treated_ha,
            p.crop_name_snapshot.as_deref().unwrap_or("—"),
            p.variety_snapshot.as_deref().unwrap_or(""),
        );
    }
    Ok(())
}
