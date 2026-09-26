<!-- Port of the Seelen UI "SeelenWeg" settings page, plus pinned app management. -->
<script lang="ts">
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import Icon from "@shared/components/Icon.svelte";
  import NumberInput from "@shared/components/NumberInput.svelte";
  import Segmented from "@shared/components/Segmented.svelte";
  import Select from "@shared/components/Select.svelte";
  import Switch from "@shared/components/Switch.svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api, iconUrl } from "@shared/ipc.ts";
  import type {
    DockMode,
    DockMonitors,
    DockSettings,
    DockSide,
    HideMode,
    MiddleClickAction,
    PinnedItemsVisibility,
    TemporalItemsVisibility,
  } from "@shared/types.ts";
  import SettingsGroup from "../components/SettingsGroup.svelte";
  import SettingsRow from "../components/SettingsRow.svelte";
  import { editor } from "../state/editor.svelte.ts";
  import { dockItemsActions, dockItemsState } from "../state/dockItems.svelte.ts";

  const dock = $derived(editor.value.dock);

  function set<K extends keyof DockSettings>(key: K, value: DockSettings[K]) {
    editor.update((s) => (s.dock[key] = value));
  }

  let importMessage = $state<string | null>(null);
  let importing = $state(false);

  async function importPinned() {
    if (importing) return;
    importing = true;
    try {
      const count = await api.importTaskbarPins();
      importMessage = t("dock.imported", { count });
    } catch (err) {
      importMessage = String(err);
    } finally {
      importing = false;
    }
  }

  async function addFile() {
    const files = await openDialog({ title: t("dock.addFile"), multiple: true, directory: false });
    const list = Array.isArray(files) ? files : files ? [files] : [];
    if (list.length) await api.pinPaths(list);
  }

  const sides: { value: DockSide; icon: string }[] = [
    { value: "Left", icon: "PanelLeft" },
    { value: "Right", icon: "PanelRight" },
    { value: "Top", icon: "PanelTop" },
    { value: "Bottom", icon: "PanelBottom" },
  ];
</script>

<div class="stack">
  <div class="description">{t("dock.description")}</div>

  <SettingsGroup>
    <SettingsRow label={t("dock.enable")} strong>
      <Switch checked={dock.enabled} onchange={(v) => set("enabled", v)} />
    </SettingsRow>
    <SettingsRow label={t("dock.reset")} strong>
      <button type="button" class="btn btn-icon" title={t("dock.reset")} onclick={() => editor.resetSection("dock")}>
        <Icon name="RotateCcw" size={16} />
      </button>
    </SettingsRow>
  </SettingsGroup>

  <SettingsGroup title={t("dock.group")}>
    <SettingsRow label={t("dock.width")}>
      <Select
        value={dock.mode}
        options={[
          { value: "FullWidth" as DockMode, label: t("dock.fullWidth") },
          { value: "MinContent" as DockMode, label: t("dock.minContent") },
        ]}
        onchange={(v) => set("mode", v)}
      />
    </SettingsRow>
    <SettingsRow label={t("dock.position")}>
      <Segmented
        value={dock.position}
        options={sides.map((s) => ({ value: s.value, icon: s.icon, title: s.value }))}
        onchange={(v) => set("position", v)}
      />
    </SettingsRow>
    <SettingsRow label={t("dock.margin")}>
      <NumberInput value={dock.margin} min={0} max={64} onchange={(v) => set("margin", v)} />
    </SettingsRow>
    <SettingsRow label={t("dock.padding")}>
      <NumberInput value={dock.padding} min={0} max={64} onchange={(v) => set("padding", v)} />
    </SettingsRow>
    <SettingsRow label={t("dock.monitors")}>
      <Select
        value={dock.monitors}
        options={[
          { value: "Primary" as DockMonitors, label: t("dock.monitorsPrimary") },
          { value: "All" as DockMonitors, label: t("dock.monitorsAll") },
        ]}
        onchange={(v) => set("monitors", v)}
      />
    </SettingsRow>
  </SettingsGroup>

  <SettingsGroup>
    {#snippet header()}
      <SettingsRow label={t("dock.autoHide")} strong>
        <Select
          value={dock.hideMode}
          options={[
            { value: "Never" as HideMode, label: t("common.never") },
            { value: "Always" as HideMode, label: t("common.always") },
            { value: "OnOverlap" as HideMode, label: t("common.onOverlap") },
          ]}
          onchange={(v) => set("hideMode", v)}
        />
      </SettingsRow>
    {/snippet}
    <SettingsRow label={t("dock.delayToShow")}>
      <NumberInput
        value={dock.delayToShow}
        max={10000}
        step={50}
        disabled={dock.hideMode === "Never"}
        onchange={(v) => set("delayToShow", v)}
      />
    </SettingsRow>
    <SettingsRow label={t("dock.delayToHide")}>
      <NumberInput
        value={dock.delayToHide}
        max={10000}
        step={50}
        disabled={dock.hideMode === "Never"}
        onchange={(v) => set("delayToHide", v)}
      />
    </SettingsRow>
    <SettingsRow label={t("dock.hideOnFullscreen")}>
      <Switch checked={dock.hideOnFullscreen} onchange={(v) => set("hideOnFullscreen", v)} />
    </SettingsRow>
  </SettingsGroup>

  <SettingsGroup title={t("dock.filter")}>
    <SettingsRow label={t("dock.temporalVisibility")}>
      <Select
        value={dock.temporalItemsVisibility}
        options={[
          { value: "All" as TemporalItemsVisibility, label: t("dock.temporalAll") },
          { value: "OnMonitor" as TemporalItemsVisibility, label: t("dock.temporalOnMonitor") },
        ]}
        onchange={(v) => set("temporalItemsVisibility", v)}
      />
    </SettingsRow>
    <SettingsRow label={t("dock.pinnedVisibility")}>
      <Select
        value={dock.pinnedItemsVisibility}
        options={[
          { value: "Always" as PinnedItemsVisibility, label: t("dock.pinnedAlways") },
          { value: "WhenPrimary" as PinnedItemsVisibility, label: t("dock.pinnedWhenPrimary") },
        ]}
        onchange={(v) => set("pinnedItemsVisibility", v)}
      />
    </SettingsRow>
  </SettingsGroup>

  <SettingsGroup title={t("dock.items")}>
    <SettingsRow label={t("dock.itemSize")}>
      <NumberInput value={dock.size} min={16} max={128} onchange={(v) => set("size", v)} />
    </SettingsRow>
    <SettingsRow label={t("dock.itemGap")}>
      <NumberInput value={dock.spaceBetweenItems} min={0} max={64} onchange={(v) => set("spaceBetweenItems", v)} />
    </SettingsRow>
    <SettingsRow label={t("dock.showTitle")}>
      <Switch checked={dock.showWindowTitle} onchange={(v) => set("showWindowTitle", v)} />
    </SettingsRow>
    <SettingsRow label={t("dock.showCounter")}>
      <Switch checked={dock.showInstanceCounter} onchange={(v) => set("showInstanceCounter", v)} />
    </SettingsRow>
    <SettingsRow label={t("dock.splitWindows")}>
      <Switch checked={dock.splitWindows} onchange={(v) => set("splitWindows", v)} />
    </SettingsRow>
    <SettingsRow label={t("dock.showEndTask")}>
      <Switch checked={dock.showEndTask} onchange={(v) => set("showEndTask", v)} />
    </SettingsRow>
  </SettingsGroup>

  <SettingsGroup>
    <SettingsRow label={t("dock.middleClick")} strong>
      <Select
        value={dock.middleClickAction}
        options={[
          { value: "CloseApp" as MiddleClickAction, label: t("dock.middleClose") },
          { value: "OpenNewInstance" as MiddleClickAction, label: t("dock.middleNew") },
        ]}
        onchange={(v) => set("middleClickAction", v)}
      />
    </SettingsRow>
  </SettingsGroup>

  <SettingsGroup>
    <SettingsRow label={t("dock.importPinned")} hint={importMessage ?? undefined}>
      <button type="button" class="btn btn-ghost" disabled={importing} onclick={importPinned}>
        {t("dock.importButton")}
      </button>
    </SettingsRow>
  </SettingsGroup>

  <SettingsGroup>
    {#snippet header()}
      <div class="pinned-head">
        <div>
          <h2 class="group-title-inline">{t("dock.pinned")}</h2>
          <p class="card-sub">{t("dock.pinnedHint")}</p>
        </div>
        <div class="pinned-actions">
          <button type="button" class="btn btn-ghost" onclick={() => dockItemsActions.reset()}>
            <Icon name="RotateCcw" size={14} />
            {t("dock.resetItems")}
          </button>
          <button type="button" class="btn btn-primary" onclick={addFile}>
            <Icon name="Plus" size={14} />
            {t("dock.addFile")}
          </button>
        </div>
      </div>
    {/snippet}
    {#each dockItemsState.pinnedApps as app (app.id)}
      <div class="pinned-row">
        <img src={iconUrl(app.relaunch?.command || app.path, app.umid)} alt="" />
        <div class="pinned-text">
          <span class="pinned-name">{app.displayName}</span>
          <span class="pinned-path">{app.path || app.umid}</span>
        </div>
        <button
          type="button"
          class="btn btn-icon danger"
          title={t("common.remove")}
          onclick={() => dockItemsActions.removeItem(app.id)}
        >
          <Icon name="X" size={16} />
        </button>
      </div>
    {:else}
      <p class="empty">{t("dock.noPinned")}</p>
    {/each}
  </SettingsGroup>
</div>

<style>
  .pinned-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  .group-title-inline {
    font-size: 16px;
    font-weight: 700;
  }

  .pinned-actions {
    display: flex;
    gap: 8px;
  }

  .pinned-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 4px;
    border-bottom: 1px solid var(--border);

    &:last-child {
      border-bottom: none;
    }

    img {
      width: 28px;
      height: 28px;
      object-fit: contain;
    }
  }

  .pinned-text {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .pinned-name {
    font-weight: 600;
  }

  .pinned-path {
    font-size: 12px;
    color: var(--fg-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .empty {
    padding: 12px 0;
    color: var(--fg-muted);
  }
</style>
