<script lang="ts">
  import Icon from "@shared/components/Icon.svelte";
  import { t, type TranslationKey } from "@shared/i18n/index.svelte.ts";
  import { api } from "@shared/ipc.ts";
  import type { PowerStatus } from "@shared/types.ts";
  import type { PopupProps } from "../registry.ts";

  let { data, close }: PopupProps = $props();
  const power = $derived<PowerStatus | null>(data ?? null);

  type Action = "lock" | "signOut" | "sleep" | "hibernate" | "restart" | "shutdown";
  const actions: { id: Action; icon: string; label: TranslationKey; confirm: boolean }[] = [
    { id: "lock", icon: "Lock", label: "power.lock", confirm: false },
    { id: "signOut", icon: "LogOut", label: "power.signOut", confirm: true },
    { id: "sleep", icon: "Moon", label: "power.sleep", confirm: false },
    { id: "restart", icon: "RotateCcw", label: "power.restart", confirm: true },
    { id: "shutdown", icon: "PowerOff", label: "power.shutdown", confirm: true },
  ];

  // destructive actions need a second click
  let armed = $state<Action | null>(null);
  let armTimer: ReturnType<typeof setTimeout> | undefined;

  function run(action: (typeof actions)[number]) {
    if (action.confirm && armed !== action.id) {
      armed = action.id;
      clearTimeout(armTimer);
      armTimer = setTimeout(() => (armed = null), 3000);
      return;
    }
    close();
    api.powerAction(action.id);
  }

  const status = $derived.by(() => {
    if (!power) return "";
    if (!power.hasBattery) return t("power.noBattery");
    if (power.charging) return t("power.charging");
    return power.pluggedIn ? t("power.pluggedIn") : t("power.onBattery");
  });
</script>

<div class="popup-surface power">
  {#if power?.hasBattery}
    <div class="battery">
      <div class="battery-head">
        <span class="battery-pct">{power.percentage}%</span>
        <span class="battery-status">{status}{power.saver ? ` · ${t("power.saver")}` : ""}</span>
      </div>
      <div class="battery-bar">
        <div
          class="battery-fill"
          class:low={power.percentage <= 15 && !power.charging}
          style:width="{power.percentage}%"
        ></div>
      </div>
    </div>
  {/if}

  <div class="actions">
    {#each actions as action (action.id)}
      <button type="button" class="action" class:armed={armed === action.id} onclick={() => run(action)}>
        <Icon name={action.icon} size={18} />
        <span>{armed === action.id ? t("power.confirm") : t(action.label)}</span>
      </button>
    {/each}
  </div>

  <button
    type="button"
    class="popup-link"
    onclick={() => {
      api.shellAction("power-settings");
      close();
    }}
  >
    {t("power.settings")}
  </button>
</div>

<style>
  .power {
    width: 300px;
    padding: 12px;
  }

  .battery {
    padding: 12px;
    border-radius: var(--radius-m);
    background: var(--surface-2);
    margin-bottom: 10px;
  }

  .battery-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    margin-bottom: 8px;
  }

  .battery-pct {
    font-family: var(--font-display);
    font-size: 24px;
    font-weight: 700;
  }

  .battery-status {
    font-size: 12px;
    color: var(--fg-secondary);
  }

  .battery-bar {
    height: 6px;
    border-radius: 3px;
    background: var(--surface-pressed);
    overflow: hidden;
  }

  .battery-fill {
    height: 100%;
    border-radius: 3px;
    background: var(--success);
    transition: width 300ms var(--ease);

    &.low {
      background: var(--danger);
    }
  }

  .actions {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 4px;
  }

  .action {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px;
    border-radius: var(--radius-m);
    font-weight: 600;
    font-size: 13px;
    color: var(--fg);
    text-align: left;
    transition: background-color var(--dur-fast) ease;

    &:hover {
      background: var(--surface-hover);
    }
    &.armed {
      background: var(--danger-soft);
      color: var(--danger);
    }
  }
</style>
