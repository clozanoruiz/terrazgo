// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Fetching reference-catalogue refreshes from the provider.
//!
//! The app ships a vendored snapshot of the FEGA SIEX catalogues and imports
//! it at startup, so codes resolve from the very first run with no network
//! (`terrazgo_core::catalogue`). That snapshot only moves when a release moves
//! it — which is why this exists: a user whose provider published a new code
//! last month can ask for it without waiting for one.
//!
//! The network half lives HERE, in the shell, and not in core: core having no
//! HTTP crate anywhere in its dependency tree is the build-enforced form of
//! the offline-first rule. This module fetches bytes and hands them over;
//! every rule about whether those bytes may be adopted is core's, and runs
//! before anything is written.
//!
//! **Manual only.** No timer, no fetch at startup. Reference data underpins
//! records with legal value, and rewriting it behind the farmer's back — on a
//! metered rural connection, at that — is the wrong default.

use terrazgo_core::catalogue::RefreshReport;

/// The provider's per-catalogue endpoint: `GET {base}{idTabla}` answers with
/// that catalogue's CSV, public and unauthenticated (docs/maintenance.md §1).
///
/// Deliberately NOT `/catalogos/zip/`: that bundle names its files by display
/// name ("Eficacia del tratamiento.csv") rather than by idTabla, so nothing
/// mechanical can match a member to the spec that reads it.
const CATALOGUE_BASE_URL: &str = "https://www11.fega.es/bdcsixwsp/catalogos/";

/// Fetch one catalogue's CSV. `Err` carries a report line rather than an
/// error, because a fetch that fails is a per-file refusal like any other —
/// one unreachable file must not stop the other 46.
pub fn fetch_catalogue(id: &str) -> Result<Vec<u8>, RefreshReport> {
    let url = format!("{CATALOGUE_BASE_URL}{id}");
    match terrazgo_net::http_get(&url, "text/csv") {
        Ok(fetched) => Ok(fetched.data),
        // A status is the server answering — a retired idTabla 404s, and that
        // is a different thing to tell the user than "no network".
        Err(terrazgo_net::NetError::Http { status }) => {
            Err(RefreshReport::refused(id, "http", status.to_string()))
        }
        Err(terrazgo_net::NetError::Offline(reason)) => {
            Err(RefreshReport::refused(id, "network", reason))
        }
    }
}
