# Terrazgo

**Aplicación libre y de código abierto para gestionar toda la explotación agrícola.**

🌐 **[terrazgo.com](https://terrazgo.com)**

Terrazgo funciona sin conexión. Los datos se guardan en tu propio dispositivo y la
aplicación sigue funcionando entera aunque no haya cobertura, porque está pensada para
usarse en el campo. Hay versiones para escritorio (Linux y Windows) y para Android.

> ⚠️ **En desarrollo activo.** Todavía no hay una versión estable. Las versiones de
> [Releases](../../releases) son versiones tempranas para probar y dar tu opinión. Hasta
> la primera versión estable, al actualizar puede que tengas que empezar con una base de
> datos nueva, así que no guardes todavía datos que no puedas permitirte perder.

## Qué hace

- **Cuaderno de explotación.** Es el primer módulo y ya está en pruebas. Recoge lo que
  pide el **RD 1311/2012**: tratamientos fitosanitarios y medidas no químicas, semilla
  tratada, tratamientos de postcosecha, de almacenes y de vehículos, análisis y cosecha.
  También lo del **RD 1051/2022**, obligatorio desde el 1 de enero de 2026:
  fertilización, plan de abonado y riego. Llevas un cuaderno por explotación y campaña,
  registras productos, aplicadores, asesores y maquinaria, y la aplicación te avisa de
  los plazos de seguridad y de cuándo caducan los carnés y las ITV. Cualquier registro se
  puede corregir. El cuaderno se imprime en **PDF** con el modelo oficial y se exporta
  también como **hoja de cálculo**, en castellano o en catalán. Todo pensado para el
  registro electrónico obligatorio desde 2027 (RD 34/2025, Reglamento UE 2023/564).
- **Ecorregímenes.** Las anotaciones que el **RD 1048/2022** exige en el cuaderno a quien
  pide un ecorrégimen: pastoreo extensivo (P1), siega sostenible e islas de biodiversidad
  (P2), espacios de biodiversidad en cultivos bajo agua (P5), cubiertas vegetales (P6) y
  cubiertas inertes de restos de poda (P7), además del mantenimiento de los pastos
  comunales del anexo IV. Salen en el apartado 9 del cuaderno, y la aplicación te dice
  qué anotaciones te faltan.
- **Varios dispositivos, un mismo cuaderno.** Puedes anotar en el campo con el móvil y
  repasarlo en casa con el ordenador, o llevar el cuaderno entre varias personas. Los
  dispositivos se sincronizan copiando un archivo, sin nube ni cuentas. Si dos personas
  cambian el mismo registro, Terrazgo te enseña las dos versiones para que elijas; si una
  misma operación se anota dos veces, te avisa. Un cuaderno borrado se puede recuperar
  durante 30 días.
- **Mapas y SIGPAC.** Mapa de la explotación en el que puedes dibujar los recintos o
  importarlos (GeoJSON o GeoPackage). Consulta el SIGPAC para comprobar referencias, ver
  la superficie oficial y saber si una parcela está en zona vulnerable a nitratos, en Red
  Natura 2000 o en una zona con restricciones fitosanitarias. Muestra las capas del
  parcelario y de los cultivos declarados, y en el móvil te localiza por GPS.
- **Fertilización.** Registro de abonos con su composición, de las aplicaciones, del plan
  de abonado y del riego.
- **Catálogos oficiales.** Los catálogos del FEGA (cultivos, plagas, productos,
  unidades…) vienen incluidos para que todo funcione sin conexión, y puedes actualizarlos
  desde *Ajustes* cuando quieras.
- **Próximamente.** Sincronizar por wifi o bluetooth, planificación del riego y de los
  cultivos, y costes.

El cuaderno es el primer módulo, no el producto: Terrazgo quiere servir para gestionar
toda la explotación, con cualquier cultivo y en cualquier comunidad autónoma.

## Descargas

En [Releases](../../releases) tienes los instaladores de cada versión:

- **Linux**: AppImage, paquete `.deb` (Debian/Ubuntu) y paquete `.rpm` (Fedora/openSUSE)
- **Windows**: instalador `.exe` y versión portable
- **Android**: APK para instalar directamente (aarch64)

## Problemas y sugerencias

¿Algo no funciona o echas algo en falta? Abre una
[incidencia](../../issues/new/choose); hay plantillas para avisar de errores y para
proponer mejoras.

## Código fuente y licencia

Este repositorio tiene el código fuente completo de cada versión publicada, una copia por
versión. La licencia es [AGPL-3.0-or-later](LICENSE): puedes usarlo, estudiarlo,
modificarlo y redistribuirlo, y cualquier versión derivada que se distribuya o se ofrezca
como servicio tiene que publicar también su código fuente.
