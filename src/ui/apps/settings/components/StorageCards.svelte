<!-- Cards to open and clean the portable data folders (logs, user data, cache, config). -->
<script lang="ts">
  import { ask } from "@tauri-apps/plugin-dialog";
  import Icon from "@shared/components/Icon.svelte";
  import { t, type TranslationKey } from "@shared/i18n/index.svelte.ts";
  import { api } from "@shared/ipc.ts";
  import type { DataKind, StorageUsage } from "@shared/types.ts";
  import { formatBytes } from "@shared/utils.ts";

  let { kinds = ["logs", "userdata", "cache"] }: { kinds?: Exclude<DataKind, "root">[] } = $props();

  const meta: Record<Exclude<DataKind, "root">, { icon: string; label: TranslationKey; hint: TranslationKey }> = {
    logs: { icon: "FileCode2", label: "data.logs", hint: "data.logsHint" },
    userdata: { icon: "HardDrive", label: "data.userdata", hint: "data.userdataHint" },
    cache: { icon: "LayoutGrid", label: "data.cache", hint: "data.cacheHint" },
    config: { icon: "Settings", label: "data.config", hint: "data.configHint" },
  };

  let usage = $state<StorageUsage | null>(null);
  let busy = $state<string | null>(null);
  let messages = $state<Partial<Record<DataKind, string>>>({});
  let restartNeeded = $state(false);

  async function refresh() {
    usage = await api.getStorageUsage();
  }
  refresh();

  async function clear(kind: Exclude<DataKind, "root">) {
    if (busy) return;
    if (kind === "config" && !(await ask(t("data.confirmConfig"), { title: "SideDock", kind: "warning" }))) {
      return;
    }
    busy = kind;
    try {
      const result = await api.clearData(kind);
      messages[kind] = result.skipped
        ? t("data.clearedSkipped", { count: result.skipped })
        : result.restartRequired
          ? t("common.restartRequired")
          : t("data.cleared");
      if (result.restartRequired) restartNeeded = true;
      await refresh();
    } catch (err) {
      messages[kind] = String(err);
    } finally {
      busy = null;
    }
  }
</script>

<div class="cards">
  {#each kinds as kind (kind)}
    <div class="card">
      <div class="card-top">
        <span class="card-icon"><Icon name={meta[kind].icon} size={18} /></span>
        <div class="card-text">
          <span class="card-title">{t(meta[kind].label)}</span>
          <span class="card-size">{usage ? formatBytes(usage[kind]) : "…"}</span>
        </div>
      </div>
      <p class="card-hint">{messages[kind] ?? t(meta[kind].hint)}</p>
      <div class="card-actions">
        <button type="button" class="btn btn-ghost" onclick={() => api.openDataDir(kind)}>
          <Icon name="FolderOpen" size={14} />
          {t("common.open")}
        </button>
        <button type="button" class="btn btn-soft-danger" disabled={busy === kind} onclick={() => clear(kind)}>
          <Icon name={kind === "config" ? "RotateCcw" : "Trash2"} size={14} />
          {kind === "config" ? t("common.reset") : t("common.clear")}
        </button>
      </div>
    </div>
  {/each}
</div>

{#if restartNeeded}
  <div class="restart-banner">
    <Icon name="Info" size={16} />
    <span>{t("common.restartRequired")}</span>
    <button type="button" class="btn btn-primary" onclick={() => api.restartApp()}>{t("common.restartNow")}</button>
  </div>
{/if}

<style>
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
    gap: 12px;
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px;
    border-radius: var(--radius-l);
    background: var(--card-bg);
    border: 1px solid var(--border);
  }

  .card-top {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .card-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    border-radius: 10px;
    background: var(--accent-soft);
    color: var(--accent);
  }

  .card-text {
    display: flex;
    flex-direction: column;
  }

  .card-title {
    font-weight: 700;
  }

  .card-size {
    font-size: 12.5px;
    color: var(--fg-muted);
    font-variant-numeric: tabular-nums;
  }

  .card-hint {
    flex: 1;
    font-size: 12.5px;
    color: var(--fg-secondary);
    min-height: 34px;
  }

  .card-actions {
    display: flex;
    gap: 8px;
  }

  .restart-banner {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 12px;
    padding: 10px 14px;
    border-radius: var(--radius-m);
    background: var(--accent-soft);
    font-weight: 600;

    span {
      flex: 1;
    }
  }
</style>
