// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Diccionario español, por áreas. Las claves son idénticas en todos los
// idiomas y ninguna puede repetirse entre archivos: i18n.js los fusiona.

export default {
  // El armazón de la pantalla de ajustes: el buscador y el índice lateral.
  // Los nombres de las secciones y de los grupos están en lib/settingsTree.js.
  "settings.search": "Buscar",
  "settings.toc": "Secciones de ajustes",
  "settings.clear_search": "Borrar la búsqueda",
  "settings.collapse_section": "Contraer {section}",
  "settings.expand_section": "Expandir {section}",
  "settings.results.one": "{count} ajuste",
  "settings.results.other": "{count} ajustes",
  "settings.no_results": "Ningún ajuste coincide con la búsqueda.",
  "settings.general": "General",
  "settings.group.language": "Idioma y formato",
  "settings.map": "Mapa",
  "settings.group.offline_maps": "Mapas sin conexión",
  "settings.group.treated_plots": "Parcelas tratadas",
  "settings.data": "Datos",
  "settings.advanced": "Avanzado",
  "settings.cache_size": "Espacio máximo para mapas sin conexión",
  "settings.cache_default": "Predeterminado ({size})",
  "settings.cache_hint":
    "Los mapas consultados se guardan para poder usarlos sin conexión; al superar el límite se eliminan primero los menos usados.",
  "settings.clear_cache": "Borrar mapas guardados",
  "settings.clear_cache_confirm":
    "¿Borrar los mapas guardados? Se descargarán de nuevo cuando haya conexión. Los datos de la explotación no se tocan.",
  "settings.phi_horizon": "Mostrar parcelas tratadas hasta (días atrás)",
  "settings.phi_horizon_hint":
    "Durante cuánto tiempo el mapa sigue señalando una parcela ya tratada cuyo plazo de seguridad ha terminado. En blanco: {days} días. No afecta al plazo de seguridad ni a las parcelas con el plazo en curso, que se muestran siempre.",
  "settings.alerts": "Avisos",
  "settings.alerts_hint":
    "Con cuánta antelación quiere que le avisemos antes de que caduque un carné de aplicador o venza la ITV de una máquina. En blanco: {licence} y {itv} días. No es un plazo legal: elija el tiempo que necesite para renovarlo, que depende de la disponibilidad de cursos y de estaciones de ITV en su zona.",
  "settings.licence_lead": "Aviso de caducidad del carné (días)",
  "settings.itv_lead": "Aviso de vencimiento de la ITV (días)",
  "settings.catalogues": "Catálogos de referencia",
  "catalogues.hint":
    "Listas oficiales de códigos (FEGA) con las que la aplicación resuelve productos, problemas, materiales y demás. Vienen dentro de la aplicación; aquí puede pedir la última publicada, y se conserva hasta que una nueva versión de la aplicación traiga la suya.",
  // Dos recuentos que concuerdan con nombres distintos: forma "Etiqueta: N",
  // que es correcta con cualquier cifra (docs/frontend-conventions.md).
  "catalogues.state": "Catálogos: {count} · Códigos: {codes}",
  "catalogues.updated_at": "última actualización: {date}",
  "catalogues.never": "aún sin importar",
  "catalogues.refresh": "Actualizar catálogos",
  "catalogues.refreshing": "Consultando el servicio…",
  "catalogues.updated": "nuevos: {added} · corregidos: {corrected}.",
  "catalogues.withdrawn.one":
    "Uno ya no figura en la lista del organismo: deja de ofrecerse, pero los registros que ya lo citan siguen resolviéndose.",
  "catalogues.withdrawn.other":
    "{count} ya no figuran en la lista del organismo: dejan de ofrecerse, pero los registros que ya los citan siguen resolviéndose.",
  "catalogues.extra_columns.one": "columna nueva que esta versión no usa: {columns}.",
  "catalogues.extra_columns.other": "columnas nuevas que esta versión no usa: {columns}.",
  "catalogues.unchanged": "Sin cambios: {count}.",
  "catalogues.refused.shape":
    "el archivo ya no tiene la forma que esta versión sabe leer; hará falta una actualización de la aplicación",
  "catalogues.refused.empty": "el archivo llegó sin datos",
  "catalogues.refused.label": "una fila llegó sin descripción",
  "catalogues.refused.control_characters": "el archivo llegó con caracteres ilegibles",
  "catalogues.refused.shrunk":
    "el archivo trae menos filas de las ya guardadas; parece una descarga incompleta",
  "catalogues.refused.network": "no se pudo conectar con el servicio",
  "catalogues.refused.http": "el servicio respondió con un error",
  "settings.profiles": "Perfiles de usuario",
  "settings.maintenance": "Mantenimiento de la base de datos",
  "settings.maintenance_hint":
    "Revisa a fondo el archivo de datos en busca de daños y, si está en buen estado, lo compacta para recuperar espacio. La aplicación ya hace una revisión rápida cada semana por su cuenta; ésta es más completa y puede tardar unos segundos.",
  "settings.check_db": "Revisar y compactar",
  "sync.title": "Sincronizar con otro dispositivo",
  "sync.hint":
    "Exporte los cambios de este dispositivo a un archivo, cópielo al otro e impórtelo allí. Un archivo en cada sentido pone los dos al día. Nada sale de sus dispositivos: el archivo va donde usted lo lleve.",
  "sync.import_confirm.one":
    "Este archivo viene de {device} y se creó el {created}. Trae un cambio. ¿Aplicarlo a este cuaderno?",
  "sync.import_confirm.other":
    "Este archivo viene de {device} y se creó el {created}. Trae {count} cambios. ¿Aplicarlos a este cuaderno?",
  "sync.pair_confirm":
    "Este dispositivo aún no está emparejado con {device}. Al emparejarlos compartirán cuaderno: los cambios de uno llegarán al otro cada vez que sincronice. ¿Emparejarlos?",
  "sync.join_other_confirm":
    "CUIDADO: este archivo viene de otro grupo de dispositivos. Si continúa, este dispositivo dejará su grupo actual y pasará al de {device}. Hágalo sólo si los dos son suyos y llevan el mismo cuaderno.",
  "sync.peers_title": "Dispositivos de esta explotación",
  "sync.peers_hint":
    "Cada dispositivo que escribe en este cuaderno aparece aquí. Póngales nombre: así, cuando dos hayan escrito el mismo registro a la vez, la pantalla dirá «Móvil de María» en lugar de un código.",
  "sync.peers_empty": "Por ahora sólo ha escrito en este cuaderno este dispositivo.",
  "sync.peer_label": "Nombre del dispositivo",
  "sync.device_this": "este dispositivo",
  "sync.device_unnamed": "un dispositivo sin nombre",
  "sync.peer_unnamed": "Sin nombre",
  "sync.peer_this_device": "Este dispositivo",
  "sync.peer_known_since": "Conocido desde",
  "sync.peer_retired": "Retirado",
  "sync.peer_retire": "Retirar",
  "sync.peer_restore": "Reactivar",
  "sync.peer_retire_confirm":
    "¿Retirar «{name}»? Lo que escribió sigue en el cuaderno y sigue a su nombre; sólo se deja de esperar que vuelva a sincronizar. Puede reactivarlo cuando quiera.",
  "conflicts.title": "Registros escritos a la vez",
  "conflicts.hint":
    "Dos dispositivos escribieron estos registros sin haberse visto. El cuaderno muestra una de las versiones en todos los dispositivos; elija cuál se queda.",
  "conflicts.review": "Revisar",
  "conflicts.written_on": "Escrito en {devices}",
  "conflicts.in_book": "Campaña {season} de {farm}",
  "conflicts.version_live": "Es la versión que muestra el cuaderno",
  "conflicts.version_waiting": "Versión en espera",
  "conflicts.written_by": "Escrito por {person}",
  "conflicts.written_at": "Escrito el {date}",
  "conflicts.keep": "Conservar ésta",
  "conflicts.keep_confirm":
    "¿Conservar la versión de {device}? El cuaderno la mostrará en todos los dispositivos. La otra no se pierde: la base de datos guarda todas las versiones del registro.",
  "conflicts.kept": "Listo. El cuaderno muestra la versión de {device}.",
  "conflicts.identical":
    "Las dos versiones dicen exactamente lo mismo. Conserve cualquiera de ellas para cerrar la diferencia.",
  "conflicts.edit_hint":
    "Si la respuesta correcta no es ninguna de las dos, corrija el registro en su pantalla: cualquier cambio cierra la diferencia en todos los dispositivos.",
  "conflicts.yes": "Sí",
  "conflicts.no": "No",
  "conflicts.absent": "—",
  "duplicates.title": "Posibles duplicados",
  "duplicates.hint":
    "Estos registros se parecen tanto que pueden ser una misma operación anotada dos veces. Revise cada pareja: conserve uno de los dos, o indique que son dos registros distintos.",
  "duplicates.review": "Revisar",
  "duplicates.written_by": "Anotados por {people}",
  "duplicates.in_book": "Campaña {season} de {farm}",
  "duplicates.record": "Registro {n}",
  "duplicates.on_device": "En {device}",
  "duplicates.by_person": "Por {person}",
  "duplicates.written_at": "Anotado el {date}",
  "duplicates.differs_hint": "Lo resaltado es lo que no coincide entre los dos.",
  "duplicates.keep": "Conservar el registro {n}",
  "duplicates.keep_confirm":
    "¿Conservar el registro {n} y eliminar el otro? El eliminado dejará de aparecer en el cuaderno, y queda anotado por qué se eliminó.",
  "duplicates.kept": "Listo. Se ha conservado un registro y eliminado el otro.",
  "duplicates.both_real": "Son dos registros distintos",
  "duplicates.judged_distinct":
    "Anotado: son dos registros distintos. No se volverá a preguntar por ellos.",
  "duplicates.both_removed": "Ya no está en el cuaderno",
  "duplicates.both_removed_hint":
    "Esta operación ya no está en el cuaderno. Estaba anotada dos veces, y cada dispositivo conservó una copia distinta y eliminó la otra antes de sincronizarse: al juntarse, quedaron eliminadas las dos. Recupere una para que vuelva a constar una vez.",
  "duplicates.restore": "Recuperar el registro {n}",
  "duplicates.restored": "Listo. El registro vuelve a estar en el cuaderno.",
  "duplicates.book_notice.one": "Un posible duplicado en esta campaña.",
  "duplicates.book_notice.other": "{count} posibles duplicados en esta campaña.",
  "duplicates.book_title": "Posibles duplicados de esta campaña",
  "duplicates.removed_each": "Una copia la eliminó {first} y la otra, {second}",
  "duplicates.remover": "{person} en {device}",
  "duplicates.removed_by": "Eliminado por {person} en {device} el {date}",
  "duplicates.removed_on_device": "Eliminado en {device} el {date}",
  "duplicates.identical": "Los dos registros dicen exactamente lo mismo: da igual cuál conserve.",
  "duplicates.identical_removed":
    "Las dos copias dicen exactamente lo mismo: da igual cuál recupere.",
  "duplicates.back": "Volver a la lista",
  "duplicates.just_saved": "El que acaba de guardar",
  "duplicates.saved_hint":
    "Lo que acaba de guardar se parece mucho a un registro que ya estaba. Si es una misma operación anotada dos veces, conserve uno; si son dos, indíquelo. También puede cerrar y decidirlo después: quedará en la lista de posibles duplicados.",
  "strays.title": "Registros en un cuaderno eliminado",
  "strays.hint":
    "Estos registros siguen en un cuaderno que ya no existe —normalmente porque se unió con otro o se eliminó mientras otro dispositivo seguía anotando en él—, así que no aparecen en ningún cuaderno, ni al imprimirlo ni al exportarlo. Páselos a un cuaderno de la misma explotación, o recupere el cuaderno eliminado.",
  "strays.removed_book": "Cuaderno eliminado",
  // Forma «Etiqueta: N»: el nombre del registro no concuerda con la cifra.
  "strays.kind_count": "{kind}: {n}",
  "strays.show_records": "Ver los registros",
  "strays.and_more": "…y {more} más.",
  "strays.into": "Pasarlos a",
  "strays.move": "Pasar los registros",
  "strays.move_confirm.one": "¿Pasar el registro de «{from}» a «{into}»?",
  "strays.move_confirm.other": "¿Pasar los {count} registros de «{from}» a «{into}»?",
  "strays.moved.one": "Listo. El registro está ahora en «{into}».",
  "strays.moved.other": "Listo. Los registros están ahora en «{into}».",
  "strays.restore": "Recuperar el cuaderno",
  "strays.restore_confirm":
    "¿Recuperar el cuaderno «{label}» de {farm}? Volverá a la lista de cuadernos con los registros que conserva y los que se eliminaron con él.",
  "strays.restored": "Listo. El cuaderno «{label}» de {farm} vuelve a estar en la lista.",
  "strays.no_book": "Esta explotación no tiene ningún cuaderno donde pasarlos: recupere este.",
  "sync.field.deleted_at": "Eliminado el",
  "sync.field.season_id": "Cuaderno",
  "sync.field.season_label": "Nombre de la campaña",
  "sync.field.season_status": "Estado de la campaña",
  "sync.field.crop_code": "Código de cultivo",
  "sync.field.product_code": "Código de producto",
  "sync.field.declared_area": "Superficie declarada (ha)",
  "sync.field.energy_type": "Tipo de energía",
  "sync.field.richness_n": "Riqueza en nitrógeno",
  "sync.field.richness_p2o5": "Riqueza en P₂O₅",
  "sync.field.richness_k2o": "Riqueza en K₂O",
  "sync.field.source": "Origen del dato",
  "sync.field.source_campaign": "Campaña de origen del dato",
  "sync.field.subject": "Objeto tratado",
  "sync.field.subject_kind": "Tipo de objeto tratado",
  "sync.field.premises": "Local o medio de transporte",
  "sync.field.sowing": "Siembra",
  "sync.field.soil_cover": "Cubierta del suelo",
  "sync.field.advisor": "Asesor",
  "sync.field.justification": "Justificación",
  "backup.title": "Copia de seguridad",
  "backup.import_loss.one":
    "Este cuaderno tiene 1 cambio que la copia no contiene. Al importarla se perderá del cuaderno.",
  "backup.import_loss.other":
    "Este cuaderno tiene {count} cambios que la copia no contiene. Al importarla se perderán del cuaderno.",
  "backup.import_loss_own.one":
    "Ese cambio se escribió en este dispositivo, así que no está en ningún otro: la copia de seguridad que se guarda antes de importar será la única que lo tenga.",
  "backup.import_loss_own.other":
    "{count} de esos cambios se escribieron en este dispositivo, así que no están en ningún otro: la copia de seguridad que se guarda antes de importar será la única que los tenga.",
  "backup.import_confirm":
    "Importar una copia de seguridad SUSTITUYE todos los datos actuales por el contenido de la copia. Antes se guarda una copia de la base de datos actual. ¿Continuar?",
};
