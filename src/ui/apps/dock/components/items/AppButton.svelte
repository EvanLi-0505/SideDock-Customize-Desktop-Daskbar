<!-- Port of Seelen UI `weg/components/items/UserApplicationItem.svelte` -->
<script lang="ts">
  import { api, iconUrl } from "@shared/ipc.ts";
  import type { AppDockItem, UserAppWindow } from "@shared/types.ts";
  import { layout } from "../../state/layout.svelte.ts";
  import { systemState } from "../../state/system.svelte.ts";
  import { activateApp, launchItem, showAppMenu } from "../../menus.svelte.ts";
  import { tooltip } from "../../overlays.ts";
  import { shouldSuppressClick } from "../../sortable.svelte.ts";

  let { item, windows }: { item: AppDockItem; windows: UserAppWindow[] } = $props();

  const settings = $derived(layout.settings);
  const isFocused = $derived(windows.some((w) => w.hwnd === systemState.focused?.hwnd));
  const title = $derived(settings.showWindowTitle && layout.horizontal && windows.length ? windows[0]!.title : null);
  const icon = $derived(iconUrl(item.relaunch?.command || item.path || windows[0]?.path, item.umid));
  const tip = $derived(windows.length === 1 && windows[0]!.title ? windows[0]!.title : item.displayName);

  let el: HTMLDivElement | null = $state(null);

  function onClick() {
    if (shouldSuppressClick()) return;
    activateApp(item, windows, el!);
  }

  function onAuxClick(e: MouseEvent) {
    if (e.button !== 1) return;
    e.preventDefault();
    if (settings.middleClickAction === "OpenNewInstance" || !windows.length) {
      launchItem(item);
    } else {
      api.windowClose(windows[0]!.hwnd);
    }
  }
</script>

<div class="weg-item-overlay" bind:this={el}>
  <div
    role="button"
    tabindex="-1"
    class="weg-item"
    class:has-title={!!title}
    use:tooltip={() => tip}
    onclick={onClick}
    onauxclick={onAuxClick}
    onmousedown={(e) => e.button === 1 && e.preventDefault()}
    oncontextmenu={(e) => showAppMenu(e, item, windows)}
    onkeydown={() => {}}
  >
    <img class="weg-item-icon" src={icon} alt="" draggable="false" />
    {#if title}
      <div class="weg-item-title">{title}</div>
    {/if}
  </div>

  {#if settings.showInstanceCounter && windows.length > 1}
    <div class="weg-item-instance-counter-badge">{windows.length}</div>
  {/if}

  {#if !title}
    <div
      class="weg-item-open-sign"
      class:weg-item-open-sign-active={windows.length > 0}
      class:weg-item-open-sign-focused={isFocused}
    ></div>
  {/if}
</div>
