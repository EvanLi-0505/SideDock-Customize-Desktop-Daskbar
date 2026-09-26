<script lang="ts">
  import Icon from "@shared/components/Icon.svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api } from "@shared/ipc.ts";
  import type { AppInfo } from "@shared/types.ts";
  import SettingsGroup from "../components/SettingsGroup.svelte";
  import SettingsRow from "../components/SettingsRow.svelte";
  import StorageCards from "../components/StorageCards.svelte";

  let info = $state<AppInfo | null>(null);
  api.getAppInfo().then((i) => (info = i));
</script>

<div class="stack">
  <div class="description">{t("data.description")}</div>

  <SettingsGroup>
    <SettingsRow label={t("data.location")} hint={info?.dataDir}>
      <button type="button" class="btn btn-ghost" onclick={() => api.openDataDir("root")}>
        <Icon name="FolderOpen" size={14} />
        {t("common.open")}
      </button>
    </SettingsRow>
  </SettingsGroup>

  <StorageCards kinds={["logs", "userdata", "cache", "config"]} />
</div>
