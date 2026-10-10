<!-- The notification area icons running now, each shown on the toolbar or in its overflow
     flyout. Same effect as dragging them on the toolbar itself. -->
<script lang="ts">
  import { onMount } from "svelte";
  import Switch from "@shared/components/Switch.svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api, Events, on, trayIconUrl } from "@shared/ipc.ts";
  import type { TrayIcon } from "@shared/types.ts";

  let icons = $state<TrayIcon[]>([]);
  let error = $state("");

  onMount(() => {
    api.getTrayIcons().then((list) => (icons = list));
    const unlisten = on<TrayIcon[]>(Events.SystemTrayChanged, (list) => (icons = list));
    return () => {
      unlisten.then((f) => f());
    };
  });

  async function place(icon: TrayIcon, inline: boolean) {
    try {
      error = "";
      await api.trayPlace(icon.key, inline);
    } catch (err) {
      error = String(err);
    }
  }
</script>

{#if icons.length}
  <ul class="tray-list">
    {#each icons as icon (icon.id)}
      <li>
        <img src={trayIconUrl(icon)} alt="" />
        <span class="name" title={icon.tooltip}>{icon.tooltip || t("toolbar.trayUnnamed")}</span>
        <Switch checked={icon.inline} label={t("toolbar.trayInline")} onchange={(v) => place(icon, v)} />
      </li>
    {/each}
  </ul>
{:else}
  <p class="muted">{t("toolbar.trayNone")}</p>
{/if}
{#if error}<p class="error">{error}</p>{/if}

<style>
  .tray-list {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 6px 16px;
    margin: 4px 0;
    padding: 0;
    list-style: none;

    li {
      display: flex;
      align-items: center;
      gap: 10px;
      padding: 6px 4px;
      border-bottom: 1px solid var(--border);
    }

    img {
      width: 16px;
      height: 16px;
      flex-shrink: 0;
    }
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
  }

  .muted {
    color: var(--fg-muted);
    font-size: 13px;
  }
  .error {
    color: var(--danger);
    font-size: 12.5px;
  }
</style>
