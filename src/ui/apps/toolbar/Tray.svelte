<!-- Notification area icons on the toolbar, plus the overflow button ("^") for the ones
     the user moved away. Icons are dragged to reorder them or to move them into / out of
     the overflow flyout, like on the Windows taskbar (see shared/trayDrag.ts). -->
<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "@shared/components/Icon.svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api, Events, on, trayIconUrl } from "@shared/ipc.ts";
  import { trayDraggable } from "@shared/trayDrag.ts";
  import type { TrayAction, TrayDrag, TrayIcon } from "@shared/types.ts";
  import { hideTooltip, openPopup, tooltip } from "./overlays.ts";
  import { toolbar } from "./state.svelte.ts";

  let iconsEl: HTMLDivElement | null = $state(null);
  let overflowButton: HTMLButtonElement | null = $state(null);
  /** key of the icon being dragged (from here or from the flyout) */
  let dragKey = $state<string | null>(null);
  /** the drag started on this toolbar */
  let localDrag = $state(false);
  let dropIndex = $state<number | null>(null);
  let openedByDrag = false;
  /** a dragged icon is held over the "^" button */
  let overButton = $state(false);

  const icons = $derived(toolbar.inlineTray);
  /** position of each icon among the icons that are not being dragged (-1: dragged) */
  const slots = $derived.by(() => {
    let n = 0;
    return icons.map((i) => (i.key === dragKey ? -1 : n++));
  });
  const staying = $derived(slots.filter((s) => s >= 0).length);
  /** the icon dragged in from the flyout, shown where it would land */
  const incoming = $derived(
    dragKey && dropIndex !== null ? toolbar.tray.find((i) => i.key === dragKey && !i.inline) : undefined,
  );

  function send(e: MouseEvent, icon: TrayIcon, action: TrayAction) {
    e.preventDefault();
    e.stopPropagation();
    hideTooltip();
    api.trayIconAction(icon.id, action);
  }

  /** where the dragged icon lands for a cursor at `x` (CSS px of this window) */
  function indexFor(x: number, key: string): number {
    const buttons = Array.from(iconsEl?.querySelectorAll<HTMLElement>(".tray-icon[data-key]") ?? []).filter(
      (b) => b.dataset.key !== key,
    );
    return buttons.filter((b) => {
      const r = b.getBoundingClientRect();
      return r.left + r.width / 2 < x;
    }).length;
  }

  function openOverflow(activate: boolean) {
    if (!overflowButton) return;
    // with the icons, so the flyout is complete (and measured) on its first render
    openPopup(overflowButton, "tray-overflow", { icons: $state.snapshot(toolbar.tray) }, undefined, { activate });
  }

  /** the cursor (CSS px of this window) is over the "^" button */
  function overOverflowButton(x: number): boolean {
    const r = overflowButton?.getBoundingClientRect();
    return !!r && x >= r.left && x <= r.right;
  }

  onMount(() => {
    const unlisten = on<TrayDrag>(Events.TrayDrag, (d) => {
      const onButton = d.inside && overOverflowButton(d.x);
      if (d.phase === "move") {
        dragKey = d.key;
        overButton = onButton;
        dropIndex = d.inside && !onButton ? indexFor(d.x, d.key) : null;
        // held over "^" or dragged off the toolbar: open the flyout to drop the icon in
        if ((onButton || !d.inside) && localDrag && !openedByDrag) {
          openedByDrag = true;
          openOverflow(false);
        }
        return;
      }
      if (d.phase === "end" && d.inside) {
        // dropped on "^": hidden; anywhere else on the toolbar: shown at that place
        const place = onButton ? api.trayPlace(d.key, false) : api.trayPlace(d.key, true, indexFor(d.x, d.key));
        place.catch(console.error);
      }
      dragKey = null;
      dropIndex = null;
      overButton = false;
    });
    return () => {
      unlisten.then((f) => f());
    };
  });
</script>

<div class="tray" role="toolbar" bind:this={iconsEl}>
  <!-- always there, like the taskbar's "^": icons are dropped on it to hide them -->
  <button
    type="button"
    class="tb-button tray-overflow"
    class:drop={overButton}
    aria-label={t("toolbar.trayOverflow")}
    bind:this={overflowButton}
    use:tooltip={() => t("toolbar.trayOverflow")}
    onclick={() => openOverflow(true)}
  >
    <Icon name={toolbar.placement === "above" ? "ChevronUp" : "ChevronDown"} />
  </button>
  {#each icons as icon, i (icon.id)}
    {#if dropIndex !== null && dropIndex === slots[i]}
      {@render marker()}
    {/if}
    <button
      type="button"
      class="tb-button tray-icon"
      class:dragging={dragKey === icon.key}
      data-key={icon.key}
      aria-label={icon.tooltip}
      use:tooltip={() => icon.tooltip}
      use:trayDraggable={{
        key: icon.key,
        onStart: () => {
          hideTooltip();
          localDrag = true;
          openedByDrag = false;
        },
        onEnd: () => (localDrag = false),
      }}
      onclick={(e) => send(e, icon, "left")}
      ondblclick={(e) => send(e, icon, "double")}
      oncontextmenu={(e) => send(e, icon, "right")}
      onauxclick={(e) => e.button === 1 && send(e, icon, "middle")}
      onmousedown={(e) => e.button === 1 && e.preventDefault()}
    >
      <img src={trayIconUrl(icon)} alt="" draggable="false" />
    </button>
  {/each}
  {#if dropIndex !== null && dropIndex >= staying}
    {@render marker()}
  {/if}
</div>

{#snippet marker()}
  {#if incoming}
    <span class="tb-button tray-icon drop-ghost"><img src={trayIconUrl(incoming)} alt="" /></span>
  {:else}
    <span class="tray-drop-marker"></span>
  {/if}
{/snippet}
