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
  "sync.title": "Sincronitza amb un altre dispositiu",
  "sync.hint":
    "Exporteu els canvis d'aquest dispositiu a un fitxer, copieu-lo a l'altre i importeu-lo allà. Un fitxer en cada sentit posa els dos al dia. No surt res dels vostres dispositius: el fitxer va on vosaltres el porteu.",
  "sync.import_confirm.one":
    "Aquest fitxer ve de {device} i es va crear el {created}. Porta un canvi. Voleu aplicar-lo a aquest quadern?",
  "sync.import_confirm.other":
    "Aquest fitxer ve de {device} i es va crear el {created}. Porta {count} canvis. Voleu aplicar-los a aquest quadern?",
  "sync.pair_confirm":
    "Aquest dispositiu encara no està aparellat amb {device}. En aparellar-los compartiran quadern: els canvis d'un arribaran a l'altre cada vegada que sincronitzeu. Els voleu aparellar?",
  "sync.join_other_confirm":
    "COMPTE: aquest fitxer ve d'un altre grup de dispositius. Si continueu, aquest dispositiu deixarà el seu grup actual i passarà al de {device}. Feu-ho només si tots dos són vostres i porten el mateix quadern.",
  "sync.peers_title": "Dispositius d'aquesta explotació",
  "sync.peers_hint":
    "Cada dispositiu que escriu en aquest quadern hi apareix. Poseu-los nom: així, quan dos hagin escrit el mateix registre alhora, la pantalla dirà «Mòbil de la Maria» en lloc d'un codi.",
  "sync.peers_empty": "Ara com ara només ha escrit en aquest quadern aquest dispositiu.",
  "sync.peer_label": "Nom del dispositiu",
  "sync.device_this": "aquest dispositiu",
  "sync.device_unnamed": "un dispositiu sense nom",
  "sync.peer_unnamed": "Sense nom",
  "sync.peer_this_device": "Aquest dispositiu",
  "sync.peer_known_since": "Conegut des de",
  "sync.peer_retired": "Retirat",
  "sync.peer_retire": "Retira",
  "sync.peer_restore": "Reactiva",
  "sync.peer_retire_confirm":
    "Voleu retirar «{name}»? El que va escriure continua al quadern i continua al seu nom; això només deixa d'esperar que torni a sincronitzar. El podeu reactivar quan vulgueu.",
  "conflicts.title": "Registres escrits alhora",
  "conflicts.hint":
    "Dos dispositius van escriure aquests registres sense haver-se vist. El quadern mostra una de les versions a tots els dispositius; trieu quina es queda.",
  "conflicts.review": "Revisa",
  "conflicts.written_on": "Escrit a {devices}",
  "conflicts.in_book": "Campanya {season} de {farm}",
  "conflicts.version_live": "És la versió que mostra el quadern",
  "conflicts.version_waiting": "Versió en espera",
  "conflicts.written_by": "Escrit per {person}",
  "conflicts.written_at": "Escrit el {date}",
  "conflicts.keep": "Conserva aquesta",
  "conflicts.keep_confirm":
    "Voleu conservar la versió de {device}? El quadern la mostrarà a tots els dispositius. L'altra no es perd: la base de dades desa totes les versions del registre.",
  "conflicts.kept": "Fet. El quadern mostra la versió de {device}.",
  "conflicts.identical":
    "Les dues versions diuen exactament el mateix. Conserveu-ne qualsevol per tancar la diferència.",
  "conflicts.edit_hint":
    "Si la resposta correcta no és cap de les dues, corregiu el registre a la seva pantalla: qualsevol canvi tanca la diferència a tots els dispositius.",
  "conflicts.yes": "Sí",
  "conflicts.no": "No",
  "conflicts.absent": "—",
  "duplicates.title": "Possibles duplicats",
  "duplicates.hint":
    "Aquests registres s'assemblen tant que poden ser una mateixa operació anotada dues vegades. Reviseu cada parella: conserveu-ne un dels dos, o indiqueu que són dos registres diferents.",
  "duplicates.review": "Revisa",
  "duplicates.written_by": "Anotats per {people}",
  "duplicates.in_book": "Campanya {season} de {farm}",
  "duplicates.record": "Registre {n}",
  "duplicates.on_device": "A {device}",
  "duplicates.by_person": "Per {person}",
  "duplicates.written_at": "Anotat el {date}",
  "duplicates.differs_hint": "El que està destacat és el que no coincideix entre els dos.",
  "duplicates.keep": "Conserva el registre {n}",
  "duplicates.keep_confirm":
    "Voleu conservar el registre {n} i eliminar l'altre? L'eliminat deixarà d'aparèixer al quadern, i queda anotat per què es va eliminar.",
  "duplicates.kept": "Fet. S'ha conservat un registre i s'ha eliminat l'altre.",
  "duplicates.both_real": "Són dos registres diferents",
  "duplicates.judged_distinct":
    "Anotat: són dos registres diferents. No se us tornarà a preguntar per ells.",
  "duplicates.both_removed": "Ja no és al quadern",
  "duplicates.both_removed_hint":
    "Aquesta operació ja no és al quadern. Estava anotada dues vegades, i cada dispositiu va conservar una còpia diferent i va eliminar l'altra abans de sincronitzar-se: en ajuntar-se, van quedar eliminades totes dues. Recupereu-ne una perquè torni a constar una vegada.",
  "duplicates.restore": "Recupera el registre {n}",
  "duplicates.restored": "Fet. El registre torna a ser al quadern.",
  "duplicates.book_notice.one": "Un possible duplicat en aquesta campanya.",
  "duplicates.book_notice.other": "{count} possibles duplicats en aquesta campanya.",
  "duplicates.book_title": "Possibles duplicats d'aquesta campanya",
  "duplicates.removed_each": "Una còpia la va eliminar {first} i l'altra, {second}",
  "duplicates.remover": "{person} a {device}",
  "duplicates.removed_by": "Eliminat per {person} a {device} el {date}",
  "duplicates.removed_on_device": "Eliminat a {device} el {date}",
  "duplicates.identical": "Els dos registres diuen exactament el mateix: tant és quin conserveu.",
  "duplicates.identical_removed":
    "Les dues còpies diuen exactament el mateix: tant és quina recupereu.",
  "duplicates.back": "Torna a la llista",
  "duplicates.just_saved": "El que acabeu de desar",
  "duplicates.saved_hint":
    "El que acabeu de desar s'assembla molt a un registre que ja hi era. Si és una mateixa operació anotada dues vegades, conserveu-ne un; si en són dues, indiqueu-ho. També podeu tancar i decidir-ho després: quedarà a la llista de possibles duplicats.",
  "strays.title": "Registres en un quadern suprimit",
  "strays.hint":
    "Aquests registres continuen en un quadern que ja no existeix —normalment perquè es va unir amb un altre o es va suprimir mentre un altre dispositiu hi continuava anotant—, així que no apareixen en cap quadern, ni en imprimir-lo ni en exportar-lo. Passeu-los a un quadern de la mateixa explotació, o recupereu el quadern suprimit.",
  "strays.removed_book": "Quadern suprimit",
  // Forma «Etiqueta: N»: el nom del registre no concorda amb la xifra.
  "strays.kind_count": "{kind}: {n}",
  "strays.show_records": "Mostra els registres",
  "strays.and_more": "…i {more} més.",
  "strays.into": "Passar-los a",
  "strays.move": "Passa els registres",
  "strays.move_confirm.one": "Voleu passar el registre de «{from}» a «{into}»?",
  "strays.move_confirm.other": "Voleu passar els {count} registres de «{from}» a «{into}»?",
  "strays.moved.one": "Fet. El registre ja és a «{into}».",
  "strays.moved.other": "Fet. Els registres ja són a «{into}».",
  "strays.restore": "Recupera el quadern",
  "strays.restore_confirm":
    "Voleu recuperar el quadern «{label}» de {farm}? Tornarà a la llista de quaderns amb els registres que conserva i els que es van suprimir amb ell.",
  "strays.restored": "Fet. El quadern «{label}» de {farm} torna a ser a la llista.",
  "strays.no_book": "Aquesta explotació no té cap quadern on passar-los: recupereu aquest.",
  "sync.field.deleted_at": "Eliminat el",
  "sync.field.season_id": "Quadern",
  "sync.field.season_label": "Nom de la campanya",
  "sync.field.season_status": "Estat de la campanya",
  "sync.field.crop_code": "Codi de conreu",
  "sync.field.product_code": "Codi de producte",
  "sync.field.declared_area": "Superfície declarada (ha)",
  "sync.field.energy_type": "Tipus d'energia",
  "sync.field.richness_n": "Riquesa en nitrogen",
  "sync.field.richness_p2o5": "Riquesa en P₂O₅",
  "sync.field.richness_k2o": "Riquesa en K₂O",
  "sync.field.source": "Origen de la dada",
  "sync.field.source_campaign": "Campanya d'origen de la dada",
  "sync.field.subject": "Objecte tractat",
  "sync.field.subject_kind": "Tipus d'objecte tractat",
  "sync.field.premises": "Local o mitjà de transport",
  "sync.field.sowing": "Sembra",
  "sync.field.soil_cover": "Coberta del sòl",
  "sync.field.advisor": "Assessor",
  "sync.field.justification": "Justificació",
  "backup.title": "Còpia de seguretat",
  "backup.import_loss.one":
    "Aquest quadern té 1 canvi que la còpia no conté. En importar-la es perdrà del quadern.",
  "backup.import_loss.other":
    "Aquest quadern té {count} canvis que la còpia no conté. En importar-la es perdran del quadern.",
  "backup.import_loss_own.one":
    "Aquest canvi es va escriure en aquest dispositiu, així que no és en cap altre: la còpia de seguretat que es desa abans d'importar serà l'única que el tindrà.",
  "backup.import_loss_own.other":
    "{count} d'aquests canvis es van escriure en aquest dispositiu, així que no són en cap altre: la còpia de seguretat que es desa abans d'importar serà l'única que els tindrà.",
  "backup.import_confirm":
    "Importar una còpia de seguretat SUBSTITUEIX totes les dades actuals pel contingut de la còpia. Abans es desa una còpia de la base de dades actual. Voleu continuar?",
};
