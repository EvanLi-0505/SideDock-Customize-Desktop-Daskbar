<script lang="ts">
  import Icon from "@shared/components/Icon.svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api } from "@shared/ipc.ts";
  import type { ModuleDockItem } from "@shared/types.ts";
  import { systemState } from "../../state/system.svelte.ts";
  import { showItemMenu } from "../../menus.svelte.ts";
  import { openPopup, tooltip } from "../../overlays.ts";
  import { shouldSuppressClick } from "../../sortable.svelte.ts";

  let { item }: { item: ModuleDockItem } = $props();

  const power = $derived(systemState.system.power);
  const icon = $derived.by(() => {
    if (!power?.hasBattery) return "Power";
    if (power.charging) return "BatteryCharging";
    if (power.percentage >= 80) return "BatteryFull";
    if (power.percentage >= 40) return "BatteryMedium";
    return "BatteryLow";
  });
  const tip = $derived(
    power?.hasBattery
      ? `${power.percentage}% · ${power.charging ? t("power.charging") : power.pluggedIn ? t("power.pluggedIn") : t("power.onBattery")}`
      : t("module.power"),
  );
  let el: HTMLDivElement | null = $state(null);
</script>

<div class="weg-item-overlay" bind:this={el}>
  <div
    role="button"
    tabindex="-1"
    class="weg-item weg-module"
    class:low={power?.hasBattery && power.percentage <= 15 && !power.charging}
    use:tooltip={() => tip}
    onclick={() => !shouldSuppressClick() && openPopup(el!, "power", $state.snapshot(power), { key: "power" })}
    oncontextmenu={(e) =>
      showItemMenu(e, item, [{ type: "item", id: "settings", label: t("power.settings"), icon: "Settings" }], () =>
        api.shellAction("power-settings"),
      )}
    onkeydown={() => {}}
  >
    <Icon name={icon} class="weg-module-icon" />
  </div>
  {#if power?.hasBattery}
    <div class="weg-item-custom-badge">{power.percentage}</div>
  {/if}
</div>
