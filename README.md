# Terrazgo

**Aplicación libre y de código abierto para la gestión integral de la explotación agrícola.**

🌐 **[terrazgo.com](https://terrazgo.com)**

Terrazgo funciona primero sin conexión (*offline-first*): todos los datos viven en tu
dispositivo, en una base de datos local, y la aplicación sigue funcionando completa sin
cobertura — pensada para el campo, no para la oficina. Hay versiones para escritorio
(Linux y Windows) y para Android.

> ⚠️ **En desarrollo activo.** Todavía no hay una versión estable. Las versiones
> publicadas en [Releases](../../releases) son versiones tempranas para probar y opinar:
> hasta la primera versión estable, actualizar puede requerir empezar con una base de
> datos nueva — no guardes todavía datos que no puedas permitirte perder.

## Módulos

- **Cuaderno de explotación (CUE)** — el primer módulo, ya en pruebas. Cubre los
  registros del **RD 1311/2012** (tratamientos fitosanitarios y actuaciones no químicas,
  semilla tratada, tratamientos de postcosecha, locales y medios de transporte, analíticas
  y cosecha) y los del **RD 1051/2022**, obligatorio desde el 1 de enero de 2026
  (fertilización, plan de abonado y riego). Registra productos, operadores, asesores y
  maquinaria, avisa de los plazos de seguridad y de la caducidad de carnés e ITV, y
  cualquier registro se puede corregir. El cuaderno se imprime en **PDF** siguiendo el
  modelo oficial y se exporta también como **hoja de cálculo**, en castellano y en
  catalán. Todo ello con la vista puesta en el registro electrónico obligatorio desde
  2027 (RD 34/2025, Reglamento UE 2023/564).
- **Ecorregímenes** — los registros que el **RD 1048/2022** obliga a anotar en el cuaderno
  a quien solicita un ecorrégimen: pastoreo extensivo (P1), siega sostenible e islas de
  biodiversidad (P2), espacios de biodiversidad en cultivos bajo agua (P5), cubiertas
  vegetales (P6) y cubiertas inertes de restos de poda (P7), más las labores de
  mantenimiento de los pastos comunales del anexo IV. Se imprimen en el apartado 9 del
  cuaderno, y el aviso de estado señala las anotaciones que aún faltan.
- **Mapas y SIGPAC** — mapa de la explotación con dibujo de recintos, importación de
  ficheros (GeoJSON/GeoPackage), consulta SIGPAC (verificación de referencias, superficie
  oficial, zonas vulnerables a nitratos / Natura 2000 / restricciones fitosanitarias),
  capas del parcelario y de cultivos declarados, y localización por GPS en el móvil.
- **Fertilización** — registro de materias fertilizantes con su composición, aplicaciones,
  plan de abonado y registro de riego.
- **En camino** — planificación del riego, planificación de cultivos y costes.

El cuaderno es el primer módulo, no el producto: Terrazgo es una aplicación de gestión
de toda la explotación, para cualquier cultivo y cualquier comunidad autónoma.

## Descargas

En [Releases](../../releases) encontrarás los instaladores de cada versión:

- **Linux** — AppImage, paquete `.deb` (Debian/Ubuntu) y paquete `.rpm` (Fedora/openSUSE)
- **Windows** — instalador `.exe` y versión portable
- **Android** — APK para instalación directa (aarch64)

## Incidencias y sugerencias

¿Algo no funciona o echas algo en falta? Abre una
[incidencia](../../issues/new/choose) — hay plantillas para errores y propuestas.

## Código fuente y licencia

Este repositorio contiene el código fuente completo de cada versión publicada
(una instantánea por versión). Licencia
[AGPL-3.0-or-later](LICENSE): libre de usar, estudiar, modificar y redistribuir;
cualquier versión derivada que se distribuya u ofrezca como servicio debe publicar
también su código fuente.
