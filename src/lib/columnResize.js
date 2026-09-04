// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Draggable column boundaries for a .data-table, as a Svelte action:
//
//     <table class="data-table" use:resizableColumns={"operators"}>
//
// An action rather than a wrapper component because the tables are hand-written
// per register with their own columns and their own cells — wrapping them would
// mean handing every column through snippets, and this needs no say in what a
// table renders. It attaches to a <thead> and nothing else.
//
// It is view tier, not the agnostic one: the handle is a component now (see
// ColumnResizer.svelte), so this imports Svelte's `mount`.
//
// TWO THINGS MAKE IT WORK, and both were measured rather than assumed.
//
// `.data-table` is `table-layout: fixed`, so the header row decides the columns
// and setting a width on a <th> IS the column width. Under auto layout the same
// code is advisory at best.
//
// And fixed layout only applies while the table has a DEFINITE width. A first
// attempt gave the resized table `width: auto` so it could grow past its pane;
// the widths were written correctly and the browser ignored every one of them,
// falling back to automatic layout — inline `218px, 151px, …` rendering as
// `98, 139, 220, …`. So the table carries an explicit pixel width, the sum of
// its columns, and `widths` below is the authority: nothing reads a rendered
// width back, because under a wrong layout that reads back nonsense.
//
// Widths are written with CSSOM (`el.style.width = …`), never
// setAttribute("style", …), which the production CSP blocks
// (docs/frontend-conventions.md → Styling).
import { mount, unmount } from "svelte";

import { t } from "../i18n.js";
import ColumnResizer from "./ColumnResizer.svelte";
import { clampWidth, forgetWidths, readWidths, saveWidths } from "./columnWidths.js";

/// A keyboard step, and the bigger one Shift asks for.
const STEP = 16;
const BIG_STEP = 64;

/// How close together two presses on one boundary mean "put it back". The
/// platform double-click interval is not readable from the web, so this is the
/// usual default.
const DOUBLE_TAP_MS = 400;

export function resizableColumns(table, tableId) {
  const headers = [...table.querySelectorAll("thead th")];
  if (headers.length === 0) return {};

  const store = globalThis.localStorage;
  const handles = [];
  /// The authority on column widths once the reader has touched them; null
  /// while the stylesheet's equal shares are still in charge.
  let widths = readWidths(store, tableId, headers.length);
  let dragging = null;
  /// The last press that did NOT move, so a second one on the same boundary can
  /// be read as a double-click.
  let lastTap = { index: -1, at: 0 };

  function apply() {
    headers.forEach((th, i) => (th.style.width = `${widths[i]}px`));
    // The definite width fixed layout needs, and the reason the table can grow
    // past its pane and scroll .table-wrap instead of squeezing its neighbours.
    table.style.width = `${widths.reduce((sum, w) => sum + w, 0)}px`;
    table.classList.add("sized");
  }

  /// Take over from the stylesheet, pinning every column at what it currently
  /// renders as.
  ///
  /// Every column, not only the one being dragged: fixed layout divides the
  /// table between the columns that declare nothing, so stating a width for one
  /// silently redistributes all the others — drag the first column and the last
  /// four move.
  function pinAll() {
    if (widths) return;
    widths = headers.map((th) => clampWidth(th.getBoundingClientRect().width));
    apply();
  }

  function setWidth(index, px) {
    const width = clampWidth(px);
    if (width === null) return;
    widths[index] = width;
    apply();
  }

  /// Back to the stylesheet's equal shares. The only way back once dragged, so
  /// it is on the handle itself rather than hidden in a menu.
  function reset() {
    widths = null;
    headers.forEach((th) => th.style.removeProperty("width"));
    table.style.removeProperty("width");
    table.classList.remove("sized");
    forgetWidths(store, tableId);
  }

  function onPointerDown(event, index) {
    pinAll();
    event.currentTarget.setPointerCapture(event.pointerId);
    dragging = { index, startX: event.clientX, startW: widths[index], moved: false };
    // NO preventDefault here, and that is deliberate. Its job would be to stop
    // the drag selecting text, which `body { user-select: none }` already does
    // app-wide; what it also does is suppress the compatibility mouse events,
    // and with them the click and dblclick the reset gesture is listening for.
    // Measured: with it, double-clicking a handle did nothing at all.
    // (`event.detail` is not an alternative — the UI Events spec fixes it at 0
    // for pointerdown, so counting presses there never fires either.)
  }

  function onPointerMove(event) {
    if (!dragging) return;
    const delta = event.clientX - dragging.startX;
    if (delta !== 0) dragging.moved = true;
    setWidth(dragging.index, dragging.startW + delta);
  }

  /// Releasing ends the drag — or, when nothing moved, counts as one tap of the
  /// double-tap that resets.
  ///
  /// Counted here rather than through a `dblclick` listener because there is no
  /// dblclick to listen to: capturing the pointer suppresses the browser's
  /// click-count synthesis, so `dblclick` never fires on the handle and every
  /// `click` arrives with `detail` stuck at 1. Both measured.
  function onPointerUp(event) {
    if (!dragging) return;
    const { index, moved } = dragging;
    dragging = null;
    if (moved) {
      saveWidths(store, tableId, widths);
      lastTap = { index: -1, at: 0 };
      return;
    }
    const at = event?.timeStamp ?? 0;
    if (lastTap.index === index && at - lastTap.at < DOUBLE_TAP_MS) {
      reset();
      lastTap = { index: -1, at: 0 };
    } else {
      lastTap = { index, at };
    }
  }

  function onKeyDown(event, index) {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    pinAll();
    setWidth(
      index,
      widths[index] + (event.key === "ArrowRight" ? 1 : -1) * (event.shiftKey ? BIG_STEP : STEP),
    );
    saveWidths(store, tableId, widths);
    event.preventDefault();
  }

  headers.forEach((th, index) => {
    // Read BEFORE mounting, and named because "resize, button" repeated six
    // times down a header row says nothing about which column is about to move.
    const label = t("table.resize_column", { column: th.textContent.trim() });
    // Mounted rather than hand-built: the handle carries an owned tooltip now,
    // and a component cannot be attached to a createElement'd button. The
    // gestures stay here — this file owns every measurement — and travel down
    // as callbacks.
    handles.push(
      mount(ColumnResizer, {
        target: th,
        props: {
          label,
          onpointerdown: (event) => onPointerDown(event, index),
          onpointermove: onPointerMove,
          onpointerup: onPointerUp,
          onkeydown: (event) => onKeyDown(event, index),
        },
      }),
    );
  });

  if (widths) apply();

  return {
    destroy() {
      for (const handle of handles) unmount(handle);
    },
  };
}
