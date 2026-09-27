<!-- Port of Seelen UI `weg/components/Dock.svelte` (dnd-kit replaced by ../sortable.svelte.ts),
     extended with overflow handling (shrink / stack / collapse) and the magnification wave. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { flip } from "svelte/animate";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api, Events, on } from "@shared/ipc.ts";
  import type { AppDockItem, DockItem } from "@shared/types.ts";
  import { dockState, getWindowsForItem, listToGroups } from "../state/items.svelte.ts";
  import { layout } from "../state/layout.svelte.ts";
  import { systemState } from "../state/system.svelte.ts";
  import { showDockMenu } from "../menus.svelte.ts";
  import { draggable, sortable } from "../sortable.svelte.ts";
  import { fitItems } from "../fit.ts";
  import { POPUP_OPENED_EVENT } from "../overlays.ts";
  import { DockWave } from "../wave.ts";
  import ItemSwitch from "./ItemSwitch.svelte";
  import MoreButton from "./items/MoreButton.svelte";

  const settings = $derived(layout.settings);

  function isItemVisible(item: DockItem): boolean {
    const monitor = systemState.dock?.monitor;
    const showPinned = settings.pinnedItemsVisibility === "Always" || !!monitor?.isPrimary;
    const windows =
      settings.temporalItemsVisibility === "OnMonitor" && monitor
        ? systemState.windows.filter((w) => w.monitor === monitor.id)
        : systemState.windows;
    if (item.type !== "App") return showPinned;
    if (item.pinned && showPinned) return true;
    return getWindowsForItem(item, windows).length > 0;
  }

  const groups = $derived(listToGroups(dockState.items, true));
  const visible = $derived({
    left: groups.left.filter(isItemVisible),
    center: groups.center.filter(isItemVisible),
    right: groups.right.filter(isItemVisible),
  });
  const isEmpty = $derived(
    [...visible.left, ...visible.center, ...visible.right].filter((i) => i.type !== "Separator")
      .length === 0,
  );

  // ---------------- overflow: shrink / stack / collapse ----------------

  let viewport = $state({ width: window.innerWidth, height: window.innerHeight });
  onMount(() => {
    const onResize = () => (viewport = { width: window.innerWidth, height: window.innerHeight });
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  });

  const kindOf = (item: DockItem) =>
    item.type === "Separator" ? "separator" : item.type === "Module" && item.module === "media" ? "large" : "normal";

  const centerApps = $derived(visible.center.filter((i): i is AppDockItem => i.type === "App"));
  const fit = $derived.by(() => {
    const all = [...visible.left, ...visible.center, ...visible.right];
    const along = layout.horizontal ? viewport.width : viewport.height;
    return fitItems({
      available: along - 2 * settings.margin - 2 * settings.padding,
      size: settings.size,
      gap: settings.spaceBetweenItems,
      mode: settings.overflowMode,
      normal: all.filter((i) => kindOf(i) === "normal").length,
      large: all.filter((i) => kindOf(i) === "large").length,
      separators: all.filter((i) => kindOf(i) === "separator").length,
      collapsible: centerApps.length,
    });
  });
  const collapsedItems = $derived(fit.collapsed ? centerApps.slice(-fit.collapsed) : []);
  const centerShown = $derived.by(() => {
    const hidden = new Set(collapsedItems.map((i) => i.id));
    return visible.center.filter((i) => !hidden.has(i.id));
  });

  $effect(() => {
    const root = document.documentElement.style;
    root.setProperty("--item-size", `${fit.size.toFixed(2)}px`);
    root.setProperty("--stack-overlap", `${fit.overlap.toFixed(2)}px`);
  });

  // ---------------- magnification wave ----------------

  let barEl: HTMLDivElement | null = $state(null);
  let containerEl: HTMLDivElement | null = $state(null);
  let wave: DockWave | null = $state(null);

  // the name label is drawn by the tooltip window (keeps the dock window small)
  const labelSink = {
    show: (text: string, anchor: { x: number; y: number; width: number; height: number }) =>
      api.tooltipShow(text, anchor, layout.placement),
    move: (anchor: { x: number; y: number; width: number; height: number }) =>
      api.tooltipFollow(anchor, layout.placement),
    hide: () => api.tooltipHide(),
  };

  $effect(() => {
    if (!barEl || !containerEl) return;
    const instance = new DockWave(containerEl, barEl, labelSink);
    wave = instance;
    return () => {
      instance.destroy();
      wave = null;
    };
  });

  const pitch = $derived(fit.size + settings.spaceBetweenItems - fit.overlap);
  $effect(() => {
    if (!wave) return;
    const along = layout.horizontal ? viewport.width : viewport.height;
    const barLength = barEl ? (layout.horizontal ? barEl.offsetWidth : barEl.offsetHeight) : along;
    wave.configure({
      enabled: settings.magnification,
      horizontal: layout.horizontal,
      side: layout.side,
      scale: settings.magnificationScale,
      range: settings.magnificationRange,
      size: fit.size,
      pitch: Math.max(1, pitch),
      fixedEnds: settings.mode === "FullWidth",
      freeLength: Math.max(0, along - 2 * settings.margin - barLength),
      labels: settings.showLabels,
    });
  });

  $effect(() => {
    wave?.suspend(!!sortable.draggingId);
  });

  $effect(() => {
    if (systemState.hidden) wave?.leave();
  });

  onMount(() => {
    const unlisten = on(Events.DockPointerLeave, () => wave?.leave());
    const move = (e: PointerEvent) => wave?.pointer(e.clientX, e.clientY);
    const out = (e: MouseEvent) => {
      if (!e.relatedTarget) wave?.leave();
    };
    const popupOpened = () => wave?.hideLabel();
    window.addEventListener("pointermove", move);
    window.addEventListener(POPUP_OPENED_EVENT, popupOpened);
    document.addEventListener("mouseout", out);
    return () => {
      unlisten.then((f) => f());
      window.removeEventListener("pointermove", move);
      window.removeEventListener(POPUP_OPENED_EVENT, popupOpened);
      document.removeEventListener("mouseout", out);
    };
  });

  // ---------------- hitbox for the backend ----------------
  // The window is larger than the bar; only the bar (plus the magnified icons while
  // hovering) may receive the mouse.

  $effect(() => {
    if (!barEl) return;
    const bar = barEl;
    const root = document.getElementById("root")!;
    const zoom = settings.magnificationScale - 1;
    const inward = settings.magnification ? fit.size * zoom + 6 : 0;
    // largest push past the bar ends: amplitude * falloff area (see wave.ts)
    const along = settings.magnification
      ? (fit.size * zoom * (settings.magnificationRange + 1)) / 2 + fit.size
      : 0;
    let last = "";
    const report = () => {
      // offsets ignore the auto-hide slide transform, which is what we want
      const box = {
        x: root.offsetLeft + bar.offsetLeft,
        y: root.offsetTop + bar.offsetTop,
        width: bar.offsetWidth,
        height: bar.offsetHeight,
        inward,
        along,
      };
      const key = JSON.stringify(box);
      if (key !== last) {
        last = key;
        api.dockSetHitbox(box);
      }
    };
    const observer = new ResizeObserver(report);
    observer.observe(bar);
    report();
    return () => observer.disconnect();
  });

  function labelFor(item: DockItem): string {
    if (item.type === "App") return item.displayName;
    if (item.type === "Module") return t(`module.${item.module}` as never);
    return "";
  }
</script>

{#snippet group(id: string, items: DockItem[])}
  <div class="weg-items-{id}">
    {#each items as item (item.id)}
      <div
        class="weg-item-drag-container"
        class:dragging={sortable.draggingId === item.id}
        data-item-id={item.id}
        data-kind={kindOf(item)}
        data-label={labelFor(item)}
        use:draggable={item.id}
        animate:flip={{ duration: sortable.draggingId ? 160 : 0 }}
      >
        <ItemSwitch {item} />
      </div>
    {/each}
    {#if id === "center" && collapsedItems.length}
      <div class="weg-item-drag-container" data-kind="normal" data-label={t("dock.moreApps")}>
        <MoreButton items={collapsedItems} />
      </div>
    {/if}
  </div>
{/snippet}

<div
  role="toolbar"
  tabindex="-1"
  class="taskbar {layout.side}"
  class:horizontal={layout.horizontal}
  class:vertical={!layout.horizontal}
  class:hidden={systemState.hidden}
  class:stacked={fit.overlap > 0}
  data-size={settings.mode === "FullWidth" ? "full-width" : "min-content"}
  data-has-margin={settings.margin > 0}
  oncontextmenu={showDockMenu}
  bind:this={barEl}
>
  <div class="taskbar-bg"></div>
  <div class="weg-items-container" bind:this={containerEl}>
    <div class="weg-items">
      {#if isEmpty}
        <span class="weg-empty-state-label">{t("weg.empty")}</span>
      {:else}
        {@render group("left", visible.left)}
        {@render group("center", centerShown)}
        {@render group("right", visible.right)}
      {/if}
    </div>
  </div>
</div>
