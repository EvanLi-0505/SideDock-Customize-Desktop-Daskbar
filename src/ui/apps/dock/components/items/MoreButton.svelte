<!-- "More" button of the Collapse overflow mode: opens a grid with the apps that did not fit. -->
<script lang="ts">
  import Icon from "@shared/components/Icon.svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { iconUrl } from "@shared/ipc.ts";
  import type { AppDockItem } from "@shared/types.ts";
  import { getWindowsForItem } from "../../state/items.svelte.ts";
  import { systemState } from "../../state/system.svelte.ts";
  import { activateApp } from "../../menus.svelte.ts";
  import { openPopup, tooltip } from "../../overlays.ts";

  let { items }: { items: AppDockItem[] } = $props();
  let el: HTMLDivElement | null = $state(null);

  const running = $derived(items.filter((i) => getWindowsForItem(i, systemState.windows).length > 0).length);

  function open() {
    const data = items.map((item) => ({
      id: item.id,
      label: item.displayName,
      icon: iconUrl(item.relaunch?.command || item.path, item.umid),
      running: getWindowsForItem(item, systemState.windows).length > 0,
    }));
    openPopup(el!, "overflow", { title: t("dock.moreApps"), items: data }, {
      key: "overflow",
      onAction: (action, value) => {
        if (action !== "activate") return;
        const item = items.find((i) => i.id === value);
        if (item) activateApp(item, getWindowsForItem(item, systemState.windows), el!);
      },
    });
  }
</script>

<div class="weg-item-overlay" bind:this={el}>
  <div
    role="button"
    tabindex="-1"
    class="weg-item weg-module weg-more"
    use:tooltip={() => t("dock.moreApps")}
    onclick={open}
    onkeydown={() => {}}
  >
    <Icon name="LayoutGrid" class="weg-module-icon" />
  </div>
  <div class="weg-item-custom-badge">{items.length}</div>
  <div class="weg-item-open-sign" class:weg-item-open-sign-active={running > 0}></div>
</div>
