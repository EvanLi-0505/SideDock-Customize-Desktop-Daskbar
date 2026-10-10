<!-- Toolbar (fancy toolbar with the system tray) and native taskbar settings. -->
<script lang="ts">
  import NumberInput from "@shared/components/NumberInput.svelte";
  import Segmented from "@shared/components/Segmented.svelte";
  import Select from "@shared/components/Select.svelte";
  import Slider from "@shared/components/Slider.svelte";
  import Switch from "@shared/components/Switch.svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api } from "@shared/ipc.ts";
  import type { DockMonitors, HideMode, StartMenuMode, ToolbarPosition, ToolbarSettings } from "@shared/types.ts";
  import SettingsGroup from "../components/SettingsGroup.svelte";
  import SettingsRow from "../components/SettingsRow.svelte";
  import ThemePicker from "../components/ThemePicker.svelte";
  import ToolbarItemsEditor from "../components/ToolbarItemsEditor.svelte";
  import TrayIconsList from "../components/TrayIconsList.svelte";
  import { editor } from "../state/editor.svelte.ts";

  const toolbar = $derived(editor.value.toolbar);
  const hideNative = $derived(editor.value.taskbar.hideNative);
  /** at the bottom with the taskbar hidden, it takes the taskbar's place */
  const inSlot = $derived(toolbar.position === "Bottom" && hideNative);
  const theme = $derived(toolbar.theme ?? editor.value.theme);
  const solid = $derived(theme !== "glass" && theme !== "clear");
  const hasTray = $derived(
    [...toolbar.items.left, ...toolbar.items.center, ...toolbar.items.right].includes("tray"),
  );
  let trayError = $state("");

  function set<K extends keyof ToolbarSettings>(key: K, value: ToolbarSettings[K]) {
    editor.update((s) => (s.toolbar[key] = value));
  }

  async function useWindowsPlacement() {
    try {
      trayError = "";
      await api.trayUseWindowsPlacement();
    } catch (err) {
      trayError = String(err);
    }
  }
</script>

<div class="stack">
  <SettingsGroup>
    {#snippet header()}
      <SettingsRow label={t("toolbar.enable")} hint={t("toolbar.enableHint")}>
        <Switch checked={toolbar.enabled} onchange={(v) => set("enabled", v)} />
      </SettingsRow>
    {/snippet}
    <SettingsRow label={t("toolbar.position")} hint={inSlot ? t("toolbar.replacesTaskbar") : t("toolbar.floatingHint")}>
      <Segmented
        value={toolbar.position}
        options={[
          { value: "Top" as ToolbarPosition, icon: "PanelTop", label: t("toolbar.top") },
          { value: "Bottom" as ToolbarPosition, icon: "PanelBottom", label: t("toolbar.bottom") },
        ]}
        onchange={(v) => set("position", v)}
      />
    </SettingsRow>
    <SettingsRow label={t("dock.monitors")}>
      <Select
        value={toolbar.monitors}
        options={[
          { value: "Primary" as DockMonitors, label: t("dock.monitorsPrimary") },
          { value: "All" as DockMonitors, label: t("dock.monitorsAll") },
        ]}
        onchange={(v) => set("monitors", v)}
      />
    </SettingsRow>
    <SettingsRow label={t("dock.autoHide")} hint={inSlot ? t("toolbar.fixedInSlot") : t("toolbar.hideModeHint")}>
      <Select
        value={toolbar.hideMode}
        disabled={inSlot}
        options={[
          { value: "Never" as HideMode, label: t("toolbar.hideNever") },
          { value: "OnOverlap" as HideMode, label: t("common.onOverlap") },
          { value: "Always" as HideMode, label: t("common.always") },
        ]}
        onchange={(v) => set("hideMode", v)}
      />
    </SettingsRow>
    {#if !inSlot && toolbar.hideMode !== "Never"}
      <SettingsRow label={t("dock.delayToShow")}>
        <NumberInput value={toolbar.delayToShow} max={10000} step={50} onchange={(v) => set("delayToShow", v)} />
      </SettingsRow>
      <SettingsRow label={t("dock.delayToHide")}>
        <NumberInput value={toolbar.delayToHide} max={10000} step={50} onchange={(v) => set("delayToHide", v)} />
      </SettingsRow>
    {/if}
  </SettingsGroup>

  <SettingsGroup title={t("toolbar.size")}>
    {#if inSlot}
      <SettingsRow label={t("toolbar.matchTaskbar")} hint={t("toolbar.matchTaskbarHint")}>
        <Switch checked={toolbar.matchTaskbarHeight} onchange={(v) => set("matchTaskbarHeight", v)} />
      </SettingsRow>
    {/if}
    <SettingsRow label={t("toolbar.height")}>
      <NumberInput
        value={toolbar.height}
        min={28}
        max={72}
        disabled={inSlot && toolbar.matchTaskbarHeight}
        onchange={(v) => set("height", v)}
      />
    </SettingsRow>
    <SettingsRow label={t("toolbar.scale")} hint={t("toolbar.scaleHint")}>
      <Slider
        value={toolbar.scale}
        min={80}
        max={140}
        step={5}
        format={(v) => `${v}%`}
        onchange={(v) => set("scale", v)}
      />
    </SettingsRow>
  </SettingsGroup>

  <SettingsGroup title={t("toolbar.theme")}>
    <ThemePicker
      value={toolbar.theme}
      inheritLabel={t("toolbar.themeInherit")}
      onchange={(v) => set("theme", v)}
    />
    {#if solid}
      <SettingsRow label={t("toolbar.opacity")}>
        <Slider
          value={toolbar.backgroundOpacity}
          min={0}
          max={100}
          step={1}
          format={(v) => `${v}%`}
          onchange={(v) => set("backgroundOpacity", v)}
        />
      </SettingsRow>
    {/if}
  </SettingsGroup>

  <SettingsGroup title={t("toolbar.items")}>
    <p class="hint">{t("toolbar.itemsHint")}</p>
    <ToolbarItemsEditor />
    <SettingsRow label={t("toolbar.showSeconds")}>
      <Switch checked={toolbar.showSeconds} onchange={(v) => set("showSeconds", v)} />
    </SettingsRow>
  </SettingsGroup>

  {#if hasTray}
    <SettingsGroup title={t("toolbar.tray")}>
      <p class="hint">{t("toolbar.trayHint")}</p>
      <SettingsRow label={t("toolbar.newIconsInline")}>
        <Switch
          checked={toolbar.tray.newIconsInline}
          onchange={(v) => editor.update((s) => (s.toolbar.tray.newIconsInline = v))}
        />
      </SettingsRow>
      <div class="tray-actions">
        <button
          type="button"
          class="btn"
          onclick={() =>
            editor.update((s) => {
              s.toolbar.tray.inline = [];
              s.toolbar.tray.overflow = [];
              s.toolbar.tray.newIconsInline = true;
            })}
        >
          {t("toolbar.trayAllInline")}
        </button>
        <button type="button" class="btn" onclick={useWindowsPlacement}>{t("toolbar.trayLikeWindows")}</button>
      </div>
      {#if trayError}<p class="error">{trayError}</p>{/if}
      <TrayIconsList />
    </SettingsGroup>
  {/if}

  <SettingsGroup title={t("taskbar.title")}>
    <SettingsRow label={t("taskbar.hide")} hint={t("taskbar.hideHint")}>
      <Switch checked={hideNative} onchange={(v) => editor.update((s) => (s.taskbar.hideNative = v))} />
    </SettingsRow>
    {#if hideNative && !toolbar.enabled}
      <p class="warning">{t("taskbar.noToolbarWarning")}</p>
    {/if}
    <SettingsRow label={t("taskbar.startMenu")} hint={t("taskbar.startMenuHint")}>
      <Segmented
        value={editor.value.launcher.startMenu}
        options={[
          { value: "Native" as StartMenuMode, label: t("launcherPage.native") },
          { value: "SideDock" as StartMenuMode, label: t("launcherPage.sidedock") },
        ]}
        onchange={(v) => editor.update((s) => (s.launcher.startMenu = v))}
      />
    </SettingsRow>
  </SettingsGroup>
</div>

<style>
  .hint {
    margin: 2px 0 10px;
    color: var(--fg-muted);
    font-size: 12.5px;
    line-height: 1.5;
  }

  .tray-actions {
    display: flex;
    gap: 8px;
    margin: 6px 0 10px;
  }

  .warning {
    margin: 4px 0;
    padding: 10px 14px;
    border-radius: var(--radius-m);
    background: color-mix(in srgb, #f5a524 16%, transparent);
    border: 1px solid color-mix(in srgb, #f5a524 40%, transparent);
    font-size: 12.5px;
  }

  .error {
    color: var(--danger);
    font-size: 12.5px;
  }
</style>
