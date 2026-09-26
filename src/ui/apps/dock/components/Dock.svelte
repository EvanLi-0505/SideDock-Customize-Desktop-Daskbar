<!-- Port of Seelen UI `weg/components/Dock.svelte` (dnd-kit replaced by ../sortable.svelte.ts) -->
<script lang="ts">
  import { flip } from "svelte/animate";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api } from "@shared/ipc.ts";
  import type { DockItem } from "@shared/types.ts";
  import { dockState, getWindowsForItem, listToGroups } from "../state/items.svelte.ts";
  import { layout } from "../state/layout.svelte.ts";
  import { systemState } from "../state/system.svelte.ts";
  import { showDockMenu } from "../menus.ts";
  import { draggable, sortable } from "../sortable.svelte.ts";
  import ItemSwitch from "./ItemSwitch.svelte";

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

  let itemsEl: HTMLDivElement | null = $state(null);

  // report the natural content length so the backend can size the window (fit-content mode)
  $effect(() => {
    if (!itemsEl) return;
    const el = itemsEl;
    const report = () => {
      const length = layout.horizontal ? el.scrollWidth : el.scrollHeight;
      api.dockSetContentLength(length + layout.settings.padding * 2);
    };
    const observer = new ResizeObserver(report);
    observer.observe(el);
    report();
    return () => observer.disconnect();
  });
</script>

{#snippet group(id: string, items: DockItem[])}
  <div class="weg-items-{id}">
    {#each items as item (item.id)}
      <div
        class="weg-item-drag-container"
        class:dragging={sortable.draggingId === item.id}
        data-item-id={item.id}
        use:draggable={item.id}
        animate:flip={{ duration: sortable.draggingId ? 160 : 0 }}
      >
        <ItemSwitch {item} />
      </div>
    {/each}
  </div>
{/snippet}

<div
  role="toolbar"
  tabindex="-1"
  class="taskbar {layout.side}"
  class:horizontal={layout.horizontal}
  class:vertical={!layout.horizontal}
  class:hidden={systemState.hidden}
  data-size={settings.mode === "FullWidth" ? "full-width" : "min-content"}
  data-has-margin={settings.margin > 0}
  oncontextmenu={showDockMenu}
>
  <div class="taskbar-bg"></div>
  <div class="weg-items-container">
    <div class="weg-items" bind:this={itemsEl}>
      {#if isEmpty}
        <span class="weg-empty-state-label">{t("weg.empty")}</span>
      {:else}
        {@render group("left", visible.left)}
        {@render group("center", visible.center)}
        {@render group("right", visible.right)}
      {/if}
    </div>
  </div>
</div>
