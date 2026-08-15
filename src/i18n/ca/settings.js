// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Diccionari català, per àrees. Les claus són idèntiques en tots els idiomes
// i cap pot repetir-se entre fitxers: i18n.js els fusiona.

export default {
  "settings.general": "General",
  "settings.map": "Mapa",
  "settings.cache_size": "Espai màxim per als mapes sense connexió",
  "settings.cache_default": "Predeterminat ({size})",
  "settings.cache_hint":
    "Els mapes consultats es desen per poder-los usar sense connexió; en superar el límit s'eliminen primer els menys usats.",
  "settings.clear_cache": "Esborra els mapes desats",
  "settings.clear_cache_confirm":
    "Voleu esborrar els mapes desats? Es tornaran a baixar quan hi hagi connexió. Les dades de l'explotació no es toquen.",
  "settings.catalogues": "Catàlegs de referència",
  "catalogues.hint":
    "Llistes oficials de codis (FEGA) amb què l'aplicació resol productes, problemes, materials i la resta. Vénen dins de l'aplicació i s'actualitzen amb cada versió; aquí podeu demanar-ne la darrera publicada.",
  "catalogues.state": "{count} catàlegs · {codes} codis",
  "catalogues.updated_at": "darrera actualització: {date}",
  "catalogues.never": "encara sense importar",
  "catalogues.refresh": "Actualitza els catàlegs",
  "catalogues.refreshing": "S'està consultant el servei…",
  "catalogues.updated": "nous: {added} · corregits: {corrected}.",
  "catalogues.extra_columns": "columnes noves que aquesta versió no fa servir: {columns}.",
  "catalogues.unchanged": "Sense canvis: {count}.",
  "catalogues.refused.shape":
    "el fitxer ja no té la forma que aquesta versió sap llegir; caldrà una actualització de l'aplicació",
  "catalogues.refused.empty": "el fitxer ha arribat sense dades",
  "catalogues.refused.label": "una fila ha arribat sense descripció",
  "catalogues.refused.control_characters": "el fitxer ha arribat amb caràcters il·legibles",
  "catalogues.refused.shrunk":
    "el fitxer porta menys files de les ja desades; sembla una baixada incompleta",
  "catalogues.refused.network": "no s'ha pogut connectar amb el servei",
  "catalogues.refused.http": "el servei ha respost amb un error",
  "settings.profiles": "Perfils d'usuari",
  "backup.title": "Còpia de seguretat",
  "backup.import_confirm":
    "Importar una còpia de seguretat SUBSTITUEIX totes les dades actuals pel contingut de la còpia. Abans es desa una còpia de la base de dades actual. Voleu continuar?",
};
