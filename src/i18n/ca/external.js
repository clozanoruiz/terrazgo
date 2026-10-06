// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Diccionari català, per àrees. Les claus són idèntiques en tots els idiomes i
// cap no es pot repetir entre fitxers: i18n.js els fusiona.
//
// Àrea "external": allò que l'app assenyala fora d'ella mateixa — els registres
// oficials que guarden cada identificador i les pàgines del projecte.
//
// Els noms propis dels registres (SIGPAC, ROMA, REGANIP, ROPO, ITEAF, MAPA) no
// es tradueixen en cap idioma: són noms, no descripcions. El que sí que es
// tradueix és la frase que els envolta.

export default {
  // --- registres oficials (els ids són els de src-tauri/src/external_links.rs)
  "registry.sigpac_hint":
    "La referència SIGPAC es troba al visor oficial del FEGA: es pot cercar per municipi o directament sobre el mapa.",
  "registry.sigpac_open": "Obrir el visor SIGPAC",

  "registry.roma_hint":
    "El número ROMA figura a la inscripció de l'equip al Registro Oficial de Maquinaria Agrícola.",
  "registry.roma_open": "Consultar el ROMA",

  "registry.reganip_hint":
    "El REGANIP inscriu aeronaus i instal·lacions fixes o semimòbils; els equips mòbils s'inscriuen al ROMA.",
  "registry.reganip_open": "Consultar el REGANIP",

  "registry.regiteaf_hint": "La inspecció periòdica es fa en una estació ITEAF autoritzada.",
  "registry.regiteaf_open": "Cercar una estació ITEAF",

  "registry.ropo_hint":
    "El número d'inscripció figura al Registro Oficial de Productores y Operadores (ROPO), tant per a aplicadors com per a assessors.",
  "registry.ropo_open": "Consultar el ROPO",

  "registry.regfiweb_hint":
    "El número de registre figura a l'etiqueta del producte i al registre de productes fitosanitaris del MAPA.",
  "registry.regfiweb_open": "Consultar el registre de fitosanitaris",

  "registry.catastro_hint":
    "La referència cadastral figura al rebut de l'IBI i a la Seu Electrònica del Cadastre.",
  "registry.catastro_open": "Consultar el Cadastre",

  // --- quant a
  "about.tab_app": "Aplicació",
  "about.tab_technical": "Detalls tècnics",
  "about.tab_libraries": "Biblioteques",
  "about.blurb":
    "Aplicació de gestió de l'explotació agrícola: mapes, quadern de camp, fertilització i reg, en un programa que funciona sense connexió i desa les vostres dades al vostre propi equip.",
  "about.libraries_hint":
    "Terrazgo es basa en aquestes biblioteques de tercers, agrupades per llicència. Quan una biblioteca permet triar entre diverses llicències, figura sota la que fa servir Terrazgo. Cada apartat mostra el text de la llicència tal com el publiquen els seus autors.",
  "about.copyright": "Copyright © {year} Carlos Lozano Ruiz",
  "about.notice_grant":
    "Aquest programa és programari lliure: podeu redistribuir-lo i/o modificar-lo sota els termes de la GNU Affero General Public License publicada per la Free Software Foundation, ja sigui la versió 3 de la llicència o (si ho preferiu) qualsevol versió posterior.",
  "about.notice_warranty":
    "Aquest programa es distribueix amb l'esperança que sigui útil, però SENSE CAP GARANTIA; ni tan sols la garantia implícita de COMERCIABILITAT o APTITUD PER A UN PROPÒSIT DETERMINAT. Consulteu la GNU Affero General Public License per a més detalls.",
  "about.notice_copy":
    "Hauríeu d'haver rebut una còpia de la GNU Affero General Public License juntament amb aquest programa. El text oficial, que és l'únic vinculant, és en anglès i el podeu consultar a l'enllaç de sota.",
  "about.licence_canonical": "Text oficial",
  "about.licence_or": "o",
  "about.title": "Quant a Terrazgo",
  "about.version": "Versió {version}",
  "about.webview": "Motor web",
  "about.system": "Sistema",
  "about.arch": "Arquitectura",
  "about.user_agent": "Identificador del navegador",
  // Els quatre blocs de la pestanya tècnica.
  "about.group_build": "Compilació",
  "about.group_machine": "Equip",
  "about.group_data": "Dades",
  "about.group_regional": "Configuració regional",
  "about.identifier": "Identificador",
  // Noms propis de formats d'empaquetatge: iguals en els tres idiomes.
  "about.packaging.development": "compilació de desenvolupament",
  "about.packaging.app_image": "AppImage",
  "about.packaging.flatpak": "Flatpak",
  "about.packaging.snap": "Snap",
  "about.packaging.apk": "APK",
  "about.packaging.installed": "instal·lat",
  "about.packaging.portable": "portable",
  "about.cpu": "Processador",
  "about.cpu_cores_threads": "{cores} nuclis / {threads} fils",
  "about.cpu_threads": "{threads} fils",
  "about.memory": "Memòria",
  "about.memory_available": "{available} lliures de {total}",
  "about.process_memory": "Memòria del procés",
  "about.memory_now": "{size} ara",
  "about.memory_peak": "màxim {size}",
  "about.display": "Pantalla",
  "about.display_at": "a ({x}, {y})",
  "about.display_scale": "escala {scale}",
  "about.display_rate": "~{fps} fps",
  "about.database": "Base de dades",
  "about.geo_cache": "Memòria cau de mapes",
  "about.schema": "esquema {version}",
  "about.system_locale": "Idioma del sistema",
  "about.language": "Idioma de l'aplicació",
  "about.formats": "Formats",
  "about.formats_system": "segons el sistema",
  "about.formats_language": "segons l'idioma",
  "about.link_homepage": "Pàgina del projecte",
  "about.link_source": "Codi font",
  "about.link_issues": "Informar d'un error",
  "about.link_licence": "Text de la llicència",
  "about.link_privacy": "Privacitat",
};
