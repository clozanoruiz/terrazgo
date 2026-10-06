// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Build an app data directory holding a book two devices wrote at once, so
//! the conflict review can be looked at in the running app.
//!
//! Development tooling, like `terrazgo-recordbook`'s `render_demo`: it is not
//! part of the app and nothing ships it. What it produces is a real database
//! in a real app data directory — point the binary at it with `XDG_DATA_HOME`
//! and the Status view opens on the queue.
//!
//! **Nothing here is simulated except the second device's keyboard.** Both
//! devices are real databases opened the way the shell opens one, every edit
//! goes through the repositories, and the changes cross by the real transport:
//! a `.tzsync` bundle written by one and applied by the other. So what the
//! screen shows is what two phones would have produced.
//!
//! Three registers end up waiting, and **the third is deliberately the other
//! way round**: the laptop's version is the one the book shows. Which side goes
//! live is decided by the clock and not by which device you are sitting at, so
//! a demo where the local device always loses teaches the wrong lesson — and
//! the pair side by side is the point (docs/sync.md → Hybrid logical clocks
//! decide which side goes live).
//!
//!     cargo run -p terrazgo --example seed_sync_conflicts -- <data-dir>
//!
//! where `<data-dir>` is `$XDG_DATA_HOME/org.terrazgo.app`.

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use rusqlite::Connection;
use serde_json::json;
use terrazgo_core::models::UpdatePlot;
use terrazgo_core::repository as core_repo;
use terrazgo_core::sync::VersionVector;

/// The register the two devices disagree about, read off the demo book.
struct Treatment {
    id: String,
    application_date: String,
    product_id: Option<String>,
    dose_unit_code: Option<String>,
    operator_id: String,
    plot_id: String,
    crop_id: Option<String>,
    surface_treated_ha: f64,
    problems: Vec<(String, String)>,
    justifications: Vec<String>,
}

fn main() -> Result<()> {
    let data_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("usage: seed_sync_conflicts <app-data-dir>"))?;
    std::fs::create_dir_all(&data_dir)?;
    let laptop_path = data_dir.join("terrazgo.db");
    let phone_path = data_dir.join("phone-replica.db");
    for path in [&laptop_path, &phone_path] {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }

    let laptop_id = terrazgo_core::sync::mint_device_id();
    let phone_id = terrazgo_core::sync::mint_device_id();

    // The device the app will run as. Written before the database, the way
    // startup does it: a connection cannot log a change without an identity.
    let settings = terrazgo_core::settings::AppSettings {
        device_id: Some(laptop_id.clone()),
        ..Default::default()
    };
    terrazgo_core::settings::save_settings(&data_dir.join("settings.json"), &settings)
        .map_err(|err| anyhow!("{err}"))?;

    let mut laptop = terrazgo_lib::db::open_app_db(&laptop_path, &laptop_id)?;
    module_phytosanitary::demo::seed_demo(&mut laptop).context("seeding the demo book")?;
    core_repo::rename_sync_peer(
        &mut laptop,
        &laptop_id,
        Some("Portátil de la oficina"),
        None,
    )?;

    // The second device joins the holding and takes everything, which is the
    // first sync two devices ever do.
    let mut phone = terrazgo_lib::db::open_app_db(&phone_path, &phone_id)?;
    carry(&laptop, &laptop_id, &mut phone, true)?;
    core_repo::rename_sync_peer(&mut phone, &phone_id, Some("Móvil de María"), None)?;

    let treatment = read_treatment(&laptop)?;
    let other_plot = second_plot(&laptop, &treatment.plot_id)?;

    // Now they part. Neither has seen the other's edit, which is the whole of
    // what makes this a conflict rather than a correction.
    correct_treatment(&mut laptop, &treatment, 0.8, Some("2ª pasada"), &[])?;
    correct_treatment(&mut phone, &treatment, 1.5, None, &[other_plot.as_str()])?;

    let plot = &treatment.plot_id;
    rename_plot(&mut laptop, plot, "El Soto bajo")?;
    rename_plot(&mut phone, plot, "El Soto")?;

    // The third one in the other order, so the laptop holds the live version of
    // something. The clock decides, so the device that writes LAST wins — and
    // the pause is what makes that a fact rather than a coin toss: both writes
    // would otherwise land in the same millisecond, where the tie-break is the
    // device id and the arrangement would depend on which UUID sorted higher.
    let crop = crop_row(&laptop)?;
    let crop_id = crop["id"]
        .as_str()
        .ok_or_else(|| anyhow!("the crop row has no id"))?
        .to_owned();
    record_variety(&mut phone, &crop, &crop_id, "Marcial", 3.2)?;
    std::thread::sleep(std::time::Duration::from_millis(5));
    record_variety(&mut laptop, &crop, &crop_id, "Rimbaud", 3.5)?;

    // And the phone hands its file to the laptop. The merge runs here, exactly
    // as it does behind Settings → Importar cambios.
    let summary = carry(&phone, &phone_id, &mut laptop, false)?;
    // The phone's own name reached the laptop with its rows; the laptop learns
    // nothing else about it.
    let waiting =
        core_repo::list_sync_conflicts(&laptop, &terrazgo_lib::registry::composed_row_captions())?;

    println!("data dir      {}", data_dir.display());
    println!("laptop        {laptop_id}  (Portátil de la oficina — the app runs as this one)");
    println!("phone         {phone_id}  (Móvil de María)");
    println!(
        "applied       {} change sets, {} rows",
        summary.change_sets_applied, summary.rows
    );
    println!("waiting       {} registers:", waiting.len());
    for entry in &waiting {
        let live = entry
            .devices
            .iter()
            .find(|device| device.live)
            .and_then(|device| device.label.clone())
            .unwrap_or_else(|| "?".into());
        println!(
            "              {} · {}   (el cuaderno muestra: {live})",
            entry.root_table,
            entry.caption.as_deref().unwrap_or("—")
        );
    }
    Ok(())
}

/// Write everything `to` has not got as a bundle, and apply it — the real
/// transport, in memory instead of through a USB stick.
fn carry(
    from: &Connection,
    from_device: &str,
    to: &mut Connection,
    join: bool,
) -> Result<terrazgo_core::bundle::ImportSummary> {
    let mut bytes = Vec::new();
    terrazgo_core::bundle::write_bundle(from, from_device, &VersionVector::default(), &mut bytes)?;
    let bundle = terrazgo_core::bundle::read_bundle(&bytes[..])?;
    if join {
        terrazgo_core::sync::join_sync_group(to, &bundle.manifest.group)?;
    }
    Ok(terrazgo_core::bundle::apply_bundle(
        to,
        &bundle,
        terrazgo_core::date::now_ms(),
    )?)
}

/// The demo's first chemical treatment, with the children a correction has to
/// restate — the form submits the whole register, so a partial update would
/// delete what it left out.
fn read_treatment(conn: &Connection) -> Result<Treatment> {
    let (id, application_date, product_id, dose_unit_code, operator_id) = conn.query_row(
        "SELECT id, application_date, product_id, dose_unit_code, operator_id
         FROM treatment_record
         WHERE deleted_at IS NULL AND product_id IS NOT NULL
         ORDER BY application_date
         LIMIT 1",
        [],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
            ))
        },
    )?;
    let (plot_id, crop_id, surface_treated_ha) = conn.query_row(
        "SELECT plot_id, crop_id, surface_treated_ha FROM treatment_plot
         WHERE treatment_record_id = ?1 ORDER BY id LIMIT 1",
        [&id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    let problems = conn
        .prepare(
            "SELECT reason_category_code, problem_code FROM treatment_problem
             WHERE treatment_record_id = ?1",
        )?
        .query_map([&id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let justifications = conn
        .prepare(
            "SELECT justification_code FROM treatment_justification
             WHERE treatment_record_id = ?1",
        )?
        .query_map([&id], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Treatment {
        id,
        application_date,
        product_id,
        dose_unit_code,
        operator_id,
        plot_id,
        crop_id,
        surface_treated_ha,
        problems,
        justifications,
    })
}

/// The demo's first crop, as a JSON object keyed by its own column names.
///
/// Read whole and handed back whole because that is how a form submits a
/// register: `UpdateCrop` ignores the columns it does not carry, so the fields
/// nobody is changing keep the values they had.
fn crop_row(conn: &Connection) -> Result<serde_json::Map<String, serde_json::Value>> {
    let mut stmt = conn
        .prepare("SELECT * FROM crop WHERE deleted_at IS NULL ORDER BY species_name, id LIMIT 1")?;
    let columns: Vec<String> = stmt.column_names().into_iter().map(str::to_owned).collect();
    let row = stmt.query_row([], |row| {
        let mut object = serde_json::Map::new();
        for (index, column) in columns.iter().enumerate() {
            let value = match row.get_ref(index)? {
                rusqlite::types::ValueRef::Null => serde_json::Value::Null,
                rusqlite::types::ValueRef::Integer(number) => json!(number),
                rusqlite::types::ValueRef::Real(number) => json!(number),
                rusqlite::types::ValueRef::Text(text) => json!(String::from_utf8_lossy(text)),
                rusqlite::types::ValueRef::Blob(_) => serde_json::Value::Null,
            };
            object.insert(column.clone(), value);
        }
        Ok(object)
    })?;
    Ok(row)
}

/// One device's reading of what is growing on the plot: a variety and a
/// surface, on a crop both of them hold.
fn record_variety(
    conn: &mut Connection,
    crop: &serde_json::Map<String, serde_json::Value>,
    crop_id: &str,
    variety: &str,
    area_ha: f64,
) -> Result<()> {
    let mut submitted = crop.clone();
    submitted.insert("variety".into(), json!(variety));
    submitted.insert("area_ha".into(), json!(area_ha));
    let update = serde_json::from_value(serde_json::Value::Object(submitted))?;
    core_repo::update_crop(conn, crop_id, update, None)?;
    Ok(())
}

/// Another plot of the same holding, for the device that adds one.
fn second_plot(conn: &Connection, besides: &str) -> Result<String> {
    Ok(conn.query_row(
        "SELECT id FROM plot WHERE deleted_at IS NULL AND id <> ?1 ORDER BY name LIMIT 1",
        [besides],
        |row| row.get(0),
    )?)
}

/// One device's correction: a dose, a note, and whichever plots it says were
/// treated. Built as JSON because the form's update type is deserialized —
/// this is the same shape the webview sends.
fn correct_treatment(
    conn: &mut Connection,
    treatment: &Treatment,
    dose: f64,
    notes: Option<&str>,
    extra_plots: &[&str],
) -> Result<()> {
    let mut plots = vec![json!({
        "plot_id": treatment.plot_id,
        "crop_id": treatment.crop_id,
        "surface_treated_ha": treatment.surface_treated_ha,
    })];
    for plot_id in extra_plots {
        plots.push(json!({
            "plot_id": plot_id,
            "crop_id": null,
            "surface_treated_ha": 1.25,
        }));
    }
    let update = serde_json::from_value(json!({
        "application_date": treatment.application_date,
        "product_id": treatment.product_id,
        "dose_value": dose,
        "dose_unit_code": treatment.dose_unit_code,
        "operator_id": treatment.operator_id,
        "notes": notes,
        "problems": treatment.problems.iter().map(|(category, code)| json!({
            "reason_category_code": category,
            "problem_code": code,
        })).collect::<Vec<_>>(),
        "justifications": treatment.justifications,
        "plots": plots,
    }))?;
    module_phytosanitary::repository::update_treatment_record(conn, &treatment.id, update, None)?;
    Ok(())
}

fn rename_plot(conn: &mut Connection, plot_id: &str, name: &str) -> Result<()> {
    let area = conn.query_row("SELECT area_ha FROM plot WHERE id = ?1", [plot_id], |row| {
        row.get(0)
    })?;
    core_repo::update_plot(
        conn,
        plot_id,
        UpdatePlot {
            name: name.to_owned(),
            area_ha: area,
            es: None,
        },
        None,
    )?;
    Ok(())
}
