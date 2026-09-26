<script lang="ts">
  import Icon from "@shared/components/Icon.svelte";
  import { t } from "@shared/i18n/index.svelte.ts";
  import { api } from "@shared/ipc.ts";
  import type { KeyboardLayout } from "@shared/types.ts";
  import type { PopupProps } from "../registry.ts";

  let { data, close }: PopupProps = $props();
  // svelte-ignore state_referenced_locally
  let layouts = $state<KeyboardLayout[]>(data ?? []);

  function select(layout: KeyboardLayout) {
    api.setKeyboardLayout(layout.id);
    close();
  }
</script>

<div class="popup-surface keyboard">
  <div class="popup-header">{t("keyboard.title")}</div>
  {#each layouts as layout (layout.id)}
    <button type="button" class="layout" class:active={layout.active} onclick={() => select(layout)}>
      <Icon name="Keyboard" size={18} />
      <span class="layout-text">
        <span class="layout-name">{layout.displayName}</span>
        {#if layout.layoutName}
          <span class="layout-sub">{layout.layoutName}</span>
        {/if}
      </span>
    </button>
  {/each}
  <button
    type="button"
    class="popup-link"
    onclick={() => {
      api.shellAction("keyboard-settings");
      close();
    }}
  >
    {t("keyboard.more")}
  </button>
</div>

<style>
  .keyboard {
    width: 300px;
    padding: 8px;
  }

  .layout {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 10px 12px;
    margin-bottom: 4px;
    border-radius: var(--radius-m);
    text-align: left;
    color: var(--fg);
    transition: background-color var(--dur-fast) ease;

    &:hover {
      background: var(--surface-hover);
    }
    &.active {
      background: var(--accent-soft);
    }
  }

  .layout-text {
    display: flex;
    flex-direction: column;
  }

  .layout-name {
    font-weight: 700;
  }

  .layout-sub {
    font-size: 12px;
    color: var(--fg-secondary);
  }
</style>
