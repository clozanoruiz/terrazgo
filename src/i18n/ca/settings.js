// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Diccionari català, per àrees. Les claus són idèntiques en tots els idiomes
// i cap pot repetir-se entre fitxers: i18n.js els fusiona.

export default {
  // L'estructura de la pantalla de configuració: el cercador i l'índex del
  // costat. Els noms de les seccions i dels grups són a lib/settingsTree.js.
  "settings.search": "Cerca",
  "settings.toc": "Seccions de la configuració",
  "settings.clear_search": "Esborra la cerca",
  "settings.collapse_section": "Redueix {section}",
  "settings.expand_section": "Amplia {section}",
  "settings.results.one": "{count} opció",
  "settings.results.other": "{count} opcions",
  "settings.no_results": "Cap opció coincideix amb la cerca.",
  "settings.general": "General",
  "settings.group.language": "Idioma i format",
  "settings.map": "Mapa",
  "settings.group.offline_maps": "Mapes sense connexió",
  "settings.group.treated_plots": "Parcel·les tractades",
  "settings.data": "Dades",
  "settings.advanced": "Avançat",
  "settings.cache_size": "Espai màxim per als mapes sense connexió",
  "settings.cache_default": "Predeterminat ({size})",
  "settings.cache_hint":
    "Els mapes consultats es desen per poder-los usar sense connexió; en superar el límit s'eliminen primer els menys usats.",
  "settings.clear_cache": "Esborra els mapes desats",
  "settings.clear_cache_confirm":
    "Voleu esborrar els mapes desats? Es tornaran a baixar quan hi hagi connexió. Les dades de l'explotació no es toquen.",
  "settings.phi_horizon": "Mostra les parcel·les tractades fins a (dies enrere)",
  "settings.phi_horizon_hint":
    "Durant quant de temps el mapa continua assenyalant una parcel·la ja tractada el termini de seguretat de la qual ha acabat. En blanc: {days} dies. No afecta el termini de seguretat ni les parcel·les amb el termini en curs, que es mostren sempre.",
  "settings.alerts": "Avisos",
  "settings.alerts_hint":
    "Amb quanta antelació voleu que us avisem abans que caduqui un carnet d'aplicador o que venci la ITV d'una màquina. En blanc: {licence} i {itv} dies. No és un termini legal: trieu el temps que necessiteu per renovar-ho, que depèn de la disponibilitat de cursos i d'estacions d'ITV a la vostra zona.",
  "settings.licence_lead": "Avís de caducitat del carnet (dies)",
  "settings.itv_lead": "Avís de venciment de la ITV (dies)",
  "settings.catalogues": "Catàlegs de referència",
  "catalogues.hint":
    "Llistes oficials de codis (FEGA) amb què l'aplicació resol productes, problemes, materials i la resta. Vénen dins de l'aplicació; aquí podeu demanar-ne la darrera publicada, i es conserva fins que una nova versió de l'aplicació hi porti la seva.",
  // Dos recomptes que concorden amb noms diferents: forma "Etiqueta: N", que
  // és correcta amb qualsevol xifra (docs/frontend-conventions.md).
  "catalogues.state": "Catàlegs: {count} · Codis: {codes}",
  "catalogues.updated_at": "darrera actualització: {date}",
  "catalogues.never": "encara sense importar",
  "catalogues.refresh": "Actualitza els catàlegs",
  "catalogues.refreshing": "S'està consultant el servei…",
  "catalogues.updated": "nous: {added} · corregits: {corrected}.",
  "catalogues.withdrawn.one":
    "Un ja no figura a la llista de l'organisme: deixa d'oferir-se, però els registres que ja el citen se segueixen resolent.",
  "catalogues.withdrawn.other":
    "{count} ja no figuren a la llista de l'organisme: deixen d'oferir-se, però els registres que ja els citen se segueixen resolent.",
  "catalogues.extra_columns.one": "columna nova que aquesta versió no fa servir: {columns}.",
  "catalogues.extra_columns.other": "columnes noves que aquesta versió no fa servir: {columns}.",
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
  "settings.maintenance": "Manteniment de la base de dades",
  "settings.maintenance_hint":
    "Revisa a fons el fitxer de dades per detectar-hi danys i, si està en bon estat, el compacta per recuperar espai. L'aplicació ja fa una revisió ràpida cada setmana pel seu compte; aquesta és més completa i pot trigar uns segons.",
  "settings.check_db": "Revisa i compacta",
  "backup.title": "Còpia de seguretat",
  "backup.import_confirm":
    "Importar una còpia de seguretat SUBSTITUEIX totes les dades actuals pel contingut de la còpia. Abans es desa una còpia de la base de dades actual. Voleu continuar?",
};
