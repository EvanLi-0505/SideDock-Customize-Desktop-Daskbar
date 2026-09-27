<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "@shared/components/Icon.svelte";
  import { t, type TranslationKey } from "@shared/i18n/index.svelte.ts";
  import { api, Events, on } from "@shared/ipc.ts";
  import { SHORTCUT_ACTIONS, acceleratorKeys, defaultShortcuts } from "@shared/shortcuts.ts";
  import type { ShortcutStatus } from "@shared/types.ts";
  import SettingsGroup from "../components/SettingsGroup.svelte";
  import SettingsRow from "../components/SettingsRow.svelte";
  import ShortcutRecorder from "../components/ShortcutRecorder.svelte";
  import { editor } from "../state/editor.svelte.ts";

  let status = $state<Record<string, ShortcutStatus>>({});

  onMount(() => {
    api.getShortcutStatus().then((s) => (status = s));
    const unlisten = on<Record<string, ShortcutStatus>>(Events.ShortcutStatus, (s) => (status = s));
    return () => {
      unlisten.then((f) => f());
    };
  });

  function assign(action: string, accelerator: string): string | null {
    const owner = Object.entries(editor.value.shortcuts).find(
      ([other, value]) => other !== action && value === accelerator,
    );
    if (owner) return t("shortcuts.duplicate");
    editor.update((s) => (s.shortcuts[action] = accelerator));
    return null;
  }

  function hint(action: string): string | undefined {
    const state = status[action];
    if (state === "conflict") return t("shortcuts.conflict");
    if (state === "invalid") return t("shortcuts.invalid");
    const fallback = SHORTCUT_ACTIONS.find((a) => a.id === action)?.defaultAccelerator ?? "";
    return `${t("shortcuts.default")}: ${acceleratorKeys(fallback).join(" + ")}`;
  }
</script>

<div class="stack">
  <div class="description">{t("shortcuts.description")}</div>

  <SettingsGroup>
    {#each SHORTCUT_ACTIONS as action (action.id)}
      {@const state = status[action.id]}
      <SettingsRow label={t(`shortcut.${action.id}` as TranslationKey)} hint={hint(action.id)}>
        {#if state === "conflict" || state === "invalid"}
          <span class="badge danger"><Icon name="CircleAlert" size={14} /></span>
        {/if}
        <ShortcutRecorder
          value={editor.value.shortcuts[action.id] ?? ""}
          onchange={(accelerator) => assign(action.id, accelerator)}
        />
        <button
          type="button"
          class="btn btn-icon"
          title={t("shortcuts.clear")}
          disabled={!editor.value.shortcuts[action.id]}
          onclick={() => editor.update((s) => (s.shortcuts[action.id] = ""))}
        >
          <Icon name="X" size={16} />
        </button>
      </SettingsRow>
    {/each}
  </SettingsGroup>

  <SettingsGroup>
    <SettingsRow label={t("shortcuts.resetAll")} strong>
      <button type="button" class="btn btn-ghost" onclick={() => editor.update((s) => (s.shortcuts = defaultShortcuts()))}>
        <Icon name="RotateCcw" size={14} />
        {t("common.reset")}
      </button>
    </SettingsRow>
  </SettingsGroup>

  <div class="tip">
    <Icon name="Info" size={16} />
    <span>{t("shortcuts.gestureTip")}</span>
  </div>
</div>

<style>
  .badge.danger {
    display: flex;
    color: var(--danger);
  }

  .tip {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 12px 16px;
    border-radius: var(--radius-m);
    background: var(--accent-soft);
    color: var(--fg-secondary);
    font-size: 13px;
    line-height: 1.5;
  }
</style>
