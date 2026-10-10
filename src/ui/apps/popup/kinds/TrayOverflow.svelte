<!-- The toolbar's overflow flyout ("^"): notification area icons the user moved off the
     toolbar. Icons are dragged back onto the toolbar, or dropped here from it. -->
<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api, Events, on, trayIconUrl } from "@shared/ipc.ts";
  import { trayDraggable } from "@shared/trayDrag.ts";
  import type { TrayAction, TrayDrag, TrayIcon } from "@shared/types.ts";
  import type { PopupProps } from "../registry.ts";

  // `data.icons`: the tray as the toolbar had it when it opened this flyout, so the first
  // render (the one the window is sized from) is already complete; later changes come
  // from the tray itself
  let { data, close }: PopupProps = $props();

  // a starting value on purpose (the flyout is created anew each time it opens)
  let all = $state<TrayIcon[]>(untrack(() => (data?.icons as TrayIcon[] | undefined) ?? []));
  const icons = $derived(all.filter((i) => !i.inline));
  /** an icon from the toolbar is dragged over this flyout */
  let incoming = $state<TrayIcon | null>(null);
  let dragging = $state<string | null>(null);

  onMount(() => {
    const changed = on<TrayIcon[]>(Events.SystemTrayChanged, (list) => (all = list));
    const drag = on<TrayDrag>(Events.TrayDrag, (d) => {
      const icon = all.find((i) => i.key === d.key);
      if (d.phase === "move") {
        incoming = d.inside && icon?.inline ? icon : null;
        return;
      }
      if (d.phase === "end" && d.inside && icon?.inline) {
        api.trayPlace(d.key, false).catch(console.error);
      }
      incoming = null;
    });
    return () => {
      changed.then((f) => f());
      drag.then((f) => f());
    };
  });

  function send(e: MouseEvent, icon: TrayIcon, action: TrayAction) {
    e.preventDefault();
    e.stopPropagation();
    api.trayIconAction(icon.id, action);
  }
</script>

<div class="popup-surface tray-overflow" class:drop={!!incoming} role="presentation" onkeydown={(e) => e.key === "Escape" && close()}>
  <div class="grid">
    {#each icons as icon (icon.id)}
      <button
        type="button"
        class="icon"
        class:dragging={dragging === icon.key}
        title={icon.tooltip}
        use:trayDraggable={{
          key: icon.key,
          onStart: () => (dragging = icon.key),
          onEnd: () => (dragging = null),
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
    {#if incoming}
      <span class="icon ghost"><img src={trayIconUrl(incoming)} alt="" /></span>
    {/if}
  </div>
  {#if !icons.length && !incoming}
    <p class="hint">{t("toolbar.trayOverflowEmpty")}</p>
  {/if}
</div>

<style>
  .tray-overflow {
    padding: 8px;
    min-width: 136px;
    max-width: 232px;
    transition: box-shadow var(--dur-fast) ease;

    &.drop {
      box-shadow:
        0 0 0 2px var(--accent),
        var(--shadow-m);
    }
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(5, 40px);
    gap: 2px;
  }

  .icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 40px;
    height: 36px;
    border-radius: var(--radius-s);
    touch-action: none;
    transition: background-color var(--dur-fast) ease;

    &:hover {
      background: var(--surface-hover);
    }
    &.dragging {
      opacity: 0.3;
    }
    &.ghost {
      opacity: 0.6;
      outline: 1px dashed var(--accent);
      outline-offset: -3px;
    }

    /* 16 px like the taskbar: the icons are shown pixel for pixel */
    img {
      width: 16px;
      height: 16px;
      pointer-events: none;
    }
  }

  .hint {
    max-width: 210px;
    margin: 2px 4px;
    color: var(--fg-muted);
    font-size: 11px;
    line-height: 1.4;
  }
</style>
