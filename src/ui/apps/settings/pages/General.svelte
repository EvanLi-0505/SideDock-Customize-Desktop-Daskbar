<script lang="ts">
  import { ask } from "@tauri-apps/plugin-dialog";
  import Icon from "@shared/components/Icon.svelte";
  import Segmented from "@shared/components/Segmented.svelte";
  import Switch from "@shared/components/Switch.svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import type { Language } from "@shared/types.ts";
  import SettingsGroup from "../components/SettingsGroup.svelte";
  import SettingsRow from "../components/SettingsRow.svelte";
  import ThemePicker from "../components/ThemePicker.svelte";
  import { editor } from "../state/editor.svelte.ts";

  const presets = ["#3b82f6", "#8b5cf6", "#ec4899", "#ef4444", "#f59e0b", "#10b981", "#14b8a6", "#64748b"];

  async function resetAll() {
    if (await ask(t("data.confirmConfig"), { title: "SideDock", kind: "warning" })) {
      await editor.resetSection("all");
    }
  }
</script>

<div class="stack">
  <SettingsGroup title={t("general.appearance")}>
    <div class="theme-row"><ThemePicker /></div>
    <SettingsRow label={t("theme.accent")}>
      <div class="swatches">
        <button
          type="button"
          class="swatch system"
          class:active={!editor.value.accentColor}
          title={t("theme.accentSystem")}
          onclick={() => editor.update((s) => (s.accentColor = ""))}
        >
          <Icon name="Monitor" size={14} />
        </button>
        {#each presets as color (color)}
          <button
            type="button"
            class="swatch"
            class:active={editor.value.accentColor === color}
            style:background={color}
            aria-label={color}
            onclick={() => editor.update((s) => (s.accentColor = color))}
          ></button>
        {/each}
        <input
          type="color"
          class="swatch-input"
          value={editor.value.accentColor || "#3b82f6"}
          onchange={(e) => editor.update((s) => (s.accentColor = (e.currentTarget as HTMLInputElement).value))}
        />
      </div>
    </SettingsRow>
    <SettingsRow label={t("general.language")}>
      <Segmented
        value={editor.value.language}
        options={[
          { value: "zh-CN" as Language, label: "简体中文" },
          { value: "en" as Language, label: "English" },
        ]}
        onchange={(v) => editor.update((s) => (s.language = v))}
      />
    </SettingsRow>
  </SettingsGroup>

  <SettingsGroup title={t("general.startup")}>
    <SettingsRow label={t("general.autostart")} hint={t("home.autostartHint")}>
      <Switch checked={editor.value.autostart} onchange={(v) => editor.update((s) => (s.autostart = v))} />
    </SettingsRow>
    <SettingsRow label={t("general.crashRecovery")} hint={t("general.crashRecoveryHint")}>
      <Switch checked={editor.value.crashRecovery} onchange={(v) => editor.update((s) => (s.crashRecovery = v))} />
    </SettingsRow>
  </SettingsGroup>

  <SettingsGroup>
    <SettingsRow label={t("general.resetAll")} strong>
      <button type="button" class="btn btn-soft-danger" onclick={resetAll}>
        <Icon name="RotateCcw" size={14} />
        {t("common.reset")}
      </button>
    </SettingsRow>
  </SettingsGroup>
</div>

<style>
  .theme-row {
    padding: 8px 0 12px;
  }

  .swatches {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .swatch {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: 2px solid transparent;
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.12);
    transition: transform var(--dur-fast) ease;

    &:hover {
      transform: scale(1.12);
    }
    &.active {
      border-color: var(--fg);
    }
    &.system {
      display: flex;
      align-items: center;
      justify-content: center;
      background: var(--surface-2);
      color: var(--fg-secondary);
    }
  }

  .swatch-input {
    width: 28px;
    height: 28px;
    border: none;
    background: none;
    cursor: pointer;
  }
</style>
