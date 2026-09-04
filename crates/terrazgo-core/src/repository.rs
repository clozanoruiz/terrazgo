// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
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

mod advisor;
mod country;
mod crop;
mod export_alias;
mod farm;
mod geo_feature;
mod harvest;
mod machinery;
mod operator;
mod premises;
mod season;
mod sowing;
mod user_profile;
mod water_point;
mod zone_flag;

pub use advisor::{
    insert_advisor, list_advisors, list_farm_advisors, remove_farm_advisor, set_farm_advisor,
    soft_delete_advisor, update_advisor,
};
pub use country::{
    list_countries, list_fertiliser_dose_units, list_gip_systems, list_growing_environments,
    list_intensity_units, list_irrigation_systems, list_irrigation_volume_units,
    list_licence_levels, list_premises_kinds, list_production_systems, list_quantity_units,
    list_sowing_kinds, list_units,
};
pub use crop::{
    crops_on_plot, find_crop_for_export, insert_crop, list_crops, soft_delete_crop, update_crop,
};
pub use export_alias::{ensure_export_alias, find_export_alias};
pub use farm::{
    get_farm, insert_farm, insert_plot, list_farms, list_plots, soft_delete_farm, soft_delete_plot,
    update_farm, update_plot,
};
pub use geo_feature::{list_geo_features_for_farm, save_geo_feature, soft_delete_geo_feature};
pub use harvest::{
    get_harvest_record, insert_harvest_record, list_harvest_records,
    list_harvest_records_for_export, soft_delete_harvest_record, update_harvest_record,
};
pub use machinery::{
    find_machinery_es, insert_machinery, list_machinery, list_machinery_details,
    soft_delete_machinery, update_machinery,
};
pub use operator::{insert_operator, list_operators, soft_delete_operator, update_operator};
pub use premises::{
    get_premises, get_premises_detail, insert_premises, list_premises, list_premises_details,
    soft_delete_premises, update_premises,
};
pub use season::{insert_season, list_seasons, soft_delete_season, update_season};
pub use sowing::{
    get_sowing_record, insert_sowing_record, list_sowing_records, list_sowing_records_for_export,
    soft_delete_sowing_record, update_sowing_record,
};
pub use user_profile::{
    insert_user_profile, list_user_profiles, soft_delete_user_profile, update_user_profile,
};
pub use water_point::{
    clear_water_declaration, insert_water_point, list_water_declarations, list_water_points,
    set_water_declaration, soft_delete_water_point, update_water_point,
};
pub use zone_flag::{list_latest_zone_flags, list_zone_flags_for_farm, replace_zone_flags};

/// Names of user-entered rows (farm, plot, season label, crop species, …) must
/// not be blank — they are what the selectors and the printed cuaderno show.
fn validate_name(name: &str) -> crate::error::Result<()> {
    if name.trim().is_empty() {
        return Err(crate::error::CoreError::Invalid("empty_name"));
    }
    Ok(())
}
