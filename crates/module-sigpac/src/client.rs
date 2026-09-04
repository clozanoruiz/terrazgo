// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Recinto lookups against the Nube de SIGPAC consultas service, riding on
//! terrazgo-geo's cache-through fetch: a response seen once is served from
//! `geo-cache.db` forever after, so a verified plot stays verifiable offline.
//! `refresh` bypasses the cache read (user-triggered re-verification, e.g. at
//! campaign rollover) while still storing the new payload.

use crate::models::{
    DeclaredCampaign, DeclaredCrop, RecintoInfo, ZoneIntersection, parse_declared_crops_response,
    parse_intersection_response, parse_recinto_response,
};
use crate::reference::SigpacRef;
use terrazgo_core::date::today_utc;
use terrazgo_core::db::Database;
use terrazgo_geo::{GeoError, Result, fetch};

/// The consultas base. Only this Rust code builds SIGPAC URLs — the webview
/// never sees the host (it talks `geo://`; production CSP stays closed).
const BASE_URL: &str = "https://sigpac-hubcloud.es/servicioconsultassigpac/query";
const INTERSECTION_URL: &str = "https://sigpac-hubcloud.es/servicioconsultassigpac/intersection";
/// The Nube de SIGPAC OGC API Features endpoint. The consultas service has no
/// declared-crops operation among its eleven, so this is the only per-reference
/// channel for the PAC declaration (CC BY 4.0, no auth).
const OGCAPI_URL: &str = "https://sigpac-hubcloud.es/ogcapi";

/// The current SIGPAC campaign year. Moved into terrazgo-geo (2026-07-11)
/// because campaign-keyed tile caching (the MVT recinto overlay) needs it
/// below the module tier; re-exported so this crate's callers keep one entry
/// point for everything SIGPAC.
pub use terrazgo_geo::fetch::current_campaign;

/// The zone layers Terrazgo checks, as (zone_type code, service layer name).
/// Order is the storage/display order.
pub const ZONE_LAYERS: &[(&str, &str)] = &[
    ("nitrate_vulnerable", "nitratos"),
    ("phytosanitary_restriction", "fitosanitarios"),
    ("natura_2000", "red_natura"),
];

/// Cache key for a by-reference lookup in geo-cache.db's `resource` table.
/// Public so tests (and future cache maintenance) address the same row the
/// client writes.
pub fn recinfo_cache_key(reference: &SigpacRef) -> String {
    format!("sigpac/recinfo/{}", reference.to_path())
}

/// Look one recinto up by its 7-part reference. `Ok(None)` means SIGPAC does
/// not know the reference — the caller's "typo or outdated ref" signal.
pub fn recinto_by_reference(
    cache: &Database,
    reference: &SigpacRef,
    refresh: bool,
) -> Result<Option<RecintoInfo>> {
    let url = format!("{BASE_URL}/recinfo/{}.geojson", reference.to_path());
    let fetched = fetch::cached_resource(
        cache,
        &recinfo_cache_key(reference),
        &url,
        "application/json",
        refresh,
    )?;
    parse_recinto_response(&fetched.data)
}

/// One zone-layer intersection for a recinto. `Ok(None)` = outside the layer
/// (the service answers `[]`). `layer` is the service name from [`ZONE_LAYERS`].
pub fn zone_intersection(
    cache: &Database,
    reference: &SigpacRef,
    layer: &str,
    refresh: bool,
) -> Result<Option<ZoneIntersection>> {
    let key = format!("sigpac/intersection/{layer}/{}", reference.to_path());
    let url = format!("{INTERSECTION_URL}/{layer}/{}.json", reference.to_path());
    let fetched = fetch::cached_resource(cache, &key, &url, "application/json", refresh)?;
    parse_intersection_response(&fetched.data)
}

/// Cache key for one campaign's declaration lines of one recinto. The campaign
/// is part of the key on purpose: a rollover writes new rows instead of
/// overwriting old ones, so last year's answer stays available as the fallback
/// and a stale entry can never mask a new campaign.
pub fn declared_crops_cache_key(campaign: i64, reference: &SigpacRef) -> String {
    format!("sigpac/cultivos/{campaign}/{}", reference.to_path())
}

/// The PAC declaration lines for one recinto in one campaign.
///
/// OGC API Features: the seven reference parts and `exp_ano` are queryables the
/// service accepts as plain query parameters (live-probed 2026-08-02). An empty
/// vector means nothing was declared — the service answers HTTP 200 with
/// `numberMatched: 0`, never a 404.
pub fn declared_crops_by_reference(
    cache: &Database,
    reference: &SigpacRef,
    campaign: i64,
    refresh: bool,
) -> Result<Vec<DeclaredCrop>> {
    let fetched = fetch::cached_resource(
        cache,
        &declared_crops_cache_key(campaign, reference),
        &declared_crops_url(reference, campaign),
        "application/json",
        refresh,
    )?;
    parse_declared_crops_response(&fetched.data)
}

/// The declaration for a recinto, from whichever campaign actually has one.
///
/// The service runs one campaign behind: today it answers `exp_ano=2025` while
/// the current campaign is 2026, because a campaign's declarations are only
/// loaded months into it. So the current campaign is tried first and the
/// previous one is the fallback, and the campaign that answered rides back with
/// the lines — a proposal must always be able to say which year it speaks for.
///
/// The two campaigns are trusted differently, and that asymmetry is the point:
///
/// * For the **current** campaign a cached empty answer is trusted only for
///   the rest of the UTC day it was fetched on. It may predate the day FEGA
///   loaded the data, and a permanently cached "nothing declared" would hide
///   the declaration for the rest of the year — but the staleness that matters
///   is measured in months, so re-asking more than once a day buys nothing and
///   costs a request per plot every time the farmer opens the panel. The
///   explicit refresh bypasses the day entirely.
/// * For the **previous** campaign the cache is authoritative, empties
///   included: that dataset is closed and will not grow.
///
/// `Ok(None)` means both campaigns were genuinely consulted and neither had a
/// declaration. If neither could be reached, the network error propagates
/// instead — reporting "nothing declared" when we could not ask would be a
/// claim we have no grounds for.
pub fn declared_crops_with_fallback(
    cache: &Database,
    reference: &SigpacRef,
    current: i64,
    refresh: bool,
) -> Result<Option<DeclaredCampaign>> {
    let stored = match fetch::cached(cache, &declared_crops_cache_key(current, reference))? {
        Some(hit) => {
            let lines = parse_declared_crops_response(&hit.data)?;
            if lines.is_empty() {
                Stored::Empty {
                    asked_today: asked_today(&hit.fetched_at, &today_utc()),
                }
            } else {
                Stored::Declared(lines)
            }
        }
        None => Stored::Missing,
    };
    let current_outcome = match stored {
        // A stored declaration is final: it exists, and it serves offline like
        // any other lookup.
        Stored::Declared(lines) if !refresh => Consulted::Declared(lines),
        // Today's "nothing declared" is trusted for the rest of the day.
        Stored::Empty { asked_today: true } if !refresh => Consulted::Empty,
        // Nothing stored, an empty from an earlier day, or an explicit
        // refresh: ask upstream, falling back to what was stored if the
        // network is gone.
        stored => match declared_crops_by_reference(cache, reference, current, true) {
            Ok(lines) => Consulted::from_lines(lines),
            Err(error) => match stored {
                Stored::Declared(lines) => Consulted::Declared(lines),
                // Today's empty stands even if the forced re-ask failed.
                Stored::Empty { asked_today: true } => Consulted::Empty,
                // An older empty is exactly what we cannot conclude from.
                Stored::Empty { asked_today: false } | Stored::Missing => {
                    Consulted::Unreachable(error)
                }
            },
        },
    };
    if let Consulted::Declared(lines) = current_outcome {
        return Ok(Some(DeclaredCampaign {
            campaign: current,
            lines,
        }));
    }

    let previous_outcome = match declared_crops_by_reference(cache, reference, current - 1, refresh)
    {
        Ok(lines) => Consulted::from_lines(lines),
        Err(error) => Consulted::Unreachable(error),
    };
    resolve_campaigns(current, current_outcome, previous_outcome)
}

/// What the cache already held for the current campaign.
enum Stored {
    Missing,
    Declared(Vec<DeclaredCrop>),
    Empty { asked_today: bool },
}

/// Whether a cached answer was fetched on the day `today` names.
///
/// Day granularity, matching the tile cache's once-per-UTC-day touch: a
/// campaign is loaded on some *day*, so the day is the natural unit for "ask
/// again". Both are ISO 8601 UTC, so comparing the date prefix is the whole
/// job; anything unparseable counts as old, which errs toward asking again.
fn asked_today(fetched_at: &str, today: &str) -> bool {
    fetched_at.len() >= 10 && today.len() == 10 && &fetched_at[..10] == today
}

/// What asking one campaign produced.
enum Consulted {
    Declared(Vec<DeclaredCrop>),
    /// Consulted, and it really has no declaration for this recinto.
    Empty,
    Unreachable(GeoError),
}

impl Consulted {
    fn from_lines(lines: Vec<DeclaredCrop>) -> Self {
        if lines.is_empty() {
            Consulted::Empty
        } else {
            Consulted::Declared(lines)
        }
    }
}

/// Decide what two consulted campaigns amount to. Split out from the fetching
/// so the rule can be tested without a network: `Ok(None)` — "SIGPAC has no
/// declaration for this recinto" — may only be said when both campaigns
/// actually answered. If either could not be reached and neither declared
/// anything, the failure is what the caller gets, because silence from an
/// unreachable service is not evidence of an empty declaration.
fn resolve_campaigns(
    current: i64,
    current_outcome: Consulted,
    previous_outcome: Consulted,
) -> Result<Option<DeclaredCampaign>> {
    match (current_outcome, previous_outcome) {
        (Consulted::Declared(lines), _) => Ok(Some(DeclaredCampaign {
            campaign: current,
            lines,
        })),
        (_, Consulted::Declared(lines)) => Ok(Some(DeclaredCampaign {
            campaign: current - 1,
            lines,
        })),
        (Consulted::Empty, Consulted::Empty) => Ok(None),
        (Consulted::Unreachable(error), _) | (_, Consulted::Unreachable(error)) => Err(error),
    }
}

fn declared_crops_url(reference: &SigpacRef, campaign: i64) -> String {
    format!(
        "{OGCAPI_URL}/collections/cultivo_declarado/items?f=json\
         &provincia={}&municipio={}&agregado={}&zona={}&poligono={}&parcela={}&recinto={}\
         &exp_ano={campaign}",
        reference.province,
        reference.municipality,
        reference.aggregate,
        reference.zone,
        reference.polygon,
        reference.parcel,
        reference.enclosure,
    )
}

/// Look up the recinto under a geographic point (map click today, GPS
/// position later). Coordinates are cached verbatim — a repeated click on
/// the same stored feature works offline; arbitrary new points need network.
pub fn recinto_by_point(cache: &Database, lon: f64, lat: f64) -> Result<Option<RecintoInfo>> {
    let key = format!("sigpac/recinfobypoint/{lon}/{lat}");
    let url = format!("{BASE_URL}/recinfobypoint/4326/{lon}/{lat}.geojson");
    let fetched = fetch::cached_resource(cache, &key, &url, "application/json", false)?;
    parse_recinto_response(&fetched.data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Map;

    fn line() -> DeclaredCrop {
        DeclaredCrop {
            properties: Map::new(),
        }
    }

    fn offline() -> GeoError {
        GeoError::Offline("no route to host".into())
    }

    /// The current campaign wins whenever it has an answer, and otherwise the
    /// previous one does — labelled with the year that actually answered.
    #[test]
    fn a_declaring_campaign_answers_and_names_itself() {
        let answer = resolve_campaigns(2026, Consulted::Declared(vec![line()]), Consulted::Empty)
            .unwrap()
            .unwrap();
        assert_eq!(answer.campaign, 2026);

        let answer = resolve_campaigns(2026, Consulted::Empty, Consulted::Declared(vec![line()]))
            .unwrap()
            .unwrap();
        assert_eq!(answer.campaign, 2025);

        // A declaration outweighs a failure on the other campaign: the answer
        // exists, so there is nothing to report a problem about.
        let answer = resolve_campaigns(
            2026,
            Consulted::Unreachable(offline()),
            Consulted::Declared(vec![line()]),
        )
        .unwrap()
        .unwrap();
        assert_eq!(answer.campaign, 2025);
    }

    /// The current campaign's "nothing declared" is re-asked once a day, not
    /// once a click: a campaign is loaded on some DAY, so anything finer just
    /// costs a request per plot every time the panel opens.
    #[test]
    fn an_empty_answer_is_trusted_for_the_rest_of_its_day() {
        assert!(asked_today("2026-08-04T09:15:00Z", "2026-08-04"));
        // Same instant, other end of the day — still today.
        assert!(asked_today("2026-08-04T23:59:59Z", "2026-08-04"));
        // Yesterday's empty is asked again; FEGA may have loaded the campaign
        // overnight.
        assert!(!asked_today("2026-08-03T23:59:59Z", "2026-08-04"));
        assert!(!asked_today("2025-08-04T09:15:00Z", "2026-08-04"));
        // Anything unreadable errs toward asking again rather than trusting.
        assert!(!asked_today("", "2026-08-04"));
        assert!(!asked_today("not-a-date", "2026-08-04"));
        assert!(!asked_today("2026-08-04T09:15:00Z", ""));
    }

    /// "SIGPAC has no crops declared for this plot" is a statement about the
    /// service's data, so it may only be made when the service answered.
    #[test]
    fn nothing_declared_requires_both_campaigns_to_have_answered() {
        assert!(
            resolve_campaigns(2026, Consulted::Empty, Consulted::Empty)
                .unwrap()
                .is_none()
        );

        for (current, previous) in [
            (Consulted::Unreachable(offline()), Consulted::Empty),
            (Consulted::Empty, Consulted::Unreachable(offline())),
            (
                Consulted::Unreachable(offline()),
                Consulted::Unreachable(offline()),
            ),
        ] {
            assert!(
                matches!(
                    resolve_campaigns(2026, current, previous),
                    Err(GeoError::Offline(_))
                ),
                "an unreachable campaign must surface as a failure, not as an empty declaration"
            );
        }
    }
}
