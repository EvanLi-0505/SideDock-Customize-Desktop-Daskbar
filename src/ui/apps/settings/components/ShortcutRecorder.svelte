<!-- Click, then press a key combination. Esc cancels. While recording, every global
     shortcut is released so the pressed keys reach this page instead of their action. -->
<script lang="ts">
  import { onDestroy } from "svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api } from "@shared/ipc.ts";
  import { acceleratorFromEvent, acceleratorKeys, isValidAccelerator } from "@shared/shortcuts.ts";

  interface Props {
    value: string;
    /** returns an error message to show, or null when accepted */
    onchange: (accelerator: string) => string | null;
  }
  let { value, onchange }: Props = $props();

  let recording = $state(false);
  let error = $state<string | null>(null);
  let button: HTMLButtonElement | null = $state(null);

  function start() {
    if (recording) return;
    recording = true;
    error = null;
    api.shortcutsSuspend(true);
    window.addEventListener("keydown", onKey, true);
  }

  function stop() {
    if (!recording) return;
    recording = false;
    window.removeEventListener("keydown", onKey, true);
    api.shortcutsSuspend(false);
  }

  function onKey(e: KeyboardEvent) {
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape" && !e.ctrlKey && !e.altKey && !e.metaKey) {
      stop();
      button?.blur();
      return;
    }
    const accelerator = acceleratorFromEvent(e);
    if (!accelerator) return; // only modifiers so far
    if (!isValidAccelerator(accelerator)) {
      error = t("shortcuts.invalid");
      return;
    }
    error = onchange(accelerator);
    if (!error) {
      stop();
      button?.blur();
    }
  }

  onDestroy(stop);
</script>

<div class="recorder">
  <button
    type="button"
    class="field"
    class:recording
    bind:this={button}
    onclick={start}
    onblur={stop}
  >
    {#if recording}
      <span class="placeholder">{t("shortcuts.record")}</span>
    {:else if value}
      {#each acceleratorKeys(value) as key, i (i)}
        <kbd>{key}</kbd>
      {/each}
    {:else}
      <span class="placeholder">{t("shortcuts.none")}</span>
    {/if}
  </button>
  {#if error}
    <span class="error">{error}</span>
  {/if}
</div>

<style>
  .recorder {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 4px;
  }

  .field {
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 220px;
    height: 34px;
    padding: 0 10px;
    border-radius: var(--radius-s);
    border: 1px solid var(--border-strong);
    background: var(--surface);
    transition: border-color var(--dur-fast) ease;

    &:hover {
      border-color: var(--accent);
    }
    &.recording {
      border-color: var(--accent);
      box-shadow: 0 0 0 3px var(--accent-soft);
    }
  }

  kbd {
    min-width: 24px;
    padding: 2px 7px;
    border-radius: 5px;
    border: 1px solid var(--border-strong);
    border-bottom-width: 2px;
    background: var(--surface-2);
    font-family: var(--font);
    font-size: 12px;
    font-weight: 700;
    text-align: center;
  }

  .placeholder {
    color: var(--fg-muted);
    font-size: 13px;
  }

  .error {
    color: var(--danger);
    font-size: 12px;
  }
</style>
