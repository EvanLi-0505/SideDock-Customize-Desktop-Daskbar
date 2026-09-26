<script lang="ts">
  import Icon from "@shared/components/Icon.svelte";
  import Switch from "@shared/components/Switch.svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api } from "@shared/ipc.ts";
  import type { PopupProps } from "../registry.ts";

  let { data, close }: PopupProps = $props();
  // svelte-ignore state_referenced_locally
  let enabled = $state<boolean | null>(data?.enabled ?? null);
  let busy = $state(false);
  const loaded = true;

  async function toggle(value: boolean) {
    if (busy) return;
    busy = true;
    try {
      await api.setBluetooth(value);
      enabled = value;
    } catch (err) {
      console.error(err);
    } finally {
      busy = false;
    }
  }
</script>

<div class="popup-surface bluetooth">
  <div class="row">
    <Icon name={enabled ? "Bluetooth" : "BluetoothOff"} size={18} />
    <span class="label">{t("bluetooth.title")}</span>
    {#if enabled === null && loaded}
      <span class="muted">{t("bluetooth.unavailable")}</span>
    {:else}
      <Switch checked={!!enabled} disabled={busy || !loaded} onchange={toggle} />
    {/if}
  </div>
  <button
    type="button"
    class="popup-link"
    onclick={() => {
      api.shellAction("bluetooth-settings");
      close();
    }}
  >
    {t("bluetooth.settings")}
  </button>
</div>

<style>
  .bluetooth {
    width: 300px;
    padding: 10px;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px;
    border-radius: var(--radius-m);
    background: var(--surface-2);
  }

  .label {
    flex: 1;
    font-weight: 700;
  }

  .muted {
    color: var(--fg-muted);
    font-size: 12px;
  }
</style>
