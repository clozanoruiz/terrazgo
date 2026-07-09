# Terrazgo

**Aplicación libre y de código abierto para la gestión integral de la explotación agrícola.**

Terrazgo funciona primero sin conexión (*offline-first*): todos los datos viven en tu
dispositivo, en una base de datos local, y la aplicación sigue funcionando completa sin
cobertura — pensada para el campo, no para la oficina. Escritorio hoy; móvil en el plan.

> ⚠️ **En desarrollo activo.** Todavía no hay una versión estable. Las versiones
> publicadas en [Releases](../../releases) son versiones tempranas para probar y opinar:
> hasta la primera versión estable, actualizar puede requerir empezar con una base de
> datos nueva — no guardes todavía datos que no puedas permitirte perder.

## Módulos

- **Cuaderno de explotación (CUE)** — el primer módulo: registro de tratamientos
  fitosanitarios (productos, operadores, maquinaria, plazos de seguridad con alertas),
  alineado con el registro electrónico obligatorio desde el 1 de enero de 2027
  (RD 1311/2012, RD 34/2025, Reglamento UE 2023/564).
- **Mapas y SIGPAC** — mapa de la explotación con dibujo de recintos, importación de
  ficheros (GeoJSON/GeoPackage), consulta SIGPAC (verificación de referencias, superficie
  oficial, zonas vulnerables a nitratos / Natura 2000 / restricciones fitosanitarias).
- **En camino** — riego, planificación de cultivos, costes, fertilización y suelo.

El cuaderno es el primer módulo, no el producto: Terrazgo es una aplicación de gestión
de toda la explotación, para cualquier cultivo y cualquier comunidad autónoma.

## Descargas

En [Releases](../../releases) encontrarás los instaladores de cada versión:

- **Linux** — AppImage y paquete `.deb`
- **Windows** — instalador `.exe`
- **Android** — más adelante

## Incidencias y sugerencias

¿Algo no funciona o echas algo en falta? Abre una
[incidencia](../../issues/new/choose) — hay plantillas para errores y propuestas,
en español.

## Código fuente y licencia

Este repositorio contiene el código fuente completo de cada versión publicada
(una instantánea por versión). Licencia
[AGPL-3.0-or-later](LICENSE): libre de usar, estudiar, modificar y redistribuir;
cualquier versión derivada que se distribuya u ofrezca como servicio debe publicar
también su código fuente.
