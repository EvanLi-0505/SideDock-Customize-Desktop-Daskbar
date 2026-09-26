<script lang="ts">
  import Icon from "@shared/components/Icon.svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api } from "@shared/ipc.ts";
  import type { ModuleDockItem } from "@shared/types.ts";
  import { systemState } from "../../state/system.svelte.ts";
  import { showItemMenu } from "../../menus.ts";
  import { openPopup, tooltip } from "../../overlays.ts";
  import { shouldSuppressClick } from "../../sortable.svelte.ts";

  let { item }: { item: ModuleDockItem } = $props();
  const enabled = $derived(systemState.system.bluetooth === true);
  let el: HTMLDivElement | null = $state(null);
</script>

<div class="weg-item-overlay" bind:this={el}>
  <div
    role="button"
    tabindex="-1"
    class="weg-item weg-module"
    class:active={enabled}
    use:tooltip={() => t("module.bluetooth")}
    onclick={() => !shouldSuppressClick() && openPopup(el!, "bluetooth", { enabled: systemState.system.bluetooth }, { key: "bluetooth" })}
    oncontextmenu={(e) =>
      showItemMenu(e, item, [{ type: "item", id: "settings", label: t("bluetooth.settings"), icon: "Settings" }], () =>
        api.shellAction("bluetooth-settings"),
      )}
    onkeydown={() => {}}
  >
    <Icon name={enabled ? "Bluetooth" : "BluetoothOff"} class="weg-module-icon" />
  </div>
</div>
