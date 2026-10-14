<!-- One toolbar item (Settings > Toolbar decides which ones and where). -->
<script lang="ts">
  import Icon from "@shared/components/Icon.svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api } from "@shared/ipc.ts";
  import { settingsState } from "@shared/state/settings.svelte.ts";
  import { networkIcon, networkTip, volumeIcon, volumeTip } from "@shared/statusIcons.ts";
  import { customButtonOf } from "@shared/toolbarItems.ts";
  import type { ToolbarItemId } from "@shared/types.ts";
  import Clock from "./Clock.svelte";
  import CustomItem from "./CustomItem.svelte";
  import PerfItem from "./PerfItem.svelte";
  import Media from "./Media.svelte";
  import Tray from "./Tray.svelte";
  import { openMenu, openPopup, tooltip } from "./overlays.ts";
  import { toolbar } from "./state.svelte.ts";

  let { id, twoLines }: { id: ToolbarItemId; twoLines: boolean } = $props();

  const system = $derived(toolbar.system);
  const customButton = $derived(customButtonOf(id, settingsState.value.toolbar.customButtons));
  const layout = $derived(system.keyboard.find((k) => k.active) ?? system.keyboard[0]);
  const power = $derived(system.power);
  const batteryIcon = $derived.by(() => {
    if (!power?.hasBattery) return "Power" as const;
    if (power.charging) return "BatteryCharging" as const;
    if (power.percentage >= 80) return "BatteryFull" as const;
    if (power.percentage >= 40) return "BatteryMedium" as const;
    return "BatteryLow" as const;
  });
  const batteryTip = $derived(
    power?.hasBattery
      ? `${power.percentage}% · ${power.charging ? t("power.charging") : power.pluggedIn ? t("power.pluggedIn") : t("power.onBattery")}`
      : t("module.power"),
  );

  let el: HTMLButtonElement | null = $state(null);

  /** one wheel notch = 2%, like over the taskbar's volume icon */
  function volumeWheel(e: WheelEvent) {
    if (!e.deltaY) return;
    e.preventDefault();
    api.volumeStep(e.deltaY < 0 ? 2 : -2);
  }

  /** right click: a single "settings" entry */
  function settingsMenu(e: MouseEvent, label: string, action: string) {
    e.stopPropagation();
    openMenu(e, [{ type: "item", id: "settings", label, icon: "Settings" }], () => api.shellAction(action));
  }
</script>

{#if customButton}
  <CustomItem button={customButton} />
{:else if id === "cpu" || id === "memory" || id === "net-speed"}
  <PerfItem kind={id} {twoLines} />
{:else if id === "tray"}
  <Tray />
{:else if id === "clock"}
  <Clock {twoLines} />
{:else if id === "media"}
  <Media />
{:else if id === "start"}
  <button
    type="button"
    class="tb-button start"
    aria-label={t("module.start-menu")}
    use:tooltip={() => t("module.start-menu")}
    onclick={() => api.shellAction("start-menu")}
  >
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <rect x="2" y="2" width="9.5" height="9.5" rx="1.6" />
      <rect x="12.5" y="2" width="9.5" height="9.5" rx="1.6" />
      <rect x="2" y="12.5" width="9.5" height="9.5" rx="1.6" />
      <rect x="12.5" y="12.5" width="9.5" height="9.5" rx="1.6" />
    </svg>
  </button>
{:else if id === "app-name"}
  {#if toolbar.appName}
    <span class="app-name">{toolbar.appName}</span>
  {/if}
{:else if id === "keyboard"}
  {#if layout && system.keyboard.length > 1}
    <button
      type="button"
      class="tb-button tb-text"
      bind:this={el}
      use:tooltip={() => `${layout.displayName}${layout.layoutName ? ` · ${layout.layoutName}` : ""}`}
      onclick={() => openPopup(el!, "keyboard", $state.snapshot(system.keyboard))}
      oncontextmenu={(e) => settingsMenu(e, t("keyboard.more"), "keyboard-settings")}
    >
      {layout.shortLabel}
    </button>
  {/if}
{:else if id === "bluetooth"}
  {#if system.bluetooth !== null}
    <button
      type="button"
      class="tb-button"
      class:dim={!system.bluetooth}
      bind:this={el}
      use:tooltip={() => t("module.bluetooth")}
      onclick={() => openPopup(el!, "bluetooth", { enabled: system.bluetooth })}
      oncontextmenu={(e) => settingsMenu(e, t("bluetooth.settings"), "bluetooth-settings")}
    >
      <Icon name={system.bluetooth ? "Bluetooth" : "BluetoothOff"} />
    </button>
  {/if}
{:else if id === "quick-settings"}
  <!-- like the Windows 11 taskbar: live network + volume icons, a click opens quick
       settings, the wheel changes the volume, the middle button mutes -->
  <button
    type="button"
    class="tb-button"
    class:dim={system.network?.kind === "none"}
    use:tooltip={() => `${networkTip(system.network)} · ${volumeTip(system.volume)}`}
    onclick={() => api.shellAction("quick-settings")}
    oncontextmenu={(e) => settingsMenu(e, t("item_menu.settings"), "network-settings")}
    onwheel={volumeWheel}
    onauxclick={(e) => e.button === 1 && api.volumeToggleMute()}
    onmousedown={(e) => e.button === 1 && e.preventDefault()}
  >
    <Icon name={networkIcon(system.network)} />
    <Icon name={volumeIcon(system.volume)} />
  </button>
{:else if id === "battery"}
  <button
    type="button"
    class="tb-button"
    class:low={power?.hasBattery && power.percentage <= 15 && !power.charging}
    bind:this={el}
    use:tooltip={() => batteryTip}
    onclick={() => openPopup(el!, "power", $state.snapshot(power))}
    oncontextmenu={(e) => settingsMenu(e, t("power.settings"), "power-settings")}
  >
    <Icon name={batteryIcon} />
    {#if power?.hasBattery}
      <span class="tb-small">{power.percentage}%</span>
    {/if}
  </button>
{:else if id === "notifications"}
  <button
    type="button"
    class="tb-button"
    use:tooltip={() => t("module.notifications")}
    onclick={() => api.shellAction("action-center")}
    oncontextmenu={(e) => settingsMenu(e, t("item_menu.settings"), "notification-settings")}
  >
    <Icon name="Bell" />
  </button>
{:else if id === "task-manager"}
  <button
    type="button"
    class="tb-button"
    use:tooltip={() => t("module.task-manager")}
    onclick={() => api.shellAction("task-manager")}
  >
    <Icon name="SquareActivity" />
  </button>
{:else if id === "show-desktop"}
  <!-- the sliver at the end of the Windows taskbar -->
  <button
    type="button"
    class="show-desktop"
    aria-label={t("module.show-desktop")}
    use:tooltip={() => t("module.show-desktop")}
    onclick={() => api.shellAction("show-desktop")}
  ></button>
{/if}
