<script lang="ts">
  import Icon from "@shared/components/Icon.svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api } from "@shared/ipc.ts";
  import type { AppInfo } from "@shared/types.ts";
  import SettingsGroup from "../components/SettingsGroup.svelte";
  import SettingsRow from "../components/SettingsRow.svelte";
  import logo from "../../../../static/icons/logo.svg";

  let info = $state<AppInfo | null>(null);
  api.getAppInfo().then((i) => (info = i));
</script>

<div class="stack">
  <section class="about">
    <img src={logo} alt="" />
    <div>
      <h2>SideDock</h2>
      <p>{t("app.tagline")}</p>
      <p class="version">
        {t("about.version")} {info?.version ?? ""}{info?.debug ? " (debug)" : ""}
      </p>
    </div>
  </section>

  <SettingsGroup title={t("about.license")}>
    <p class="text">{t("about.licenseText")}</p>
    <p class="text muted">Seelen UI — https://github.com/eythaann/Seelen-UI (AGPL-3.0)</p>
  </SettingsGroup>

  <SettingsGroup>
    <SettingsRow label={t("about.restart")} strong>
      <button type="button" class="btn btn-ghost" onclick={() => api.restartApp()}>
        <Icon name="RefreshCw" size={14} />
        {t("about.restart")}
      </button>
    </SettingsRow>
    <SettingsRow label={t("about.quit")} strong>
      <button type="button" class="btn btn-soft-danger" onclick={() => api.quitApp()}>
        <Icon name="Power" size={14} />
        {t("about.quit")}
      </button>
    </SettingsRow>
  </SettingsGroup>
</div>

<style>
  .about {
    display: flex;
    align-items: center;
    gap: 20px;
    padding: 24px;
    border-radius: var(--radius-xl);
    background: var(--card-bg);
    border: 1px solid var(--border);

    img {
      width: 72px;
      height: 72px;
    }
    h2 {
      font-family: var(--font-display);
      font-size: 26px;
    }
    p {
      color: var(--fg-secondary);
    }
  }

  .version {
    font-size: 12.5px;
    font-variant-numeric: tabular-nums;
  }

  .text {
    padding: 4px 0;
    line-height: 1.6;
  }

  .muted {
    color: var(--fg-muted);
    font-size: 12.5px;
    user-select: text;
  }
</style>
