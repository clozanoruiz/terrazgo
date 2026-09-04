// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Diccionario español, por áreas. Las claves son idénticas en todos los
// idiomas y ninguna puede repetirse entre archivos: i18n.js los fusiona.
//
// Área "external": lo que la app señala fuera de sí misma — los registros
// oficiales que guardan cada identificador y las páginas del proyecto.
//
// Los nombres propios de los registros (SIGPAC, ROMA, REGANIP, ROPO, ITEAF,
// MAPA) no se traducen en ningún idioma: son nombres, no descripciones. Lo
// que sí se traduce es la frase que los rodea.

export default {
  // --- registros oficiales (los ids son los de src-tauri/src/external_links.rs)
  "registry.sigpac_hint":
    "La referencia SIGPAC figura en el visor oficial del FEGA: se busca por municipio o directamente sobre el mapa.",
  "registry.sigpac_open": "Abrir el visor SIGPAC",

  "registry.roma_hint":
    "El número ROMA figura en la inscripción del equipo en el Registro Oficial de Maquinaria Agrícola.",
  "registry.roma_open": "Consultar el ROMA",

  "registry.reganip_hint":
    "El REGANIP inscribe aeronaves e instalaciones fijas o semimóviles; los equipos móviles se inscriben en el ROMA.",
  "registry.reganip_open": "Consultar el REGANIP",

  "registry.regiteaf_hint": "La inspección periódica se realiza en una estación ITEAF autorizada.",
  "registry.regiteaf_open": "Buscar una estación ITEAF",

  "registry.ropo_hint":
    "El número de inscripción figura en el Registro Oficial de Productores y Operadores (ROPO), tanto para aplicadores como para asesores.",
  "registry.ropo_open": "Consultar el ROPO",

  "registry.regfiweb_hint":
    "El número de registro figura en la etiqueta del producto y en el registro de productos fitosanitarios del MAPA.",
  "registry.regfiweb_open": "Consultar el registro de fitosanitarios",

  "registry.catastro_hint":
    "La referencia catastral figura en el recibo del IBI y en la Sede Electrónica del Catastro.",
  "registry.catastro_open": "Consultar el Catastro",

  // --- acerca de
  "about.tab_app": "Aplicación",
  "about.tab_technical": "Detalles técnicos",
  "about.tab_libraries": "Bibliotecas",
  "about.blurb":
    "Aplicación de gestión de la explotación agrícola: mapas, cuaderno de campo, fertilización y riego, en un programa que funciona sin conexión y guarda sus datos en su propio equipo.",
  "about.libraries_hint":
    "Terrazgo se apoya en estas bibliotecas de terceros, agrupadas por licencia. Cada apartado muestra el texto de la licencia tal como lo publican sus autores.",
  "about.copyright": "Copyright © {year} Carlos Lozano Ruiz",
  "about.notice_grant":
    "Este programa es software libre: puede redistribuirlo y/o modificarlo bajo los términos de la GNU Affero General Public License publicada por la Free Software Foundation, ya sea la versión 3 de la licencia o (a su elección) cualquier versión posterior.",
  "about.notice_warranty":
    "Este programa se distribuye con la esperanza de que sea útil, pero SIN NINGUNA GARANTÍA; ni siquiera la garantía implícita de COMERCIABILIDAD o APTITUD PARA UN PROPÓSITO DETERMINADO. Consulte la GNU Affero General Public License para más detalles.",
  "about.notice_copy":
    "Debería haber recibido una copia de la GNU Affero General Public License junto con este programa. El texto oficial, que es el único vinculante, está en inglés y puede consultarlo en el enlace de abajo.",
  "about.licence_canonical": "Texto oficial",
  "about.licence_or": "o",
  "about.title": "Acerca de Terrazgo",
  "about.version": "Versión {version}",
  "about.webview": "Motor web",
  "about.system": "Sistema",
  "about.arch": "Arquitectura",
  "about.user_agent": "Identificador del navegador",
  "about.link_homepage": "Página del proyecto",
  "about.link_source": "Código fuente",
  "about.link_issues": "Informar de un fallo",
  "about.link_licence": "Texto de la licencia",
  "about.link_privacy": "Privacidad",
};
