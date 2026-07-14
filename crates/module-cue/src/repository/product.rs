// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Product catalogue: products, active substances and per-country authorisations.
//!
//! Products are soft-deleted (treatment history keeps resolving them). Their
//! substance links and authorisation rows are detail rows of the catalogue —
//! removing one is a hard delete logged with a null after-image, the same
//! contract as the regional extension rows. Past treatment records are immune
//! either way: they snapshot name, authorisation number and substances at
//! write time.

use super::audit::{log_delete, log_insert, log_update};
use crate::date::now_utc_iso;
use crate::error::{CueError, Result};
use crate::models::{
    ActiveSubstance, NewProduct, NewProductAuthorisation, Product, ProductActiveSubstance,
    ProductAuthorisation, ProductAuthorisationFields, ProductDetail, ProductSubstance,
    UpdateProduct,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use uuid::Uuid;

/// Insert an active substance. Synced user data (UUIDv7 PK since 2026-07-02),
/// so the full row image is logged in `record_change` like any other insert.
pub fn insert_active_substance(
    conn: &mut Connection,
    name: &str,
    cas_number: Option<&str>,
) -> Result<ActiveSubstance> {
    if name.trim().is_empty() {
        return Err(CueError::Invalid("empty_name"));
    }
    let tx = conn.transaction()?;
    let substance = ActiveSubstance {
        id: Uuid::now_v7().to_string(),
        name: name.to_string(),
        cas_number: cas_number.map(str::to_string),
    };
    tx.execute(
        "INSERT INTO active_substance (id, name, cas_number) VALUES (?1, ?2, ?3)",
        params![substance.id, substance.name, substance.cas_number],
    )?;
    log_insert(&tx, "active_substance", &substance.id, None, &substance)?;
    tx.commit()?;
    Ok(substance)
}

/// Every registered substance, for the product form's substance picker.
pub fn list_active_substances(conn: &Connection) -> Result<Vec<ActiveSubstance>> {
    let mut stmt =
        conn.prepare("SELECT id, name, cas_number FROM active_substance ORDER BY name, id")?;
    let substances = stmt
        .query_map([], |r| {
            Ok(ActiveSubstance {
                id: r.get(0)?,
                name: r.get(1)?,
                cas_number: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(substances)
}

pub fn insert_product(conn: &mut Connection, new: NewProduct) -> Result<Product> {
    let tx = conn.transaction()?;
    let product = insert_product_tx(&tx, new)?;
    tx.commit()?;
    Ok(product)
}

/// Create a product together with its first per-country authorisation, in one
/// transaction — the registry form's happy path. A product with no
/// authorisation is never offered to the treatment form, so the two rows
/// belong together: if either insert fails, neither lands.
pub fn insert_product_with_authorisation(
    conn: &mut Connection,
    new: NewProduct,
    authorisation: ProductAuthorisationFields,
) -> Result<ProductDetail> {
    let tx = conn.transaction()?;
    let product = insert_product_tx(&tx, new)?;
    let authorisation = insert_authorisation_tx(&tx, &product.id, authorisation)?;
    tx.commit()?;
    Ok(ProductDetail {
        product,
        substances: Vec::new(),
        authorisations: vec![authorisation],
    })
}

fn insert_product_tx(tx: &Transaction, new: NewProduct) -> Result<Product> {
    if new.commercial_name.trim().is_empty() {
        return Err(CueError::Invalid("empty_name"));
    }
    let now = now_utc_iso();
    let product = Product {
        id: Uuid::now_v7().to_string(),
        commercial_name: new.commercial_name,
        holder: new.holder,
        formulation_type_code: new.formulation_type_code,
        default_phi_days: new.default_phi_days,
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    };
    tx.execute(
        "INSERT INTO product
           (id, commercial_name, holder, formulation_type_code, default_phi_days, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            product.id, product.commercial_name, product.holder, product.formulation_type_code,
            product.default_phi_days, product.created_at, product.updated_at
        ],
    )?;
    log_insert(tx, "product", &product.id, None, &product)?;
    Ok(product)
}

/// Full-row update; the submitted state replaces the stored one. Past records
/// are safe: they carry `phi_days_used` and the `*_snapshot` columns.
pub fn update_product(conn: &mut Connection, id: &str, update: UpdateProduct) -> Result<Product> {
    if update.commercial_name.trim().is_empty() {
        return Err(CueError::Invalid("empty_name"));
    }
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM product WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_product,
        )
        .optional()?
        .ok_or(CueError::NotFound)?;

    let mut after = before.clone();
    after.commercial_name = update.commercial_name;
    after.holder = update.holder;
    after.formulation_type_code = update.formulation_type_code;
    after.default_phi_days = update.default_phi_days;
    after.updated_at = now_utc_iso();

    tx.execute(
        "UPDATE product SET commercial_name = ?2, holder = ?3, formulation_type_code = ?4,
                            default_phi_days = ?5, updated_at = ?6
         WHERE id = ?1",
        params![
            id,
            after.commercial_name,
            after.holder,
            after.formulation_type_code,
            after.default_phi_days,
            after.updated_at
        ],
    )?;
    log_update(&tx, "product", id, None, &before, &after)?;
    tx.commit()?;
    Ok(after)
}

/// Soft delete: the product leaves the registry and the treatment form's
/// dropdown, but the row stays so treatment history keeps resolving. Its
/// substance links and authorisations stay with it.
pub fn soft_delete_product(conn: &mut Connection, id: &str) -> Result<()> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM product WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            map_product,
        )
        .optional()?
        .ok_or(CueError::NotFound)?;
    let now = now_utc_iso();
    let mut after = before.clone();
    after.deleted_at = Some(now.clone());
    after.updated_at = now.clone();
    tx.execute(
        "UPDATE product SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    log_delete(&tx, "product", id, None, &before, Some(&after))?;
    tx.commit()?;
    Ok(())
}

/// Attach an active substance + concentration to a product (junction row).
/// The row has its own UUID and is logged under that id, so a future
/// update/delete of one substance link is addressable in `record_change`.
pub fn add_product_active_substance(
    conn: &mut Connection,
    product_id: &str,
    active_substance_id: &str,
    concentration_value: Option<f64>,
    concentration_unit_code: Option<&str>,
) -> Result<ProductActiveSubstance> {
    let tx = conn.transaction()?;
    let link = ProductActiveSubstance {
        id: Uuid::now_v7().to_string(),
        product_id: product_id.to_string(),
        active_substance_id: active_substance_id.to_string(),
        concentration_value,
        concentration_unit_code: concentration_unit_code.map(str::to_string),
    };
    tx.execute(
        "INSERT INTO product_active_substance
           (id, product_id, active_substance_id, concentration_value, concentration_unit_code)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            link.id,
            link.product_id,
            link.active_substance_id,
            link.concentration_value,
            link.concentration_unit_code
        ],
    )?;
    log_insert(&tx, "product_active_substance", &link.id, None, &link)?;
    tx.commit()?;
    Ok(link)
}

/// Detach a substance from a product. Hard delete (catalogue detail row, like
/// the regional extensions) logged with a null after-image; past treatment
/// records keep their `active_substances_snapshot`.
pub fn remove_product_active_substance(conn: &mut Connection, link_id: &str) -> Result<()> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM product_active_substance WHERE id = ?1",
            [link_id],
            map_substance_link,
        )
        .optional()?
        .ok_or(CueError::NotFound)?;
    tx.execute(
        "DELETE FROM product_active_substance WHERE id = ?1",
        [link_id],
    )?;
    log_delete(
        &tx,
        "product_active_substance",
        link_id,
        None,
        &before,
        None,
    )?;
    tx.commit()?;
    Ok(())
}

/// Active products holding an authorisation in one country — the treatment
/// form's product dropdown. `insert_treatment_record` rejects unauthorised
/// products (`AuthorisationMissing`), so others are not offered at all.
pub fn list_products_authorised(conn: &Connection, country_code: &str) -> Result<Vec<Product>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT product.* FROM product
         JOIN product_authorisation ON product_authorisation.product_id = product.id
         WHERE product_authorisation.country_code = ?1 AND product.deleted_at IS NULL
         ORDER BY product.commercial_name, product.id",
    )?;
    let products = stmt
        .query_map([country_code], map_product)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(products)
}

/// Every active product with its substances and authorisations — the registry
/// list (country-agnostic, unlike `list_products_authorised`).
pub fn list_product_details(conn: &Connection) -> Result<Vec<ProductDetail>> {
    let mut stmt = conn
        .prepare("SELECT * FROM product WHERE deleted_at IS NULL ORDER BY commercial_name, id")?;
    let products = stmt
        .query_map([], map_product)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    products
        .into_iter()
        .map(|product| {
            let substances = product_substances(conn, &product.id)?;
            let authorisations = product_authorisations(conn, &product.id)?;
            Ok(ProductDetail {
                product,
                substances,
                authorisations,
            })
        })
        .collect()
}

fn product_substances(conn: &Connection, product_id: &str) -> Result<Vec<ProductSubstance>> {
    let mut stmt = conn.prepare(
        "SELECT link.id, link.active_substance_id, substance.name, substance.cas_number,
                link.concentration_value, link.concentration_unit_code
         FROM product_active_substance AS link
         JOIN active_substance AS substance ON substance.id = link.active_substance_id
         WHERE link.product_id = ?1
         ORDER BY substance.name, link.id",
    )?;
    let substances = stmt
        .query_map([product_id], |r| {
            Ok(ProductSubstance {
                id: r.get(0)?,
                active_substance_id: r.get(1)?,
                name: r.get(2)?,
                cas_number: r.get(3)?,
                concentration_value: r.get(4)?,
                concentration_unit_code: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(substances)
}

fn product_authorisations(
    conn: &Connection,
    product_id: &str,
) -> Result<Vec<ProductAuthorisation>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM product_authorisation WHERE product_id = ?1 ORDER BY country_code, id",
    )?;
    let authorisations = stmt
        .query_map([product_id], map_authorisation)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(authorisations)
}

/// Register a per-country authorisation number for a product (multi-country case).
pub fn add_product_authorisation(
    conn: &mut Connection,
    new: NewProductAuthorisation,
) -> Result<ProductAuthorisation> {
    let tx = conn.transaction()?;
    let auth = insert_authorisation_tx(
        &tx,
        &new.product_id,
        ProductAuthorisationFields {
            country_code: new.country_code,
            authorisation_number: new.authorisation_number,
            status: new.status,
            valid_from: new.valid_from,
            valid_until: new.valid_until,
        },
    )?;
    tx.commit()?;
    Ok(auth)
}

/// Remove a per-country authorisation. Hard delete logged with a null
/// after-image; past records keep `authorisation_number_snapshot`. The product
/// simply stops being offered in that country's treatment form.
pub fn remove_product_authorisation(conn: &mut Connection, authorisation_id: &str) -> Result<()> {
    let tx = conn.transaction()?;
    let before = tx
        .query_row(
            "SELECT * FROM product_authorisation WHERE id = ?1",
            [authorisation_id],
            map_authorisation,
        )
        .optional()?
        .ok_or(CueError::NotFound)?;
    tx.execute(
        "DELETE FROM product_authorisation WHERE id = ?1",
        [authorisation_id],
    )?;
    log_delete(
        &tx,
        "product_authorisation",
        authorisation_id,
        None,
        &before,
        None,
    )?;
    tx.commit()?;
    Ok(())
}

fn insert_authorisation_tx(
    tx: &Transaction,
    product_id: &str,
    fields: ProductAuthorisationFields,
) -> Result<ProductAuthorisation> {
    if fields.authorisation_number.trim().is_empty() {
        return Err(CueError::Invalid("empty_authorisation_number"));
    }
    let auth = ProductAuthorisation {
        id: Uuid::now_v7().to_string(),
        product_id: product_id.to_string(),
        country_code: fields.country_code,
        authorisation_number: fields.authorisation_number,
        status: fields.status,
        valid_from: fields.valid_from,
        valid_until: fields.valid_until,
    };
    tx.execute(
        "INSERT INTO product_authorisation
           (id, product_id, country_code, authorisation_number, status, valid_from, valid_until)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            auth.id,
            auth.product_id,
            auth.country_code,
            auth.authorisation_number,
            auth.status,
            auth.valid_from,
            auth.valid_until
        ],
    )?;
    log_insert(tx, "product_authorisation", &auth.id, None, &auth)?;
    Ok(auth)
}

fn map_product(row: &Row) -> rusqlite::Result<Product> {
    Ok(Product {
        id: row.get("id")?,
        commercial_name: row.get("commercial_name")?,
        holder: row.get("holder")?,
        formulation_type_code: row.get("formulation_type_code")?,
        default_phi_days: row.get("default_phi_days")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

fn map_substance_link(row: &Row) -> rusqlite::Result<ProductActiveSubstance> {
    Ok(ProductActiveSubstance {
        id: row.get("id")?,
        product_id: row.get("product_id")?,
        active_substance_id: row.get("active_substance_id")?,
        concentration_value: row.get("concentration_value")?,
        concentration_unit_code: row.get("concentration_unit_code")?,
    })
}

fn map_authorisation(row: &Row) -> rusqlite::Result<ProductAuthorisation> {
    Ok(ProductAuthorisation {
        id: row.get("id")?,
        product_id: row.get("product_id")?,
        country_code: row.get("country_code")?,
        authorisation_number: row.get("authorisation_number")?,
        status: row.get("status")?,
        valid_from: row.get("valid_from")?,
        valid_until: row.get("valid_until")?,
    })
}
