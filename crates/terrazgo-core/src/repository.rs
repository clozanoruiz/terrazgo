// SPDX-License-Identifier: AGPL-3.0-or-later

//! Repository for the core-owned entities (farm, plot), one submodule per entity
//! group, public functions re-exported here.
//!
//! Same invariant as the module repositories: every write to a synced user-data
//! table also appends a COMPLETE row image to `record_change`, inside the same
//! transaction (audit trail + future sync delta source).
//!
//! Writes take `&mut Connection` because `conn.transaction()` needs a mutable
//! borrow; reads take `&Connection`.

mod country;
mod crop;
mod farm;
mod geo_feature;
mod machinery;
mod operator;
mod season;
mod zone_flag;

pub use country::{list_countries, list_licence_levels, list_production_systems};
pub use crop::{insert_crop, list_crops};
pub use farm::{
    get_farm, insert_farm, insert_plot, list_farms, list_plots, soft_delete_farm, soft_delete_plot,
    update_farm, update_plot,
};
pub use geo_feature::{list_geo_features_for_farm, save_geo_feature, soft_delete_geo_feature};
pub use machinery::{
    insert_machinery, list_machinery, list_machinery_details, soft_delete_machinery,
    update_machinery,
};
pub use operator::{insert_operator, list_operators, soft_delete_operator, update_operator};
pub use season::{insert_season, list_seasons};
pub use zone_flag::{list_zone_flags_for_farm, replace_zone_flags};

/// Names of user-entered rows (farm, plot, season label, crop species, …) must
/// not be blank — they are what the selectors and the printed cuaderno show.
fn validate_name(name: &str) -> crate::error::Result<()> {
    if name.trim().is_empty() {
        return Err(crate::error::CoreError::Invalid("empty_name"));
    }
    Ok(())
}
