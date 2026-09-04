// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// The settings screen's own structure, as data: sections, the groups inside
// them, and the individual settings inside those. Rendered twice — once as the
// table of contents beside the screen (SettingsToc.svelte) and once as the
// screen itself (SettingsView.svelte) — which is the nav.js arrangement and
// exists for the same reason: a contents list that can disagree with what it
// lists is worse than none.
//
// Framework-agnostic tier: no Svelte imports (docs/frontend-conventions.md).
// It names i18n KEYS and never resolved text, the nav.js/icons.js split again —
// the caller supplies the resolver, which is also what makes the matcher below
// testable without loading a dictionary.
//
// Order is general first, specific last, and it is the reading order of the
// screen as well as of the tree.
//
// A NODE'S SEARCHABLE TEXT IS DECLARED, NOT SCRAPED. The alternative — reading
// the rendered DOM — would count the words of whatever happened to be on
// screen, so a setting scrolled behind a closed panel would stop being
// findable. Declaring it means the hit counts are a property of the settings
// themselves; it also means a key listed here that no longer exists would
// silently search its own name, which is what settingsTree.test.js pins.
import { fold, foldTokens, matchRank } from "./collate.js";

export const SETTINGS_TREE = [
  {
    id: "general",
    labelKey: "settings.general",
    groups: [
      {
        id: "general.language",
        labelKey: "settings.group.language",
        items: [
          { id: "language", keys: ["lang.label"] },
          { id: "format", keys: ["format.label", "format.hint"] },
        ],
      },
      {
        // The lead times are a preference about being told, not about the map
        // or the database, so they sit with the other general preferences —
        // the alerts they govern stay on the Status view.
        id: "general.alerts",
        labelKey: "settings.alerts",
        hintKey: "settings.alerts_hint",
        items: [
          { id: "licence_lead", keys: ["settings.licence_lead"] },
          { id: "itv_lead", keys: ["settings.itv_lead"] },
        ],
      },
    ],
  },
  {
    id: "map",
    labelKey: "settings.map",
    groups: [
      {
        id: "map.offline",
        labelKey: "settings.group.offline_maps",
        items: [
          { id: "cache_size", keys: ["settings.cache_size", "settings.cache_hint"] },
          { id: "clear_cache", keys: ["settings.clear_cache"] },
        ],
      },
      {
        id: "map.treated",
        labelKey: "settings.group.treated_plots",
        items: [{ id: "phi_horizon", keys: ["settings.phi_horizon", "settings.phi_horizon_hint"] }],
      },
    ],
  },
  {
    id: "data",
    labelKey: "settings.data",
    groups: [
      {
        // One item, because the panel is one thing to find: a farmer searching
        // for "perfil" wants the profiles panel, not a hit per column of the
        // table inside it. The rows in it are farm data rather than settings,
        // and indexing them would answer a search for a person's name with a
        // settings hit.
        id: "data.profiles",
        labelKey: "settings.profiles",
        hintKey: "profiles.hint",
        items: [{ id: "profiles", keys: ["profiles.active_label", "profiles.new"] }],
      },
      {
        id: "data.catalogues",
        labelKey: "settings.catalogues",
        hintKey: "catalogues.hint",
        items: [{ id: "catalogues", keys: ["catalogues.refresh", "catalogues.state"] }],
      },
    ],
  },
  {
    id: "advanced",
    labelKey: "settings.advanced",
    groups: [
      {
        id: "advanced.backup",
        labelKey: "backup.title",
        items: [
          { id: "backup_export", keys: ["actions.export_backup"] },
          { id: "backup_import", keys: ["actions.import_backup"] },
        ],
      },
      {
        id: "advanced.maintenance",
        labelKey: "settings.maintenance",
        hintKey: "settings.maintenance_hint",
        items: [{ id: "check_db", keys: ["settings.check_db"] }],
      },
      {
        id: "advanced.about",
        labelKey: "about.title",
        items: [{ id: "about", keys: ["about.title", "about.version", "about.copyright"] }],
      },
    ],
  },
];

/// The DOM id of a section or group heading.
///
/// One definition because three callers need to agree on it: the view writes
/// it, the contents list scrolls to it, and the scroll spy reads it back.
/// Dots become dashes only to keep the ids readable in devtools — nothing here
/// puts them through a CSS selector, where a dot would have to be escaped.
export function settingsAnchor(id) {
  return `set-${id.replace(/\./g, "-")}`;
}

/// Every anchor id in the order the screen renders them.
///
/// The scroll spy walks this and stops at the first heading below the reading
/// line, which is only correct while the list is in document order — so it is
/// derived from the tree rather than written out a second time.
export function settingsAnchors() {
  return SETTINGS_TREE.flatMap((section) => [section.id, ...section.groups.map((g) => g.id)]);
}

function anyMatch(keys, tokens, textOf) {
  return keys.some((key) => key && matchRank(fold(textOf(key)), tokens) >= 0);
}

/// Which settings a query leaves visible, and how many hits each node holds.
///
/// `textOf(key)` resolves an i18n key to display text — injected rather than
/// imported so this stays in the agnostic tier and so a test can supply its own
/// corpus.
///
/// Returns `{ filtering, hits, total }`, where `hits` maps EVERY node id — a
/// section, a group and an item alike — to a count. An item is 1 or 0, a group
/// is how many of its items survived, a section is the sum of its groups. One
/// map rather than three, so the view has a single predicate (`hits[id] > 0`)
/// for "render this" and the tree has a single number to print.
///
/// A container matching by its OWN text keeps all of its contents: searching
/// for "avisos" should hand back the alerts group intact rather than the empty
/// shell of a group whose heading matched and whose fields did not.
export function searchSettings(query, textOf) {
  const tokens = foldTokens(query);
  const hits = {};
  let total = 0;

  for (const section of SETTINGS_TREE) {
    // No query means everything matches, which makes the empty case fall out of
    // the same code path rather than needing one of its own.
    const sectionHit = tokens.length === 0 || anyMatch([section.labelKey], tokens, textOf);
    let sectionCount = 0;

    for (const group of section.groups) {
      const groupHit = sectionHit || anyMatch([group.labelKey, group.hintKey], tokens, textOf);
      let groupCount = 0;

      for (const item of group.items) {
        const visible = groupHit || anyMatch(item.keys, tokens, textOf);
        hits[item.id] = visible ? 1 : 0;
        if (visible) groupCount += 1;
      }

      hits[group.id] = groupCount;
      sectionCount += groupCount;
    }

    hits[section.id] = sectionCount;
    total += sectionCount;
  }

  return { filtering: tokens.length > 0, hits, total };
}
