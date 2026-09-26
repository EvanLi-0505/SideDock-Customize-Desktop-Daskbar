<script lang="ts">
  import Icon from "@shared/components/Icon.svelte";
  import Segmented from "@shared/components/Segmented.svelte";
  import Switch from "@shared/components/Switch.svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api } from "@shared/ipc.ts";
  import type { AppInfo, DockSide, Language } from "@shared/types.ts";
  import SettingsRow from "../components/SettingsRow.svelte";
  import StorageCards from "../components/StorageCards.svelte";
  import ThemePicker from "../components/ThemePicker.svelte";
  import { editor } from "../state/editor.svelte.ts";
  import type { PageProps } from "../pages.ts";
  import logo from "../../../../static/icons/logo.svg";

  let { navigate }: PageProps = $props();

  let info = $state<AppInfo | null>(null);
  api.getAppInfo().then((i) => (info = i));

  const dock = $derived(editor.value.dock);
  const sides: { value: DockSide; icon: string }[] = [
    { value: "Left", icon: "PanelLeft" },
    { value: "Right", icon: "PanelRight" },
    { value: "Top", icon: "PanelTop" },
    { value: "Bottom", icon: "PanelBottom" },
  ];
</script>

<div class="stack">
  {#if info?.previousSessionCrashed}
    <div class="notice">
      <Icon name="CircleAlert" size={18} />
      <span>{t("home.crashNotice")}</span>
      <button type="button" class="btn btn-ghost" onclick={() => api.openDataDir("logs")}>{t("data.logs")}</button>
    </div>
  {/if}

  <section class="hero">
    <img class="hero-logo" src={logo} alt="" />
    <div class="hero-text">
      <h2>{t("home.welcome")}</h2>
      <p>{t("home.subtitle")}</p>
    </div>
    <div class="hero-status">
      <span class="status-dot" class:on={dock.enabled}></span>
      <span>{t("home.dockStatus")} · {dock.enabled ? t("home.dockOn") : t("home.dockOff")}</span>
      <Switch
        checked={dock.enabled}
        label={t("dock.enable")}
        onchange={(v) => editor.update((s) => (s.dock.enabled = v))}
      />
    </div>
  </section>

  <section class="card">
    <h3 class="card-title">{t("home.theme")}</h3>
    <ThemePicker />
  </section>

  <section class="card">
    <h3 class="card-title">{t("home.quick")}</h3>
    <SettingsRow label={t("home.language")}>
      <Segmented
        value={editor.value.language}
        options={[
          { value: "zh-CN" as Language, label: "中文" },
          { value: "en" as Language, label: "English" },
        ]}
        onchange={(v) => editor.update((s) => (s.language = v))}
      />
    </SettingsRow>
    <SettingsRow label={t("home.autostart")} hint={t("home.autostartHint")}>
      <Switch checked={editor.value.autostart} onchange={(v) => editor.update((s) => (s.autostart = v))} />
    </SettingsRow>
    <SettingsRow label={t("home.position")}>
      <Segmented
        value={dock.position}
        options={sides.map((s) => ({ value: s.value, icon: s.icon, title: s.value }))}
        onchange={(v) => editor.update((s) => (s.dock.position = v))}
      />
      <button type="button" class="btn btn-ghost" onclick={() => navigate("dock")}>
        {t("home.editDock")}
        <Icon name="ChevronRight" size={14} />
      </button>
    </SettingsRow>
  </section>

  <section class="card">
    <div class="card-head">
      <div>
        <h3 class="card-title">{t("home.storage")}</h3>
        <p class="card-sub">{t("home.storageHint")}</p>
      </div>
      <button type="button" class="btn btn-ghost" onclick={() => api.openDataDir("root")}>
        <Icon name="ExternalLink" size={14} />
        {t("home.openDataFolder")}
      </button>
    </div>
    <StorageCards />
  </section>
</div>

<style>
  .hero {
    display: flex;
    align-items: center;
    gap: 20px;
    padding: 22px 24px;
    border-radius: var(--radius-xl);
    background:
      radial-gradient(circle at 0% 0%, color-mix(in srgb, var(--accent) 22%, transparent), transparent 55%),
      var(--card-bg);
    border: 1px solid var(--border);
  }

  .hero-logo {
    width: 64px;
    height: 64px;
    filter: drop-shadow(0 6px 14px rgb(91 92 255 / 0.35));
  }

  .hero-text {
    flex: 1;

    h2 {
      font-family: var(--font-display);
      font-size: 24px;
      font-weight: 700;
    }
    p {
      color: var(--fg-secondary);
    }
  }

  .hero-status {
    display: flex;
    align-items: center;
    gap: 10px;
    font-weight: 600;
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--fg-disabled);

    &.on {
      background: var(--success);
      box-shadow: 0 0 0 4px color-mix(in srgb, var(--success) 25%, transparent);
    }
  }

  .notice {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 16px;
    border-radius: var(--radius-m);
    background: color-mix(in srgb, #f5a524 18%, transparent);
    border: 1px solid color-mix(in srgb, #f5a524 45%, transparent);
    font-weight: 600;

    span {
      flex: 1;
    }
  }
</style>
