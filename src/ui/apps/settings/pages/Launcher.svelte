<!-- Start menu (SideDock launcher) and window switcher (stage manager) settings. -->
<script lang="ts">
  import Icon from "@shared/components/Icon.svelte";
  import Segmented from "@shared/components/Segmented.svelte";
  import Slider from "@shared/components/Slider.svelte";
  import Switch from "@shared/components/Switch.svelte";
  import { onMount } from "svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api, Events, on } from "@shared/ipc.ts";
  import { acceleratorKeys } from "@shared/shortcuts.ts";
  import type { StartMenuMode, WinKeyState } from "@shared/types.ts";
  import SettingsGroup from "../components/SettingsGroup.svelte";
  import SettingsRow from "../components/SettingsRow.svelte";
  import { editor } from "../state/editor.svelte.ts";
  import type { PageProps } from "../pages.ts";

  let { navigate }: PageProps = $props();

  const launcher = $derived(editor.value.launcher);
  const sidedock = $derived(launcher.startMenu === "SideDock");
  const keys = (id: string) => acceleratorKeys(editor.value.shortcuts[id] ?? "");

  // ---------------- Win key takeover ----------------
  // Turning it on asks for administrator rights once (UAC) to register the scheduled
  // task that starts the elevated helper; turning it off removes the task.
  let winKey = $state<WinKeyState>({ status: "off", taskRegistered: false });
  let winKeyBusy = $state(false);
  let winKeyError = $state("");

  onMount(() => {
    api.winKeyState().then((s) => (winKey = s));
    const unlisten = on<WinKeyState>(Events.WinKeyStatus, (s) => (winKey = s));
    return () => {
      unlisten.then((f) => f());
    };
  });

  async function setWinKey(enabled: boolean) {
    winKeyBusy = true;
    winKeyError = "";
    try {
      winKey = await api.winKeySetEnabled(enabled);
    } catch (err) {
      winKeyError = String(err);
    } finally {
      winKeyBusy = false;
    }
  }
</script>

{#snippet shortcut(id: string)}
  <div class="shortcut">
    {#each keys(id) as key (key)}
      <kbd>{key}</kbd>
    {:else}
      <span class="muted">{t("shortcuts.none")}</span>
    {/each}
    <button type="button" class="btn btn-ghost" onclick={() => navigate("shortcuts")}>
      {t("launcherPage.editShortcuts")}
    </button>
  </div>
{/snippet}

<div class="stack">
  <SettingsGroup title={t("launcherPage.startMenu")}>
    <SettingsRow label={t("launcherPage.startButton")}>
      <Segmented
        value={launcher.startMenu}
        options={[
          { value: "Native" as StartMenuMode, label: t("launcherPage.native") },
          { value: "SideDock" as StartMenuMode, label: t("launcherPage.sidedock") },
        ]}
        onchange={(v) => editor.update((s) => (s.launcher.startMenu = v))}
      />
    </SettingsRow>
    <SettingsRow label={t("launcherPage.winKey")} hint={t("launcherPage.winKeyHint")}>
      <Switch
        checked={launcher.takeOverWinKey && sidedock}
        disabled={!sidedock || winKeyBusy}
        onchange={(v) => setWinKey(v)}
      />
    </SettingsRow>
    {#if winKeyBusy || winKeyError || winKey.status !== "off" || winKey.taskRegistered}
      <div class="win-key-status" class:error={!!winKeyError || winKey.status === "failed"}>
        {#if winKeyBusy}
          <span>{t("winKey.working")}</span>
        {:else if winKeyError}
          <span>{t("winKey.error", { error: winKeyError })}</span>
        {:else if winKey.status === "active"}
          <span>{t("winKey.active")}</span>
        {:else if winKey.status === "starting"}
          <span>{t("winKey.starting")}</span>
        {:else if winKey.status === "needsAuthorization"}
          <span>{t("winKey.needsAuthorization")}</span>
          <button type="button" class="btn" onclick={() => setWinKey(true)}>{t("winKey.authorize")}</button>
        {:else if winKey.status === "failed"}
          <span>{t("winKey.failed")}</span>
          <button type="button" class="btn" onclick={() => setWinKey(true)}>{t("winKey.retry")}</button>
        {:else if winKey.taskRegistered}
          <span>{t("winKey.leftover")}</span>
          <button type="button" class="btn" onclick={() => setWinKey(false)}>{t("winKey.remove")}</button>
        {/if}
      </div>
    {/if}
    <SettingsRow label={t("launcherPage.iconSize")}>
      <Slider
        value={launcher.iconSize}
        min={40}
        max={96}
        step={4}
        format={(v) => `${v}px`}
        onchange={(v) => editor.update((s) => (s.launcher.iconSize = v))}
      />
    </SettingsRow>
    <SettingsRow label={t("launcherPage.shortcut")}>
      {@render shortcut("open-launcher")}
    </SettingsRow>
    <SettingsRow label={t("launcherPage.openNow")}>
      <button type="button" class="btn" onclick={() => api.overlayToggleLauncher()}>
        <Icon name="LayoutGrid" size={14} />
        {t("launcherPage.openNow")}
      </button>
    </SettingsRow>
  </SettingsGroup>

  <SettingsGroup title={t("launcherPage.switcher")}>
    <SettingsRow label={t("launcherPage.shortcut")} hint={t("launcherPage.switcherHint")}>
      {@render shortcut("window-switcher")}
    </SettingsRow>
  </SettingsGroup>
</div>

<style>
  .win-key-status {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin: -4px 0 6px;
    padding: 8px 12px;
    border-radius: var(--radius-m);
    background: var(--surface-2);
    color: var(--fg-secondary);
    font-size: 12.5px;

    &.error {
      color: var(--danger);
      background: var(--danger-soft);
    }
  }

  .shortcut {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  kbd {
    min-width: 26px;
    padding: 3px 8px;
    border-radius: 6px;
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    box-shadow: inset 0 -1px 0 var(--border-strong);
    font-family: var(--font);
    font-size: 12px;
    font-weight: 600;
    text-align: center;
  }

  .muted {
    color: var(--fg-muted);
    font-size: 13px;
  }
</style>
