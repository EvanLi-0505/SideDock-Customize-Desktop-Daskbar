<!-- Modules that simply trigger a shell action: start menu, show desktop, network, ... -->
<script lang="ts">
  import Icon from "@shared/components/Icon.svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api } from "@shared/ipc.ts";
  import { moduleMeta } from "@shared/modules.ts";
  import type { ModuleDockItem } from "@shared/types.ts";
  import { showItemMenu } from "../../menus.svelte.ts";
  import { tooltip } from "../../overlays.ts";
  import { shouldSuppressClick } from "../../sortable.svelte.ts";

  let { item }: { item: ModuleDockItem } = $props();

  const actions: Record<string, string> = {
    "start-menu": "start-menu",
    "show-desktop": "show-desktop",
    network: "network",
    notifications: "action-center",
    "task-manager": "task-manager",
  };
  const settingsActions: Record<string, string> = {
    network: "network-settings",
    notifications: "notification-settings",
  };

  const meta = $derived(moduleMeta(item.module));

  function onClick() {
    if (shouldSuppressClick()) return;
    const action = actions[item.module];
    if (action) api.shellAction(action);
  }

  function onContextMenu(e: MouseEvent) {
    const settingsAction = settingsActions[item.module];
    showItemMenu(
      e,
      item,
      settingsAction ? [{ type: "item", id: "settings", label: t("item_menu.settings"), icon: "Settings" }] : [],
      () => settingsAction && api.shellAction(settingsAction),
    );
  }
</script>

<div class="weg-item-overlay">
  <div
    role="button"
    tabindex="-1"
    class="weg-item weg-module"
    data-module={item.module}
    use:tooltip={() => t(`module.${item.module}` as never)}
    onclick={onClick}
    oncontextmenu={onContextMenu}
    onkeydown={() => {}}
  >
    {#if item.module === "start-menu"}
      <svg class="weg-module-start" viewBox="0 0 24 24" aria-hidden="true">
        <rect x="2" y="2" width="9.5" height="9.5" rx="1.6" />
        <rect x="12.5" y="2" width="9.5" height="9.5" rx="1.6" />
        <rect x="2" y="12.5" width="9.5" height="9.5" rx="1.6" />
        <rect x="12.5" y="12.5" width="9.5" height="9.5" rx="1.6" />
      </svg>
    {:else}
      <Icon name={meta.icon} class="weg-module-icon" />
    {/if}
  </div>
</div>
