<!-- Configuration backups: where they go, making one, restoring or deleting one. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { ask, open as openDialog } from "@tauri-apps/plugin-dialog";
  import Icon from "@shared/components/Icon.svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api } from "@shared/ipc.ts";
  import type { BackupEntry, BackupInfo } from "@shared/types.ts";
  import { formatBytes } from "@shared/utils.ts";

  let info = $state<BackupInfo | null>(null);
  /** backups listed before whose file is gone (deleted or moved by hand) */
  let gone = $state<BackupEntry[]>([]);
  let busy = $state(false);
  let message = $state<string | null>(null);
  let failed = $state(false);

  /** Reads the backup folder again; backups that vanished stay listed as no longer valid. */
  async function refresh() {
    const next = await api.backupInfo();
    if (info && info.dir === next.dir) {
      const present = new Set(next.entries.map((e) => e.path));
      const known = [...info.entries, ...gone];
      const nextGone = known.filter((e, i) => !present.has(e.path) && known.findIndex((x) => x.path === e.path) === i);
      // "backed up: ..." no longer holds once that file is gone
      if (nextGone.length > gone.length && !failed) message = null;
      gone = nextGone;
    } else {
      gone = [];
    }
    info = next;
  }

  // the folder can change behind our back (Explorer): look again now and then, and when
  // the settings window comes back to the front
  onMount(() => {
    refresh();
    const timer = setInterval(() => {
      if (!busy && document.visibilityState === "visible") refresh().catch(() => {});
    }, 4000);
    const onFocus = () => refresh().catch(() => {});
    window.addEventListener("focus", onFocus);
    return () => {
      clearInterval(timer);
      window.removeEventListener("focus", onFocus);
    };
  });

  function describe(err: unknown): string {
    const text = String(err);
    return text.includes("backup-missing") ? t("backup.missing") : text;
  }

  async function run(task: () => Promise<void>) {
    if (busy) return;
    busy = true;
    message = null;
    failed = false;
    try {
      await task();
    } catch (err) {
      message = describe(err);
      failed = true;
      // a file that vanished shows up as such
      await refresh().catch(() => {});
    } finally {
      busy = false;
    }
  }

  const backupNow = () =>
    run(async () => {
      const entry = await api.backupCreate();
      message = t("backup.created", { name: entry.name });
      await refresh();
    });

  const chooseDir = () =>
    run(async () => {
      const dir = await openDialog({ title: t("backup.chooseDir"), directory: true, multiple: false });
      if (typeof dir !== "string") return;
      gone = [];
      info = await api.backupSetDir(dir);
    });

  const defaultDir = () =>
    run(async () => {
      gone = [];
      info = await api.backupSetDir(null);
    });

  async function restore(path: string) {
    if (!(await ask(t("backup.confirmRestore"), { title: "SideDock", kind: "warning" }))) return;
    await run(async () => {
      await api.backupRestore(path);
      message = t("backup.restarting");
    });
  }

  const restoreFromFile = () =>
    run(async () => {
      const file = await openDialog({
        title: t("backup.restoreFile"),
        multiple: false,
        directory: false,
        filters: [{ name: "SideDock", extensions: ["json"] }],
      });
      if (typeof file !== "string") return;
      busy = false;
      await restore(file);
    });

  async function remove(entry: BackupEntry) {
    if (!(await ask(t("backup.confirmDelete", { name: entry.name }), { title: "SideDock", kind: "warning" }))) return;
    await run(async () => {
      await api.backupDelete(entry.path);
      await refresh();
    });
  }

  /** a backup that is gone: only its entry is left to remove */
  function forget(entry: BackupEntry) {
    gone = gone.filter((e) => e.path !== entry.path);
  }
</script>

<section class="backup">
  <div class="head">
    <span class="head-icon"><Icon name="DatabaseBackup" size={18} /></span>
    <div class="head-text">
      <span class="title">{t("backup.title")}</span>
      <span class="hint">{t("backup.hint")}</span>
    </div>
    <div class="head-actions">
      <button type="button" class="btn btn-ghost" disabled={busy} onclick={restoreFromFile}>
        <Icon name="ArchiveRestore" size={14} />
        {t("backup.restoreFile")}
      </button>
      <button type="button" class="btn btn-primary" disabled={busy} onclick={backupNow}>
        <Icon name="DatabaseBackup" size={14} />
        {t("backup.now")}
      </button>
    </div>
  </div>

  <div class="location">
    <div class="location-text">
      <span class="label">{t("backup.location")}{info?.custom ? "" : ` · ${t("backup.defaultLocation")}`}</span>
      <span class="path">{info?.dir ?? "…"}</span>
    </div>
    <div class="location-actions">
      <button
        type="button"
        class="btn btn-ghost"
        disabled={busy || !info?.dirExists}
        onclick={() => api.backupOpenDir().catch((e) => ((message = describe(e)), (failed = true)))}
      >
        <Icon name="FolderOpen" size={14} />
        {t("common.open")}
      </button>
      <button type="button" class="btn btn-ghost" disabled={busy} onclick={chooseDir}>
        <Icon name="FolderCog" size={14} />
        {t("backup.change")}
      </button>
      {#if info?.custom}
        <button type="button" class="btn btn-ghost" disabled={busy} title={info.defaultDir} onclick={defaultDir}>
          <Icon name="RotateCcw" size={14} />
          {t("backup.useDefault")}
        </button>
      {/if}
    </div>
  </div>

  {#if info && !info.dirExists && gone.length}
    <p class="message failed">{t("backup.dirGone")}</p>
  {/if}
  {#if message}
    <p class="message" class:failed>{message}</p>
  {/if}

  <div class="list">
    {#each info?.entries ?? [] as entry (entry.path)}
      <div class="entry">
        <Icon name="FileText" size={16} />
        <div class="entry-text">
          <span class="entry-name">
            {entry.created}
            {#if entry.automatic}<span class="tag">{t("backup.automatic")}</span>{/if}
          </span>
          <span class="entry-sub">{entry.name} · {formatBytes(entry.size)} · v{entry.appVersion}</span>
        </div>
        <button type="button" class="btn btn-ghost" disabled={busy} onclick={() => restore(entry.path)}>
          <Icon name="ArchiveRestore" size={14} />
          {t("backup.restore")}
        </button>
        <button
          type="button"
          class="btn btn-icon danger"
          disabled={busy}
          title={t("common.remove")}
          onclick={() => remove(entry)}
        >
          <Icon name="Trash2" size={15} />
        </button>
      </div>
    {/each}
    {#each gone as entry (entry.path)}
      <div class="entry gone">
        <Icon name="CircleAlert" size={16} />
        <div class="entry-text">
          <span class="entry-name">
            {entry.created}
            <span class="tag tag-gone">{t("backup.gone")}</span>
          </span>
          <span class="entry-sub">{entry.name} · {t("backup.goneHint")}</span>
        </div>
        <button type="button" class="btn btn-ghost" onclick={() => forget(entry)}>
          <Icon name="X" size={14} />
          {t("backup.removeRecord")}
        </button>
      </div>
    {/each}
    {#if !info?.entries.length && !gone.length}
      <p class="empty">{t("backup.none")}</p>
    {/if}
  </div>
</section>

<style>
  .backup {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 14px 18px;
    border-radius: var(--radius-l);
    background: var(--card-bg);
    border: 1px solid var(--border);
  }

  .head {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .head-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    border-radius: 10px;
    background: var(--accent-soft);
    color: var(--accent);
  }

  .head-text {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .title {
    font-weight: 700;
  }

  .hint {
    font-size: 12.5px;
    color: var(--fg-secondary);
  }

  .head-actions,
  .location-actions {
    display: flex;
    gap: 8px;
    flex-shrink: 0;
  }

  .location {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border-radius: var(--radius-m);
    background: var(--surface-2);
  }

  .location-text {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .label {
    font-size: 12.5px;
    font-weight: 600;
  }

  .path {
    font-size: 12.5px;
    color: var(--fg-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .message {
    font-size: 12.5px;
    color: var(--fg-secondary);

    &.failed {
      color: var(--danger, #d64545);
    }
  }

  .list {
    display: flex;
    flex-direction: column;
  }

  .entry {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 4px;
    border-bottom: 1px solid var(--border);
    color: var(--fg-secondary);

    &:last-child {
      border-bottom: none;
    }
  }

  .entry-text {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .entry-name {
    font-weight: 600;
    color: var(--fg);
  }

  .entry-sub {
    font-size: 12px;
    color: var(--fg-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tag {
    margin-left: 6px;
    padding: 1px 6px;
    border-radius: 6px;
    font-size: 11px;
    font-weight: 600;
    background: var(--surface-2);
    color: var(--fg-secondary);
  }

  .entry.gone {
    opacity: 0.85;

    .entry-name {
      color: var(--fg-secondary);
    }
  }

  .tag-gone {
    background: color-mix(in srgb, var(--danger, #d64545) 16%, transparent);
    color: var(--danger, #d64545);
  }

  .empty {
    padding: 6px 0;
    font-size: 12.5px;
    color: var(--fg-muted);
  }
</style>
