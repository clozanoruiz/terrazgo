// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// English dictionary, by area. Keys are identical across every locale and none
// may repeat between files: i18n.js merges them.
//
// Area "external": what the app points at outside itself — the official
// registries that hold each identifier, and the project's own pages.
//
// Registry proper nouns (SIGPAC, ROMA, REGANIP, ROPO, ITEAF, MAPA) are not
// translated in any locale: they are names, not descriptions. The sentence
// around them is what translates.

export default {
  // --- official registries (ids match src-tauri/src/external_links.rs)
  "registry.sigpac_hint":
    "The SIGPAC reference is in FEGA's official viewer: search by municipality, or find the parcel on the map.",
  "registry.sigpac_open": "Open the SIGPAC viewer",

  "registry.roma_hint":
    "The ROMA number is on the equipment's entry in the Registro Oficial de Maquinaria Agrícola.",
  "registry.roma_open": "Search ROMA",

  "registry.reganip_hint":
    "REGANIP registers aircraft and fixed or semi-mobile installations; mobile equipment is registered in ROMA.",
  "registry.reganip_open": "Search REGANIP",

  "registry.regiteaf_hint": "Periodic inspections are carried out at an authorised ITEAF station.",
  "registry.regiteaf_open": "Find an ITEAF station",

  "registry.ropo_hint":
    "The inscription number is in the Registro Oficial de Productores y Operadores (ROPO), for applicators and advisors alike.",
  "registry.ropo_open": "Search ROPO",

  "registry.regfiweb_hint":
    "The registration number is on the product label and in MAPA's register of plant protection products.",
  "registry.regfiweb_open": "Search the plant-protection register",

  "registry.catastro_hint":
    "The cadastral reference is on the IBI bill and in the Sede Electrónica del Catastro.",
  "registry.catastro_open": "Search the Catastro",

  // --- about
  "about.tab_app": "App",
  "about.tab_technical": "Technical details",
  "about.tab_libraries": "Libraries",
  "about.blurb":
    "Farm management software: maps, the field record book, fertilisation and irrigation, in a program that works offline and keeps your data on your own machine.",
  "about.libraries_hint":
    "Terrazgo is built on these third-party libraries, grouped by licence. Each section holds the licence text exactly as its authors publish it.",
  "about.copyright": "Copyright © {year} Carlos Lozano Ruiz",
  "about.notice_grant":
    "This program is free software: you can redistribute it and/or modify it under the terms of the GNU Affero General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.",
  "about.notice_warranty":
    "This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more details.",
  "about.notice_copy":
    "You should have received a copy of the GNU Affero General Public License along with this program. The official text is the link below.",
  "about.licence_canonical": "Official text",
  "about.licence_or": "or",
  "about.title": "About Terrazgo",
  "about.version": "Version {version}",
  "about.webview": "Web engine",
  "about.system": "System",
  "about.arch": "Architecture",
  "about.user_agent": "Browser identifier",
  "about.link_homepage": "Project website",
  "about.link_source": "Source code",
  "about.link_issues": "Report a problem",
  "about.link_licence": "Licence text",
  "about.link_privacy": "Privacy",
};
