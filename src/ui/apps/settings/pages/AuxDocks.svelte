<!-- Auxiliary docks: up to three extra docks of pinned items, each on an edge of its own. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import Icon from "@shared/components/Icon.svelte";
  import NumberInput from "@shared/components/NumberInput.svelte";
  import Segmented from "@shared/components/Segmented.svelte";
  import Select from "@shared/components/Select.svelte";
  import Slider from "@shared/components/Slider.svelte";
  import Switch from "@shared/components/Switch.svelte";
  import { t, type TranslationKey } from "@shared/i18n/index.svelte.ts";
  import { api, Events, iconUrl, on } from "@shared/ipc.ts";
  import { MODULES } from "@shared/modules.ts";
  import type {
    AppDockItem,
    AuxDockSettings,
    DockItem,
    DockItems,
    DockMode,
    DockSide,
    HideMode,
    ModuleId,
    OverflowMode,
  } from "@shared/types.ts";
  import SettingsGroup from "../components/SettingsGroup.svelte";
  import SettingsRow from "../components/SettingsRow.svelte";
  import { editor } from "../state/editor.svelte.ts";

  const SOURCE = "settings-aux";

  const docks = $derived(editor.value.auxDocks);
  const mainSide = $derived(editor.value.dock.position);

  function set<K extends keyof AuxDockSettings>(index: number, key: K, value: AuxDockSettings[K]) {
    editor.update((s) => (s.auxDocks[index][key] = value));
  }

  /** Moving onto another auxiliary dock's edge swaps the two (the main dock's edge is off limits). */
  function setSide(index: number, side: DockSide) {
    editor.update((s) => {
      const old = s.auxDocks[index].position;
      for (const other of s.auxDocks) {
        if (other.position === side) other.position = old;
      }
      s.auxDocks[index].position = side;
    });
  }

  const sides: { value: DockSide; icon: string }[] = [
    { value: "Left", icon: "PanelLeft" },
    { value: "Right", icon: "PanelRight" },
    { value: "Top", icon: "PanelTop" },
    { value: "Bottom", icon: "PanelBottom" },
  ];

  // ---------- pinned items, one store per dock ----------
  let items = $state<Record<string, DockItems>>({});

  function pinnedOf(id: string): AppDockItem[] {
    const list = items[id];
    if (!list) return [];
    return [...list.left, ...list.center, ...list.right].filter(
      (i): i is AppDockItem => i.type === "App" && i.pinned,
    );
  }

  onMount(() => {
    let unlisten: (() => void) | undefined;
    let gone = false;
    (async () => {
      const off = await on<{ source: string; store: string | null; items: DockItems }>(
        Events.DockItemsChanged,
        (p) => {
          if (p.store && p.source !== SOURCE) items[p.store] = p.items;
        },
      );
      if (gone) off();
      else unlisten = off;
      for (const dock of editor.value.auxDocks) {
        items[dock.id] = await api.getDockItems(dock.id);
      }
    })();
    return () => {
      gone = true;
      unlisten?.();
    };
  });

  async function addFiles(id: string) {
    const files = await openDialog({ title: t("dock.addFile"), multiple: true, directory: false });
    const list = Array.isArray(files) ? files : files ? [files] : [];
    if (list.length) await api.pinPaths(list, id);
  }

  function hasModule(id: string, module: ModuleId): boolean {
    const list = items[id];
    if (!list) return false;
    return [...list.left, ...list.center, ...list.right].some((i) => i.type === "Module" && i.module === module);
  }

  async function toggleModule(id: string, module: ModuleId, enabled: boolean) {
    const list = items[id];
    if (!list) return;
    const other = (l: DockItem[]) => l.filter((i) => !(i.type === "Module" && i.module === module));
    const next = { ...list, left: other(list.left), center: other(list.center), right: other(list.right) };
    if (enabled) next.right = [...next.right, { id: `module-${module}`, type: "Module", module }];
    items[id] = next;
    items[id] = await api.saveDockItems(next, SOURCE, id);
  }

  async function removeItem(id: string, itemId: string) {
    const list = items[id];
    if (!list) return;
    const keep = (l: DockItem[]) => l.filter((i) => i.id !== itemId);
    const next = { ...list, left: keep(list.left), center: keep(list.center), right: keep(list.right) };
    items[id] = next;
    items[id] = await api.saveDockItems(next, SOURCE, id);
  }
</script>

<div class="stack">
  <div class="description">{t("aux.description")}</div>
  <p class="note">{t("aux.layoutHint")}</p>

  {#each docks as dock, i (dock.id)}
    <SettingsGroup>
      {#snippet header()}
        <SettingsRow label={t("aux.title", { n: i + 1 })} strong>
          <Switch checked={dock.enabled} onchange={(v) => set(i, "enabled", v)} />
        </SettingsRow>
      {/snippet}
      {#if dock.enabled}
        <SettingsRow label={t("dock.position")} hint={t("aux.positionHint")}>
          <Segmented
            value={dock.position}
            options={sides.map((s) => ({
              value: s.value,
              icon: s.icon,
              title: s.value === mainSide ? t("aux.mainEdge") : s.value,
              disabled: s.value === mainSide,
            }))}
            onchange={(v) => setSide(i, v)}
          />
        </SettingsRow>
        <SettingsRow label={t("dock.width")}>
          <Select
            value={dock.mode}
            options={[
              { value: "FullWidth" as DockMode, label: t("dock.fullWidth") },
              { value: "MinContent" as DockMode, label: t("dock.minContent") },
            ]}
            onchange={(v) => set(i, "mode", v)}
          />
        </SettingsRow>
        <SettingsRow label={t("dock.itemSize")}>
          <NumberInput value={dock.size} min={16} max={128} onchange={(v) => set(i, "size", v)} />
        </SettingsRow>
        <SettingsRow label={t("dock.autoHide")} hint={dock.hideMode === "Never" ? t("dock.neverHint") : undefined}>
          <Select
            value={dock.hideMode}
            options={[
              { value: "Never" as HideMode, label: t("common.never") },
              { value: "Always" as HideMode, label: t("common.always") },
              { value: "OnOverlap" as HideMode, label: t("common.onOverlap") },
            ]}
            onchange={(v) => set(i, "hideMode", v)}
          />
        </SettingsRow>
        <SettingsRow label={t("dock.delayToShow")}>
          <NumberInput
            value={dock.delayToShow}
            max={10000}
            step={50}
            disabled={dock.hideMode === "Never"}
            onchange={(v) => set(i, "delayToShow", v)}
          />
        </SettingsRow>
        <SettingsRow label={t("dock.delayToHide")}>
          <NumberInput
            value={dock.delayToHide}
            max={10000}
            step={50}
            disabled={dock.hideMode === "Never"}
            onchange={(v) => set(i, "delayToHide", v)}
          />
        </SettingsRow>

        <SettingsRow label={t("dock.magnify")} strong>
          <Switch checked={dock.magnification} onchange={(v) => set(i, "magnification", v)} />
        </SettingsRow>
        <div class="nested">
          <SettingsRow label={t("dock.magnifyScale")}>
            <Slider
              value={dock.magnificationScale}
              min={1.2}
              max={2.5}
              step={0.1}
              disabled={!dock.magnification}
              format={(v) => `${v.toFixed(1)}×`}
              onchange={(v) => set(i, "magnificationScale", v)}
            />
          </SettingsRow>
          <SettingsRow label={t("dock.magnifyRange")}>
            <Slider
              value={dock.magnificationRange}
              min={1}
              max={6}
              step={1}
              disabled={!dock.magnification}
              onchange={(v) => set(i, "magnificationRange", v)}
            />
          </SettingsRow>
          <SettingsRow label={t("dock.showLabels")}>
            <Switch
              checked={dock.showLabels}
              disabled={!dock.magnification}
              onchange={(v) => set(i, "showLabels", v)}
            />
          </SettingsRow>
        </div>

        <SettingsRow label={t("dock.overflow")} hint={t("dock.overflowHint")} strong>
          <Select
            value={dock.overflowMode}
            options={[
              { value: "ShrinkThenStack" as OverflowMode, label: t("dock.overflowStack") },
              { value: "ShrinkOnly" as OverflowMode, label: t("dock.overflowShrink") },
              { value: "Collapse" as OverflowMode, label: t("dock.overflowCollapse") },
            ]}
            onchange={(v) => set(i, "overflowMode", v)}
          />
        </SettingsRow>

        <div class="pinned-head">
          <span class="pinned-title">{t("aux.modules")}</span>
        </div>
        <div class="modules">
          {#each MODULES as module (module.id)}
            {@const active = hasModule(dock.id, module.id)}
            <button
              type="button"
              class="module"
              class:on={active}
              aria-pressed={active}
              onclick={() => toggleModule(dock.id, module.id, !active)}
            >
              <Icon name={module.icon} size={15} />
              {t(`module.${module.id}` as TranslationKey)}
            </button>
          {/each}
        </div>

        <div class="pinned-head">
          <span class="pinned-title">{t("aux.items")}</span>
          <button type="button" class="btn btn-ghost" onclick={() => addFiles(dock.id)}>
            <Icon name="Plus" size={14} />
            {t("dock.addFile")}
          </button>
        </div>
        {#each pinnedOf(dock.id) as app (app.id)}
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
              onclick={() => removeItem(dock.id, app.id)}
            >
              <Icon name="X" size={16} />
            </button>
          </div>
        {:else}
          <p class="empty">{t("aux.noItems")}</p>
        {/each}
      {/if}
    </SettingsGroup>
  {/each}
</div>

<style>
  .note {
    font-size: 12px;
    color: var(--fg-muted);
    line-height: 1.6;
  }

  .pinned-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding-top: 10px;
  }

  .pinned-title {
    font-weight: 600;
  }

  .nested {
    margin-left: 4px;
    padding-left: 16px;
    border-left: 2px solid var(--border);
  }

  .modules {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 8px 0 4px;
  }

  .module {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 10px;
    border-radius: 999px;
    border: 1px solid var(--border-strong);
    background: var(--surface-2);
    color: var(--fg-secondary);
    font-size: 12.5px;
    cursor: pointer;

    &.on {
      border-color: color-mix(in srgb, var(--accent) 55%, transparent);
      background: var(--accent-soft);
      color: var(--accent);
    }
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
    padding: 8px 0;
    color: var(--fg-muted);
  }
</style>
