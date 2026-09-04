<!-- SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz -->
<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<script>
  // The app's tab strip, on Bits UI.
  //
  // It changes nothing about the LOOK: Bits UI is headless, so `.tabstrip` and
  // `.tab` in styles.css are what draw it either way. What it buys is the
  // behaviour the hand-rolled strip never had — arrow keys and Home/End move
  // between tabs, one tab stop for the whole strip rather than one per tab, and
  // the `aria-controls`/`aria-labelledby` pair that ties a tab to its panel.
  // The hand-rolled version set `role="tab"` and `aria-selected` and stopped
  // there, which is a partial ARIA contract: it tells a screen reader this is a
  // tab list and then gives it nothing to navigate.
  //
  // WHAT DOES NOT FIT GOES INTO A MENU (2026-09-03), the shape GitHub's repo
  // nav uses. The strip used to be `overflow-x: auto` and the tabs that did not
  // fit were simply off the edge — which is the worst of both worlds on the two
  // screens that matter: a sideways scroller shows nothing that says more tabs
  // exist, and on a phone it competes with the page's own scrolling. So the
  // strip is measured, the tabs that fit stay in it, and the rest are offered
  // by a "More" button beside them — the `.tabrow` that wraps both.
  //
  // The class is `.tabrow` because `.tabbar` is the SHELL's phone navigation;
  // naming it that made the two trade rules in both directions, and the second
  // direction — a negative margin reaching the phone nav and scrolling the
  // whole document sideways at 1280px — is invisible from this file.
  //
  // TABS KEEP THEIR ORDER; the menu never promotes the current one into the
  // strip. Reordering a strip under the reader to keep the selection visible
  // is a worse trade than the one thing it fixes, so the button carries the
  // selected marker instead, and the row inside the menu is ticked.
  //
  // A view never imports bits-ui (docs/frontend-conventions.md → Owned
  // controls), which is why this wrapper exists. `preventScroll={false}` does
  // not appear on the strip on purpose: that default is about the body scroll
  // lock a FLOATING layer takes, and a tab strip does not float — the rule was
  // narrowed to say so on 2026-08-26. The menu below it DOES float, and
  // TzMenu passes it.
  import { tick, untrack } from "svelte";
  import { Separator, Tabs } from "bits-ui";
  import { t } from "../i18n.js";
  import TzMenu from "./TzMenu.svelte";
  import { visibleTabCount } from "./tabOverflow.js";

  let {
    /// [{ value, label }] — the strip, in order.
    items = [],
    value = $bindable(items[0]?.value),
    /// Called when the reader picks a different tab. Bound state alone cannot
    /// say WHEN it changed, and a caller usually has something to forget: a
    /// selection made in the old panel means nothing in the new one.
    onchange = null,
    /// A strip drawn directly under another strip — section 9's registers
    /// inside the record book's eco-schemes tab. It reads lighter (see
    /// `.tabrow.subtabs`), so two levels are not mistaken for one. Not a
    /// property of being nested at all: the product inspector's strip sits
    /// under a form rather than under a strip, and has nothing to be confused
    /// with.
    nested = false,
    /// This strip's panel is a PANE of a frame rather than a block of a page:
    /// it fills the height left over and lets what is inside it scroll, which
    /// is what keeps the bands above from moving (see `.tabpanel-framed`).
    ///
    /// **A strip inside a `.view.framed` must pass this.** Without it the panel
    /// never claims the leftover height, so the panes inside it have none to
    /// scroll within — and because the frame clips, their content is not
    /// merely unscrolled but unreachable. It fails silently and it looks like
    /// nothing: the catalogue's edit panels stopped scrolling exactly this way
    /// when the flag replaced the structural selector and one caller was left
    /// behind.
    ///
    /// Declared rather than inferred from where the strip sits, despite that.
    /// It used to be `.view.framed > .tabs-root > .tabpanel`, which could not
    /// tell the record book's nested strips (a pane) from the product
    /// inspector's (a block inside a scrolling pane), and widening it to reach
    /// the first would have silently restyled the second.
    framed = false,
    /// Rendered inside the active tab's panel, so the panel is what the tab
    /// actually controls rather than a sibling that happens to change.
    panel,
  } = $props();

  let rowEl = $state(null);
  let stripEl = $state(null);
  let moreEl = $state(null);

  /// How many tabs the strip may hold. UNBOUNDED until the row has been
  /// measured, and `Infinity` says so where `items.length` only looked like it
  /// did: a `$state` initialiser captures the value it is given ONCE, so seeding
  /// it from a prop is a reference Svelte warns about and a number that stops
  /// following the list it came from. `slice` takes Infinity happily.
  ///
  /// Unbounded is also the only safe starting answer: a tab's width can be read
  /// only while it is in the DOM, so showing everything is what makes the first
  /// measurement possible.
  let limit = $state(Infinity);

  // Measurements, deliberately NOT $state. They are inputs the split reads, and
  // making them reactive would restart the very effect that took them.
  let widths = [];
  let moreWidth = 0;
  let available = 0;

  const shown = $derived(items.slice(0, limit));
  const hidden = $derived(items.slice(limit));
  const currentIsHidden = $derived(hidden.some((item) => item.value === value));

  /// Natural tab widths, and the width of the overflow group beside them.
  ///
  /// Widths can only be READ while every tab is in the strip, so the cache is
  /// refreshed on exactly those passes and reused on the others. That is what
  /// makes it self-healing rather than merely cached: every time the row is
  /// wide enough to hold everything, the numbers are taken again — after a
  /// font swap, a zoom, or a locale whose labels are longer.
  function measure() {
    const tabs = stripEl?.querySelectorAll(".tab") ?? [];
    if (tabs.length > 0 && tabs.length === items.length) {
      const measured = [...tabs].map((el) => el.getBoundingClientRect().width);
      if (measured.every((width) => width > 0)) widths = measured;
    }
    if (moreEl) moreWidth = moreEl.getBoundingClientRect().width;
  }

  /// Re-run the split against the current measurements.
  ///
  /// TWO PASSES, and the second is not defensive padding. The overflow group is
  /// only in the DOM while something overflows, so the first time a strip runs
  /// out of room it is measured with `moreWidth` still 0 and keeps one tab too
  /// many. The second pass runs with the button on screen and measurable, and
  /// then agrees with itself — the inputs no longer change, which is why this
  /// terminates rather than chasing its own tail.
  async function resplit() {
    for (let pass = 0; pass < 2; pass += 1) {
      if (!rowEl || !stripEl) return;
      measure();
      // A cache left over from a different `items` cannot answer for this one,
      // and answering anyway would hide tabs it never measured. Staying
      // unbounded is what puts them all back in the DOM for the next pass.
      if (widths.length !== items.length) return;
      const gap = parseFloat(getComputedStyle(stripEl).columnGap) || 0;
      const next = visibleTabCount({ widths, gap, available, moreWidth });
      if (next === limit) return;
      limit = next;
      await tick();
    }
  }

  // The ROW is observed, never the strip. Our own split changes what is INSIDE
  // the strip, so observing it would feed the observer its own output; the
  // row's width comes from the layout above and is unmoved by anything decided
  // here. `contentRect` is the content box, which is what the tabs have to
  // share — the row's own full-bleed padding is not available to them.
  $effect(() => {
    if (!rowEl) return;
    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) available = entry.contentRect.width;
      resplit();
    });
    observer.observe(rowEl);
    return () => observer.disconnect();
  });

  // A locale switch remounts the routed view, so longer labels normally arrive
  // with a fresh component. A strip nested inside a panel does not always get
  // that, and `items` is the prop carrying the labels either way. Putting every
  // tab back in the strip first is what lets `measure()` read the new widths.
  //
  // `untrack` is load-bearing: `resplit` READS `limit` to decide whether
  // anything changed, and this effect WRITES it. Tracked, that is a loop —
  // write, re-run, write again — and it is the whole reason the measurements
  // above are plain variables rather than state.
  $effect(() => {
    // Reading the prop is what subscribes this effect to a new list; `void`
    // rather than a value nobody uses, so it is plainly the subscription and
    // not a leftover.
    void items;
    untrack(() => {
      limit = Infinity;
      resplit();
    });
  });

  function pick(next) {
    value = next;
    onchange?.(next);
  }
</script>

<!-- `display: contents` on the root (see .tabs-root): the row and the panel
     have to be laid out by whatever contains this component — inside a framed
     view they are the fixed band and the growing body — and a wrapper box in
     between would break that chain. -->
<Tabs.Root bind:value onValueChange={(next) => onchange?.(next)} class="tabs-root">
  <div class="tabrow" class:subtabs={nested} bind:this={rowEl}>
    <!-- The menu button is a SIBLING of the tab list, never a child of it: a
         `role="tablist"` whose children are not all tabs is a broken ARIA
         contract, and the button is a menu trigger rather than a tab. -->
    <Tabs.List class="tabstrip" bind:ref={stripEl}>
      {#each shown as item (item.value)}
        <Tabs.Trigger value={item.value} class="tab">{item.label}</Tabs.Trigger>
      {/each}
    </Tabs.List>

    {#if hidden.length > 0}
      <!-- Measured as ONE group, divider included, so the split reserves what
           the row actually gives up rather than the button alone. -->
      <div class="tab-overflow" bind:this={moreEl}>
        <Separator.Root orientation="vertical" decorative class="tab-divider" />
        <TzMenu
          items={hidden}
          value={currentIsHidden ? value : ""}
          label={t("tabs.more")}
          triggerClass={currentIsHidden ? "tab tab-more is-current" : "tab tab-more"}
          onselect={pick}
        />
      </div>
    {/if}
  </div>

  {#each items as item (item.value)}
    <Tabs.Content value={item.value} class={framed ? "tabpanel tabpanel-framed" : "tabpanel"}>
      {#if value === item.value}
        {@render panel(item)}
      {/if}
    </Tabs.Content>
  {/each}
</Tabs.Root>
